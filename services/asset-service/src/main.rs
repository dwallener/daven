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
use domain_models::{Asset, AssetAvailability, PointGeometry};
use event_contracts::{AssetUpdated, EventEnvelope};
use serde::{Deserialize, Serialize};
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

#[derive(Debug, Deserialize)]
struct CreateAssetRequest {
    pub callsign: String,
    pub platform_type: String,
    pub domain: String,
    pub location: PointGeometry,
    pub availability: Option<AssetAvailability>,
    pub capabilities: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct UpdateAssetRequest {
    pub callsign: Option<String>,
    pub platform_type: Option<String>,
    pub domain: Option<String>,
    pub availability: Option<AssetAvailability>,
    pub capabilities: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct AssetTelemetryRequest {
    pub location: PointGeometry,
    pub recorded_at: Option<DateTime<Utc>>,
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
        .with_context(|| "failed to connect asset-service to postgres")?;

    bootstrap_database(&db).await?;

    let state = AppState { db };
    let app = Router::new()
        .route("/health", get(health))
        .route("/assets", get(list_assets).post(create_asset))
        .route("/assets/{id}", get(get_asset).patch(update_asset))
        .route("/assets/{id}/telemetry", post(update_asset_telemetry))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    tracing::info!(address = %addr, "asset-service listening");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn bootstrap_database(db: &PgPool) -> Result<()> {
    sqlx::raw_sql(include_str!("../../../infra/db/migrations/0001_init.sql"))
        .execute(db)
        .await
        .with_context(|| "failed to execute asset bootstrap schema")?;
    Ok(())
}

async fn health(State(state): State<AppState>) -> Result<Json<HealthResponse>, ApiError> {
    sqlx::query("select 1")
        .execute(&state.db)
        .await
        .map_err(ApiError::internal)?;

    Ok(Json(HealthResponse {
        service: "asset-service",
        status: "ok",
    }))
}

async fn create_asset(
    State(state): State<AppState>,
    Json(request): Json<CreateAssetRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let asset = Asset {
        id: format!("asset_{}", Uuid::new_v4().simple()),
        callsign: request.callsign,
        platform_type: request.platform_type,
        domain: request.domain,
        location: request.location,
        availability: request.availability.unwrap_or(AssetAvailability::Available),
        capabilities: request.capabilities.unwrap_or_default(),
        updated_at: Utc::now(),
    };

    sqlx::query(
        r#"
        insert into assets (id, callsign, platform_type, domain, longitude, latitude, availability, capabilities, updated_at)
        values ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        "#,
    )
    .bind(&asset.id)
    .bind(&asset.callsign)
    .bind(&asset.platform_type)
    .bind(&asset.domain)
    .bind(asset.location.coordinates[0])
    .bind(asset.location.coordinates[1])
    .bind(serialize_availability(&asset.availability))
    .bind(SqlJson(asset.capabilities.clone()))
    .bind(asset.updated_at)
    .execute(&state.db)
    .await
    .map_err(ApiError::internal)?;

    let event = EventEnvelope::new(
        "AssetUpdated",
        "asset-service",
        AssetUpdated {
            asset: asset.clone(),
        },
    );

    Ok((
        StatusCode::CREATED,
        Json(EventResponse {
            resource: asset,
            event: serde_json::to_value(event).map_err(ApiError::internal)?,
        }),
    ))
}

async fn list_assets(State(state): State<AppState>) -> Result<Json<Vec<Asset>>, ApiError> {
    let rows = sqlx::query(
        r#"
        select id, callsign, platform_type, domain, longitude, latitude, availability, capabilities, updated_at
        from assets
        order by updated_at desc, callsign asc
        "#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(ApiError::internal)?;

    let assets = rows
        .into_iter()
        .map(map_asset_row)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Json(assets))
}

async fn get_asset(
    Path(id): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<Asset>, ApiError> {
    let row = sqlx::query(
        r#"
        select id, callsign, platform_type, domain, longitude, latitude, availability, capabilities, updated_at
        from assets
        where id = $1
        "#,
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .map_err(ApiError::internal)?;

    let row = row.ok_or_else(|| ApiError::not_found(format!("asset {id} was not found")))?;
    Ok(Json(map_asset_row(row)?))
}

async fn update_asset(
    Path(id): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<UpdateAssetRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let mut asset = load_asset(&state.db, &id).await?;

    if let Some(callsign) = request.callsign {
        asset.callsign = callsign;
    }
    if let Some(platform_type) = request.platform_type {
        asset.platform_type = platform_type;
    }
    if let Some(domain) = request.domain {
        asset.domain = domain;
    }
    if let Some(availability) = request.availability {
        asset.availability = availability;
    }
    if let Some(capabilities) = request.capabilities {
        asset.capabilities = capabilities;
    }
    asset.updated_at = Utc::now();

    sqlx::query(
        r#"
        update assets
        set callsign = $2,
            platform_type = $3,
            domain = $4,
            availability = $5,
            capabilities = $6,
            updated_at = $7
        where id = $1
        "#,
    )
    .bind(&asset.id)
    .bind(&asset.callsign)
    .bind(&asset.platform_type)
    .bind(&asset.domain)
    .bind(serialize_availability(&asset.availability))
    .bind(SqlJson(asset.capabilities.clone()))
    .bind(asset.updated_at)
    .execute(&state.db)
    .await
    .map_err(ApiError::internal)?;

    let event = EventEnvelope::new(
        "AssetUpdated",
        "asset-service",
        AssetUpdated {
            asset: asset.clone(),
        },
    );

    Ok(Json(EventResponse {
        resource: asset,
        event: serde_json::to_value(event).map_err(ApiError::internal)?,
    }))
}

async fn update_asset_telemetry(
    Path(id): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<AssetTelemetryRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let mut asset = load_asset(&state.db, &id).await?;
    let recorded_at = request.recorded_at.unwrap_or_else(Utc::now);
    asset.location = request.location;
    asset.updated_at = recorded_at;

    let mut tx = state.db.begin().await.map_err(ApiError::internal)?;
    sqlx::query(
        r#"
        insert into asset_telemetry (asset_id, longitude, latitude, recorded_at)
        values ($1, $2, $3, $4)
        "#,
    )
    .bind(&asset.id)
    .bind(asset.location.coordinates[0])
    .bind(asset.location.coordinates[1])
    .bind(recorded_at)
    .execute(&mut *tx)
    .await
    .map_err(ApiError::internal)?;

    sqlx::query(
        r#"
        update assets
        set longitude = $2,
            latitude = $3,
            updated_at = $4
        where id = $1
        "#,
    )
    .bind(&asset.id)
    .bind(asset.location.coordinates[0])
    .bind(asset.location.coordinates[1])
    .bind(asset.updated_at)
    .execute(&mut *tx)
    .await
    .map_err(ApiError::internal)?;
    tx.commit().await.map_err(ApiError::internal)?;

    let event = EventEnvelope::new(
        "AssetUpdated",
        "asset-service",
        AssetUpdated {
            asset: asset.clone(),
        },
    );

    Ok(Json(EventResponse {
        resource: asset,
        event: serde_json::to_value(event).map_err(ApiError::internal)?,
    }))
}

async fn load_asset(db: &PgPool, id: &str) -> Result<Asset, ApiError> {
    let row = sqlx::query(
        r#"
        select id, callsign, platform_type, domain, longitude, latitude, availability, capabilities, updated_at
        from assets
        where id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(db)
    .await
    .map_err(ApiError::internal)?;

    let row = row.ok_or_else(|| ApiError::not_found(format!("asset {id} was not found")))?;
    map_asset_row(row)
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

fn serialize_availability(value: &AssetAvailability) -> &'static str {
    match value {
        AssetAvailability::Available => "AVAILABLE",
        AssetAvailability::Tasked => "TASKED",
        AssetAvailability::Unavailable => "UNAVAILABLE",
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
