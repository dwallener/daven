use anyhow::{Context, Result};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
};
use chrono::Utc;
use config::AppConfig;
use domain_models::{
    Asset, AssetAvailability, PointGeometry, Recommendation, RecommendationCandidate, Target,
    TargetStateTransition, TargetStatus,
};
use event_contracts::{EventEnvelope, RecommendationGenerated};
use serde::Serialize;
use sqlx::{PgPool, Row, postgres::PgPoolOptions, types::Json as SqlJson};
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use uuid::Uuid;

#[derive(Clone)]
struct AppState {
    db: PgPool,
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    service: &'static str,
    status: &'static str,
}

#[derive(Debug, Serialize)]
struct EventResponse<T> {
    pub resource: T,
    pub event: serde_json::Value,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let config = AppConfig::from_env()?;
    let addr = config.socket_addr()?;

    let db = PgPoolOptions::new()
        .max_connections(10)
        .connect(&config.database_url)
        .await
        .with_context(|| "failed to connect recommendation-service to postgres")?;

    bootstrap_database(&db).await?;

    let state = AppState { db };
    let app = Router::new()
        .route("/health", get(health))
        .route(
            "/recommendations/targets/{target_id}",
            get(generate_recommendation),
        )
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    tracing::info!(address = %addr, "recommendation-service listening");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn bootstrap_database(db: &PgPool) -> Result<()> {
    sqlx::raw_sql(include_str!("../../../infra/db/migrations/0001_init.sql"))
        .execute(db)
        .await
        .with_context(|| "failed to execute recommendation bootstrap schema")?;
    Ok(())
}

async fn health(State(state): State<AppState>) -> Result<Json<HealthResponse>, ApiError> {
    sqlx::query("select 1")
        .execute(&state.db)
        .await
        .map_err(ApiError::internal)?;

    Ok(Json(HealthResponse {
        service: "recommendation-service",
        status: "ok",
    }))
}

async fn generate_recommendation(
    Path(target_id): Path<String>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let target = load_target(&state.db, &target_id).await?;
    let assets = load_available_assets(&state.db).await?;

    let weights = serde_json::json!({
        "capability_match": 0.6,
        "distance": 0.4
    });

    let mut candidates = assets
        .into_iter()
        .map(|asset| build_candidate(&target, asset))
        .collect::<Vec<_>>();

    candidates.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    for (index, candidate) in candidates.iter_mut().enumerate() {
        candidate.rank = (index + 1) as u32;
    }

    let recommendation = Recommendation {
        id: format!("rec_{}", Uuid::new_v4().simple()),
        target_id: target.id.clone(),
        generated_at: Utc::now(),
        candidates,
    };

    persist_recommendation(&state.db, &recommendation, weights.clone()).await?;

    let event = EventEnvelope::new(
        "RecommendationGenerated",
        "recommendation-service",
        RecommendationGenerated {
            recommendation: recommendation.clone(),
        },
    );

    Ok(Json(EventResponse {
        resource: recommendation,
        event: serde_json::to_value(event).map_err(ApiError::internal)?,
    }))
}

fn build_candidate(target: &Target, asset: Asset) -> RecommendationCandidate {
    let distance_km = haversine_km(target.location.coordinates, asset.location.coordinates);
    let capability_match = if asset
        .capabilities
        .iter()
        .any(|capability| capability == "strike")
    {
        1.0
    } else if asset
        .capabilities
        .iter()
        .any(|capability| capability == "observe")
    {
        0.6
    } else {
        0.2
    };
    let distance_score = (1.0 - (distance_km / 100.0)).clamp(0.0, 1.0);
    let score = (capability_match * 0.6) + (distance_score * 0.4);

    RecommendationCandidate {
        asset_id: asset.id,
        score,
        rank: 0,
        explanation: serde_json::json!({
            "distance_km": distance_km,
            "capability_match": capability_match,
            "availability": "AVAILABLE"
        }),
    }
}

async fn persist_recommendation(
    db: &PgPool,
    recommendation: &Recommendation,
    weights: serde_json::Value,
) -> Result<(), ApiError> {
    let mut tx = db.begin().await.map_err(ApiError::internal)?;

    sqlx::query(
        r#"
        insert into recommendations (id, target_id, generated_at, weights)
        values ($1, $2, $3, $4)
        "#,
    )
    .bind(&recommendation.id)
    .bind(&recommendation.target_id)
    .bind(recommendation.generated_at)
    .bind(SqlJson(weights))
    .execute(&mut *tx)
    .await
    .map_err(ApiError::internal)?;

    for candidate in &recommendation.candidates {
        sqlx::query(
            r#"
            insert into recommendation_candidates (recommendation_id, asset_id, score, rank, explanation)
            values ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(&recommendation.id)
        .bind(&candidate.asset_id)
        .bind(candidate.score)
        .bind(candidate.rank as i32)
        .bind(SqlJson(candidate.explanation.clone()))
        .execute(&mut *tx)
        .await
        .map_err(ApiError::internal)?;
    }

    tx.commit().await.map_err(ApiError::internal)?;
    Ok(())
}

async fn load_target(db: &PgPool, id: &str) -> Result<Target, ApiError> {
    let row = sqlx::query(
        r#"
        select id, board_id, title, status, classification, priority, longitude, latitude,
               source_detection_id, created_by, created_at, updated_at, labels
        from targets
        where id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(db)
    .await
    .map_err(ApiError::internal)?;

    let row = row.ok_or_else(|| ApiError::not_found(format!("target {id} was not found")))?;
    let history_rows = sqlx::query(
        r#"
        select from_status, to_status, actor, transitioned_at
        from target_state_history
        where target_id = $1
        order by id asc
        "#,
    )
    .bind(id)
    .fetch_all(db)
    .await
    .map_err(ApiError::internal)?;

    map_target_row(row, history_rows)
}

async fn load_available_assets(db: &PgPool) -> Result<Vec<Asset>, ApiError> {
    let rows = sqlx::query(
        r#"
        select id, callsign, platform_type, domain, longitude, latitude, availability, capabilities, updated_at
        from assets
        where availability = 'AVAILABLE'
        order by updated_at desc, callsign asc
        "#,
    )
    .fetch_all(db)
    .await
    .map_err(ApiError::internal)?;

    rows.into_iter().map(map_asset_row).collect()
}

fn map_asset_row(row: sqlx::postgres::PgRow) -> Result<Asset, ApiError> {
    let longitude: f64 = row.try_get("longitude").map_err(ApiError::internal)?;
    let latitude: f64 = row.try_get("latitude").map_err(ApiError::internal)?;
    let capabilities: SqlJson<Vec<String>> =
        row.try_get("capabilities").map_err(ApiError::internal)?;
    let availability: String = row.try_get("availability").map_err(ApiError::internal)?;

    Ok(Asset {
        id: row.try_get("id").map_err(ApiError::internal)?,
        callsign: row.try_get("callsign").map_err(ApiError::internal)?,
        platform_type: row.try_get("platform_type").map_err(ApiError::internal)?,
        domain: row.try_get("domain").map_err(ApiError::internal)?,
        location: PointGeometry::point(longitude, latitude),
        availability: parse_availability(&availability)?,
        capabilities: capabilities.0,
        updated_at: row.try_get("updated_at").map_err(ApiError::internal)?,
    })
}

fn map_target_row(
    row: sqlx::postgres::PgRow,
    history_rows: Vec<sqlx::postgres::PgRow>,
) -> Result<Target, ApiError> {
    let longitude: f64 = row.try_get("longitude").map_err(ApiError::internal)?;
    let latitude: f64 = row.try_get("latitude").map_err(ApiError::internal)?;
    let labels: SqlJson<Vec<String>> = row.try_get("labels").map_err(ApiError::internal)?;

    let state_history = history_rows
        .into_iter()
        .map(|history_row| {
            let from_status = history_row
                .try_get::<Option<String>, _>("from_status")
                .map_err(ApiError::internal)?
                .map(|value| parse_target_status(&value))
                .transpose()?;
            let to_status: String = history_row
                .try_get("to_status")
                .map_err(ApiError::internal)?;
            Ok(TargetStateTransition {
                from: from_status,
                to: parse_target_status(&to_status)?,
                at: history_row
                    .try_get("transitioned_at")
                    .map_err(ApiError::internal)?,
                by: history_row.try_get("actor").map_err(ApiError::internal)?,
            })
        })
        .collect::<Result<Vec<_>, ApiError>>()?;

    let status: String = row.try_get("status").map_err(ApiError::internal)?;
    Ok(Target {
        id: row.try_get("id").map_err(ApiError::internal)?,
        board_id: row.try_get("board_id").map_err(ApiError::internal)?,
        title: row.try_get("title").map_err(ApiError::internal)?,
        status: parse_target_status(&status)?,
        classification: row.try_get("classification").map_err(ApiError::internal)?,
        priority: row.try_get("priority").map_err(ApiError::internal)?,
        location: PointGeometry::point(longitude, latitude),
        source_detection_id: row
            .try_get("source_detection_id")
            .map_err(ApiError::internal)?,
        created_by: row.try_get("created_by").map_err(ApiError::internal)?,
        created_at: row.try_get("created_at").map_err(ApiError::internal)?,
        updated_at: row.try_get("updated_at").map_err(ApiError::internal)?,
        labels: labels.0,
        state_history,
    })
}

fn parse_target_status(value: &str) -> Result<TargetStatus, ApiError> {
    match value {
        "NOMINATED" => Ok(TargetStatus::Nominated),
        "TRIAGED" => Ok(TargetStatus::Triaged),
        "PENDING_PAIRING" => Ok(TargetStatus::PendingPairing),
        "PAIRED" => Ok(TargetStatus::Paired),
        "PLAN_DRAFTED" => Ok(TargetStatus::PlanDrafted),
        "PENDING_APPROVAL" => Ok(TargetStatus::PendingApproval),
        "APPROVED" => Ok(TargetStatus::Approved),
        "IN_EXECUTION" => Ok(TargetStatus::InExecution),
        "PENDING_BDA" => Ok(TargetStatus::PendingBda),
        "ASSESSED_COMPLETE" => Ok(TargetStatus::AssessedComplete),
        "REJECTED" => Ok(TargetStatus::Rejected),
        "ARCHIVED" => Ok(TargetStatus::Archived),
        _ => Err(ApiError::internal(format!(
            "unknown target status: {value}"
        ))),
    }
}

fn parse_availability(value: &str) -> Result<AssetAvailability, ApiError> {
    match value {
        "AVAILABLE" => Ok(AssetAvailability::Available),
        "TASKED" => Ok(AssetAvailability::Tasked),
        "UNAVAILABLE" => Ok(AssetAvailability::Unavailable),
        _ => Err(ApiError::internal(format!(
            "unknown asset availability: {value}"
        ))),
    }
}

fn haversine_km(from: [f64; 2], to: [f64; 2]) -> f32 {
    let earth_radius_km = 6371.0_f64;
    let lon1 = from[0].to_radians();
    let lat1 = from[1].to_radians();
    let lon2 = to[0].to_radians();
    let lat2 = to[1].to_radians();
    let delta_lon = lon2 - lon1;
    let delta_lat = lat2 - lat1;
    let a =
        (delta_lat / 2.0).sin().powi(2) + lat1.cos() * lat2.cos() * (delta_lon / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());
    (earth_radius_km * c) as f32
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn not_found(message: String) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message,
        }
    }

    fn internal(error: impl std::fmt::Display) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: error.to_string(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        (
            self.status,
            Json(serde_json::json!({
                "error": self.message,
            })),
        )
            .into_response()
    }
}
