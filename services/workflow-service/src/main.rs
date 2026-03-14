use std::collections::BTreeMap;

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
use domain_models::{Board, Detection, PointGeometry, Target, TargetStateTransition, TargetStatus};
use event_contracts::{EventEnvelope, TargetNominated, TargetTransitioned};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres, Row, Transaction, postgres::PgPoolOptions, types::Json as SqlJson};
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use uuid::Uuid;

const DEFAULT_BOARD_ID: &str = "dynamic-main";
const DEFAULT_BOARD_NAME: &str = "Dynamic Main";
const DEFAULT_BOARD_COLUMNS: [&str; 7] = [
    "DELIBERATE",
    "PENDING_PAIRING",
    "PAIRED",
    "IN_EXECUTION",
    "PENDING_BDA",
    "COMPLETE",
    "REJECTED_ARCHIVED",
];

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
struct CreateTargetRequest {
    pub board_id: Option<String>,
    pub title: String,
    pub classification: Option<String>,
    pub priority: Option<i32>,
    pub location: PointGeometry,
    pub created_by: String,
    pub labels: Option<Vec<String>>,
    pub source_detection_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TransitionTargetRequest {
    pub to_status: TargetStatus,
    pub actor: String,
}

#[derive(Debug, Deserialize)]
struct NominateDetectionRequest {
    pub board_id: Option<String>,
    pub title: Option<String>,
    pub classification: Option<String>,
    pub actor: String,
    pub labels: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
struct BoardTargetsResponse {
    pub board: Board,
    pub columns: BTreeMap<String, Vec<Target>>,
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
        .with_context(|| "failed to connect workflow-service to postgres")?;

    bootstrap_database(&db).await?;

    let state = AppState { db };

