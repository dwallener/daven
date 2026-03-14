use anyhow::{Context, Result};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use config::AppConfig;
use domain_models::{Detection, PointGeometry};
use event_contracts::{DetectionCreated, EventEnvelope};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row, postgres::PgPoolOptions};
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

#[derive(Debug, Deserialize)]
struct CreateDetectionRequest {
    pub source_type: String,
    pub source_id: String,
    pub external_ref: Option<String>,
    pub timestamp: Option<DateTime<Utc>>,
    pub location: PointGeometry,
    pub classification: Option<String>,
    pub confidence: Option<f32>,
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
        .with_context(|| "failed to connect ingest-service to postgres")?;

    bootstrap_database(&db).await?;

    let state = AppState { db };
    let app = Router::new()
        .route("/health", get(health))
        .route("/ingest/detections", post(create_detection))
        .route("/detections/{id}", get(get_detection))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    tracing::info!(address = %addr, "ingest-service listening");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn bootstrap_database(db: &PgPool) -> Result<()> {
    sqlx::raw_sql(include_str!("../../../infra/db/migrations/0001_init.sql"))
        .execute(db)
        .await
        .with_context(|| "failed to execute ingest bootstrap schema")?;
    Ok(())
}

async fn health(State(state): State<AppState>) -> Result<Json<HealthResponse>, ApiError> {
    sqlx::query("select 1")
        .execute(&state.db)
        .await
        .map_err(ApiError::internal)?;

    Ok(Json(HealthResponse {
        service: "ingest-service",
        status: "ok",
    }))
}

async fn create_detection(
    State(state): State<AppState>,
    Json(request): Json<CreateDetectionRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let detection = Detection {
        id: format!("det_{}", Uuid::new_v4().simple()),
        source_type: request.source_type,
        source_id: request.source_id,
        external_ref: request.external_ref,
        timestamp: request.timestamp.unwrap_or_else(Utc::now),
        geometry: request.location,
        classification: request.classification,
        confidence: request.confidence,
    };

    sqlx::query(
        r#"
        insert into detections (
            id, source_type, source_id, external_ref, ts, longitude, latitude, classification, confidence
        )
        values ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        "#,
    )
    .bind(&detection.id)
    .bind(&detection.source_type)
    .bind(&detection.source_id)
    .bind(&detection.external_ref)
    .bind(detection.timestamp)
    .bind(detection.geometry.coordinates[0])
    .bind(detection.geometry.coordinates[1])
    .bind(&detection.classification)
    .bind(detection.confidence)
    .execute(&state.db)
    .await
    .map_err(ApiError::internal)?;

    let event = EventEnvelope::new(
        "DetectionCreated",
        "ingest-service",
        DetectionCreated {
            detection: detection.clone(),
        },
    );

    Ok((
        StatusCode::CREATED,
        Json(EventResponse {
            resource: detection,
            event: serde_json::to_value(event).map_err(ApiError::internal)?,
        }),
    ))
}

async fn get_detection(
    Path(id): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<Detection>, ApiError> {
    let row = sqlx::query(
        r#"
        select id, source_type, source_id, external_ref, ts, longitude, latitude, classification, confidence
        from detections
        where id = $1
        "#,
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .map_err(ApiError::internal)?;

    let row = row.ok_or_else(|| ApiError::not_found(format!("detection {id} was not found")))?;
    Ok(Json(map_detection_row(row)?))
}

fn map_detection_row(row: sqlx::postgres::PgRow) -> Result<Detection, ApiError> {
    let longitude: f64 = row.try_get("longitude").map_err(ApiError::internal)?;
    let latitude: f64 = row.try_get("latitude").map_err(ApiError::internal)?;
    Ok(Detection {
        id: row.try_get("id").map_err(ApiError::internal)?,
        source_type: row.try_get("source_type").map_err(ApiError::internal)?,
        source_id: row.try_get("source_id").map_err(ApiError::internal)?,
        external_ref: row.try_get("external_ref").map_err(ApiError::internal)?,
        timestamp: row.try_get("ts").map_err(ApiError::internal)?,
        geometry: PointGeometry::point(longitude, latitude),
        classification: row.try_get("classification").map_err(ApiError::internal)?,
        confidence: row.try_get("confidence").map_err(ApiError::internal)?,
    })
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