    let app = Router::new()
        .route("/health", get(health))
        .route("/targets", post(create_target))
        .route("/targets/{id}", get(get_target))
        .route("/targets/{id}/transition", post(transition_target))
        .route("/boards", get(list_boards))
        .route("/boards/{id}", get(get_board))
        .route("/boards/{id}/targets", get(get_board_targets))
        .route("/detections/{id}/nominate", post(nominate_detection))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    tracing::info!(address = %addr, "workflow-service listening");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn bootstrap_database(db: &PgPool) -> Result<()> {
    sqlx::raw_sql(include_str!("../../../infra/db/migrations/0001_init.sql"))
        .execute(db)
        .await
        .with_context(|| "failed to execute workflow bootstrap schema")?;

    sqlx::query(
        r#"
        insert into boards (id, name, statuses)
        values ($1, $2, $3)
        on conflict (id) do update
        set name = excluded.name,
            statuses = excluded.statuses
        "#,
    )
    .bind(DEFAULT_BOARD_ID)
    .bind(DEFAULT_BOARD_NAME)
    .bind(SqlJson(
        DEFAULT_BOARD_COLUMNS
            .iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>(),
    ))
    .execute(db)
    .await
    .with_context(|| "failed to seed default workflow board")?;

    Ok(())
}

async fn health(State(state): State<AppState>) -> Result<Json<HealthResponse>, ApiError> {
    sqlx::query("select 1")
        .execute(&state.db)
        .await
        .map_err(ApiError::internal)?;

    Ok(Json(HealthResponse {
        service: "workflow-service",
        status: "ok",
    }))
}

async fn create_target(
    State(state): State<AppState>,
    Json(request): Json<CreateTargetRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let board_id = request
        .board_id
        .unwrap_or_else(|| DEFAULT_BOARD_ID.to_string());
    ensure_board_exists(&state.db, &board_id).await?;

    let now = Utc::now();
    let target = Target {
        id: format!("tgt_{}", Uuid::new_v4().simple()),
        board_id,
        title: request.title,
        status: TargetStatus::Nominated,
        classification: request.classification,
        priority: request.priority.unwrap_or(50),
        location: request.location,
        source_detection_id: request.source_detection_id,
        created_by: request.created_by.clone(),
        created_at: now,
        updated_at: now,
        labels: request.labels.unwrap_or_default(),
        state_history: vec![TargetStateTransition {
            from: None,
            to: TargetStatus::Nominated,
            at: now,
            by: request.created_by,
        }],
    };

    insert_target(&state.db, &target).await?;
    Ok((StatusCode::CREATED, Json(target)))
}

async fn get_target(
    Path(id): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<Target>, ApiError> {
    let target = load_target(&state.db, &id).await?;
    Ok(Json(target))
}

async fn transition_target(
    Path(id): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<TransitionTargetRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let mut tx = state.db.begin().await.map_err(ApiError::internal)?;
    let target = load_target_in_tx(&mut tx, &id).await?;
    let current = target.status.clone();
    validate_transition(&current, &request.to_status)?;

    let now = Utc::now();
    sqlx::query(
        r#"
        update targets
        set status = $2,
            updated_at = $3
        where id = $1
        "#,
    )
    .bind(&id)
    .bind(serialize_status(&request.to_status))
    .bind(now)
    .execute(&mut *tx)
    .await
    .map_err(ApiError::internal)?;

    sqlx::query(
        r#"
        insert into target_state_history (target_id, from_status, to_status, actor, transitioned_at)
        values ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(&id)
    .bind(Some(serialize_status(&current)))
    .bind(serialize_status(&request.to_status))
    .bind(&request.actor)
    .bind(now)
    .execute(&mut *tx)
    .await
    .map_err(ApiError::internal)?;

    tx.commit().await.map_err(ApiError::internal)?;
    let updated_target = load_target(&state.db, &id).await?;

    let event = EventEnvelope::new(
        "TargetTransitioned",
        "workflow-service",
        TargetTransitioned {
            target_id: updated_target.id.clone(),
            from: current,
            to: request.to_status,
            actor: request.actor,
        },
    );

    Ok((
        StatusCode::OK,
        Json(EventResponse {
            resource: updated_target,
            event: serde_json::to_value(event).map_err(ApiError::internal)?,
        }),
    ))
}

async fn list_boards(State(state): State<AppState>) -> Result<Json<Vec<Board>>, ApiError> {
    let boards = load_boards(&state.db).await?;
    Ok(Json(boards))
}

async fn get_board(
    Path(id): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<Board>, ApiError> {
    let board = load_board(&state.db, &id).await?;
    Ok(Json(board))
}

async fn get_board_targets(
    Path(id): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<BoardTargetsResponse>, ApiError> {
    let board = load_board(&state.db, &id).await?;
    let targets = load_targets_for_board(&state.db, &id).await?;

    let mut columns: BTreeMap<String, Vec<Target>> = board
        .statuses
        .iter()
        .map(|status| (status.clone(), Vec::new()))
        .collect();

    for target in targets {
        columns
            .entry(target.status.as_board_column().to_string())
            .or_default()
            .push(target);
    }

    Ok(Json(BoardTargetsResponse { board, columns }))
}

async fn nominate_detection(
    Path(detection_id): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<NominateDetectionRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let board_id = request
        .board_id
        .unwrap_or_else(|| DEFAULT_BOARD_ID.to_string());
    ensure_board_exists(&state.db, &board_id).await?;
    let detection = load_detection(&state.db, &detection_id).await?;

    let now = Utc::now();
    let target = Target {
        id: format!("tgt_{}", Uuid::new_v4().simple()),
        board_id,
        title: request
            .title
            .unwrap_or_else(|| default_target_title(&detection)),
        status: TargetStatus::Nominated,
        classification: request.classification.or(detection.classification.clone()),
        priority: 50,
        location: detection.geometry.clone(),
        source_detection_id: Some(detection_id),
        created_by: request.actor.clone(),
        created_at: now,
        updated_at: now,
        labels: request.labels.unwrap_or_default(),
        state_history: vec![TargetStateTransition {
            from: None,
            to: TargetStatus::Nominated,
            at: now,
            by: request.actor.clone(),
        }],
    };

    insert_target(&state.db, &target).await?;

    let event = EventEnvelope::new(
        "TargetNominated",
        "workflow-service",
        TargetNominated {
            target: target.clone(),
        },
    );

    Ok((
        StatusCode::CREATED,
        Json(EventResponse {
            resource: target,
            event: serde_json::to_value(event).map_err(ApiError::internal)?,
        }),
    ))
}

async fn insert_target(db: &PgPool, target: &Target) -> Result<(), ApiError> {
    let mut tx = db.begin().await.map_err(ApiError::internal)?;

    sqlx::query(
        r#"
        insert into targets (
            id, board_id, title, status, classification, priority, longitude, latitude,
            source_detection_id, created_by, created_at, updated_at, labels
        )
        values (
            $1, $2, $3, $4, $5, $6,
            $7, $8, $9, $10, $11, $12, $13
        )
        "#,
    )
    .bind(&target.id)
    .bind(&target.board_id)
    .bind(&target.title)
    .bind(serialize_status(&target.status))
    .bind(&target.classification)
    .bind(target.priority)
    .bind(target.location.coordinates[0])
    .bind(target.location.coordinates[1])
    .bind(&target.source_detection_id)
    .bind(&target.created_by)
    .bind(target.created_at)
    .bind(target.updated_at)
    .bind(SqlJson(target.labels.clone()))
    .execute(&mut *tx)
    .await
    .map_err(ApiError::internal)?;

    let initial_state = target
        .state_history
        .first()
        .ok_or_else(|| ApiError::internal("target is missing initial state history"))?;

    sqlx::query(
        r#"
        insert into target_state_history (target_id, from_status, to_status, actor, transitioned_at)
        values ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(&target.id)
    .bind(initial_state.from.as_ref().map(serialize_status))
    .bind(serialize_status(&initial_state.to))
    .bind(&initial_state.by)
    .bind(initial_state.at)
    .execute(&mut *tx)
    .await
    .map_err(ApiError::internal)?;

    tx.commit().await.map_err(ApiError::internal)?;
    Ok(())
}

async fn ensure_board_exists(db: &PgPool, board_id: &str) -> Result<(), ApiError> {
    let exists = sqlx::query_scalar::<_, bool>("select exists(select 1 from boards where id = $1)")
        .bind(board_id)
        .fetch_one(db)
        .await
        .map_err(ApiError::internal)?;

    if exists {
        Ok(())
    } else {
        Err(ApiError::not_found(format!(
            "board {board_id} was not found"
        )))
    }
}

async fn load_boards(db: &PgPool) -> Result<Vec<Board>, ApiError> {
    let rows = sqlx::query("select id, name, statuses from boards order by name")
        .fetch_all(db)
        .await
        .map_err(ApiError::internal)?;

    rows.into_iter().map(map_board_row).collect()
}

async fn load_board(db: &PgPool, id: &str) -> Result<Board, ApiError> {
    let row = sqlx::query("select id, name, statuses from boards where id = $1")
        .bind(id)
        .fetch_optional(db)
        .await
        .map_err(ApiError::internal)?;

    let row = row.ok_or_else(|| ApiError::not_found(format!("board {id} was not found")))?;
    map_board_row(row)
}

async fn load_targets_for_board(db: &PgPool, board_id: &str) -> Result<Vec<Target>, ApiError> {
    let rows = sqlx::query(
        r#"
        select
            id, board_id, title, status, classification, priority, longitude, latitude,
            source_detection_id, created_by, created_at, updated_at, labels
        from targets
        where board_id = $1
        order by updated_at desc, created_at desc
        "#,
    )
    .bind(board_id)
    .fetch_all(db)
    .await
    .map_err(ApiError::internal)?;

    let mut targets = Vec::with_capacity(rows.len());
    for row in rows {
        targets.push(map_target_row(db, row).await?);
    }
    Ok(targets)
}

async fn load_target(db: &PgPool, id: &str) -> Result<Target, ApiError> {
    let row = sqlx::query(
        r#"
        select
            id, board_id, title, status, classification, priority, longitude, latitude,
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
    map_target_row(db, row).await
}

async fn load_detection(db: &PgPool, id: &str) -> Result<Detection, ApiError> {
    let row = sqlx::query(
        r#"
        select id, source_type, source_id, external_ref, ts, longitude, latitude, classification, confidence
        from detections
        where id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(db)
    .await
    .map_err(ApiError::internal)?;

    let row = row.ok_or_else(|| ApiError::not_found(format!("detection {id} was not found")))?;
    map_detection_row(row)
}

async fn load_target_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    id: &str,
) -> Result<Target, ApiError> {
    let row = sqlx::query(
        r#"
        select
            id, board_id, title, status, classification, priority, longitude, latitude,
            source_detection_id, created_by, created_at, updated_at, labels
        from targets
        where id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(&mut **tx)
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
    .fetch_all(&mut **tx)
    .await
    .map_err(ApiError::internal)?;

    map_target_with_history_row(row, history_rows)
}

async fn map_target_row(db: &PgPool, row: sqlx::postgres::PgRow) -> Result<Target, ApiError> {
    let target_id: String = row.try_get("id").map_err(ApiError::internal)?;
    let history_rows = sqlx::query(
        r#"
        select from_status, to_status, actor, transitioned_at
        from target_state_history
        where target_id = $1
        order by id asc
        "#,
    )
    .bind(&target_id)
    .fetch_all(db)
    .await
    .map_err(ApiError::internal)?;

    map_target_with_history_row(row, history_rows)
}

fn map_board_row(row: sqlx::postgres::PgRow) -> Result<Board, ApiError> {
    let statuses: SqlJson<Vec<String>> = row.try_get("statuses").map_err(ApiError::internal)?;
    Ok(Board {
        id: row.try_get("id").map_err(ApiError::internal)?,
        name: row.try_get("name").map_err(ApiError::internal)?,
        statuses: statuses.0,
    })
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

fn map_target_with_history_row(
    row: sqlx::postgres::PgRow,
    history_rows: Vec<sqlx::postgres::PgRow>,
) -> Result<Target, ApiError> {
    let longitude: f64 = row.try_get("longitude").map_err(ApiError::internal)?;
    let latitude: f64 = row.try_get("latitude").map_err(ApiError::internal)?;
    let labels: SqlJson<Vec<String>> = row.try_get("labels").map_err(ApiError::internal)?;

    let mut history = Vec::with_capacity(history_rows.len());
    for history_row in history_rows {
        let from_status = history_row
            .try_get::<Option<String>, _>("from_status")
            .map_err(ApiError::internal)?
            .map(|value| parse_status(&value))
            .transpose()?;
        let to_status_raw: String = history_row
            .try_get("to_status")
            .map_err(ApiError::internal)?;
        history.push(TargetStateTransition {
            from: from_status,
            to: parse_status(&to_status_raw)?,
            at: history_row
                .try_get::<DateTime<Utc>, _>("transitioned_at")
                .map_err(ApiError::internal)?,
            by: history_row.try_get("actor").map_err(ApiError::internal)?,
        });
    }

    let status_raw: String = row.try_get("status").map_err(ApiError::internal)?;
    Ok(Target {
        id: row.try_get("id").map_err(ApiError::internal)?,
        board_id: row.try_get("board_id").map_err(ApiError::internal)?,
        title: row.try_get("title").map_err(ApiError::internal)?,
        status: parse_status(&status_raw)?,
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
        state_history: history,
    })
}

fn parse_status(value: &str) -> Result<TargetStatus, ApiError> {
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

fn serialize_status(status: &TargetStatus) -> &'static str {
    match status {
        TargetStatus::Nominated => "NOMINATED",
        TargetStatus::Triaged => "TRIAGED",
        TargetStatus::PendingPairing => "PENDING_PAIRING",
        TargetStatus::Paired => "PAIRED",
        TargetStatus::PlanDrafted => "PLAN_DRAFTED",
        TargetStatus::PendingApproval => "PENDING_APPROVAL",
        TargetStatus::Approved => "APPROVED",
        TargetStatus::InExecution => "IN_EXECUTION",
        TargetStatus::PendingBda => "PENDING_BDA",
        TargetStatus::AssessedComplete => "ASSESSED_COMPLETE",
        TargetStatus::Rejected => "REJECTED",
        TargetStatus::Archived => "ARCHIVED",
    }
}

fn default_target_title(detection: &Detection) -> String {
    match detection.classification.as_deref() {
        Some(classification) => format!("Detection {}", classification),
        None => format!("Detection {}", detection.id),
    }
}

fn validate_transition(from: &TargetStatus, to: &TargetStatus) -> Result<(), ApiError> {
    let allowed = match from {
        TargetStatus::Nominated => &[TargetStatus::Triaged, TargetStatus::Rejected][..],
        TargetStatus::Triaged => &[TargetStatus::PendingPairing, TargetStatus::Rejected],
        TargetStatus::PendingPairing => &[TargetStatus::Paired, TargetStatus::Rejected],
        TargetStatus::Paired => &[TargetStatus::PlanDrafted, TargetStatus::Rejected],
        TargetStatus::PlanDrafted => &[TargetStatus::PendingApproval, TargetStatus::Rejected],
        TargetStatus::PendingApproval => &[TargetStatus::Approved, TargetStatus::Rejected],
        TargetStatus::Approved => &[TargetStatus::InExecution, TargetStatus::Rejected],
        TargetStatus::InExecution => &[TargetStatus::PendingBda, TargetStatus::Rejected],
        TargetStatus::PendingBda => &[TargetStatus::AssessedComplete, TargetStatus::Rejected],
        TargetStatus::AssessedComplete | TargetStatus::Rejected | TargetStatus::Archived => &[],
    };

    if allowed.contains(to) {
        Ok(())
    } else {
        Err(ApiError::bad_request(format!(
            "invalid transition from {from:?} to {to:?}"
        )))
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

    fn bad_request(message: String) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
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
