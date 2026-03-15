use anyhow::{Context, Result};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use chrono::Utc;
use config::AppConfig;
use domain_models::{ApprovalStatus, TargetStatus, Task, TaskStatus};
use event_contracts::{EventEnvelope, TaskExecuted};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row, postgres::PgPoolOptions, types::Json as SqlJson};
use tower_http::{cors::CorsLayer, trace::TraceLayer};

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
struct ExecutionActionRequest {
    pub actor: String,
    pub notes: Option<String>,
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
        .with_context(|| "failed to connect execution-adapter to postgres")?;

    bootstrap_database(&db).await?;

    let state = AppState { db };
    let app = Router::new()
        .route("/health", get(health))
        .route("/tasks/{id}", get(get_task))
        .route("/tasks/{id}/dispatch", post(dispatch_task))
        .route("/tasks/{id}/complete", post(complete_task))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    tracing::info!(address = %addr, "execution-adapter listening");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn bootstrap_database(db: &PgPool) -> Result<()> {
    sqlx::raw_sql(include_str!("../../../infra/db/migrations/0001_init.sql"))
        .execute(db)
        .await
        .with_context(|| "failed to execute execution bootstrap schema")?;
    Ok(())
}

async fn health(State(state): State<AppState>) -> Result<Json<HealthResponse>, ApiError> {
    sqlx::query("select 1")
        .execute(&state.db)
        .await
        .map_err(ApiError::internal)?;

    Ok(Json(HealthResponse {
        service: "execution-adapter",
        status: "ok",
    }))
}

async fn get_task(
    Path(id): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<Task>, ApiError> {
    Ok(Json(load_task(&state.db, &id).await?))
}

async fn dispatch_task(
    Path(id): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<ExecutionActionRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let task = update_execution_state(
        &state.db,
        &id,
        TaskStatus::InExecution,
        TargetStatus::InExecution,
        "DISPATCHED",
        &request.actor,
        request.notes,
    )
    .await?;

    let event = EventEnvelope::new(
        "TaskExecuted",
        "execution-adapter",
        TaskExecuted { task: task.clone() },
    );

    Ok(Json(EventResponse {
        resource: task,
        event: serde_json::to_value(event).map_err(ApiError::internal)?,
    }))
}

async fn complete_task(
    Path(id): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<ExecutionActionRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let task = update_execution_state(
        &state.db,
        &id,
        TaskStatus::Completed,
        TargetStatus::PendingBda,
        "COMPLETED",
        &request.actor,
        request.notes,
    )
    .await?;

    Ok(Json(EventResponse {
        resource: task,
        event: serde_json::json!({
            "event_type": "TaskCompleted",
            "producer": "execution-adapter",
            "actor": request.actor,
        }),
    }))
}

async fn update_execution_state(
    db: &PgPool,
    task_id: &str,
    task_status: TaskStatus,
    target_status: TargetStatus,
    execution_status: &str,
    actor: &str,
    notes: Option<String>,
) -> Result<Task, ApiError> {
    let mut tx = db.begin().await.map_err(ApiError::internal)?;
    let task = load_task_in_tx(&mut tx, task_id).await?;

    if task.approval_status != ApprovalStatus::Approved {
        return Err(ApiError::bad_request(
            "task must be approved before execution actions".to_string(),
        ));
    }

    sqlx::query(
        r#"
        update tasks
        set status = $2,
            updated_at = $3
        where id = $1
        "#,
    )
    .bind(task_id)
    .bind(serialize_task_status(&task_status))
    .bind(Utc::now())
    .execute(&mut *tx)
    .await
    .map_err(ApiError::internal)?;

    sqlx::query(
        r#"
        insert into task_execution_updates (task_id, execution_status, actor, notes)
        values ($1, $2, $3, $4)
        "#,
    )
    .bind(task_id)
    .bind(execution_status)
    .bind(actor)
    .bind(notes)
    .execute(&mut *tx)
    .await
    .map_err(ApiError::internal)?;

    update_target_status(&mut tx, &task.target_id, target_status, actor).await?;
    tx.commit().await.map_err(ApiError::internal)?;

    load_task(db, task_id).await
}

async fn update_target_status(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    target_id: &str,
    new_status: TargetStatus,
    actor: &str,
) -> Result<(), ApiError> {
    let current_status: String = sqlx::query_scalar("select status from targets where id = $1")
        .bind(target_id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found(format!("target {target_id} was not found")))?;

    sqlx::query(
        r#"
        update targets
        set status = $2,
            updated_at = $3
        where id = $1
        "#,
    )
    .bind(target_id)
    .bind(serialize_target_status(&new_status))
    .bind(Utc::now())
    .execute(&mut **tx)
    .await
    .map_err(ApiError::internal)?;

    sqlx::query(
        r#"
        insert into target_state_history (target_id, from_status, to_status, actor, transitioned_at)
        values ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(target_id)
    .bind(Some(current_status))
    .bind(serialize_target_status(&new_status))
    .bind(actor)
    .bind(Utc::now())
    .execute(&mut **tx)
    .await
    .map_err(ApiError::internal)?;

    Ok(())
}

async fn load_task(db: &PgPool, task_id: &str) -> Result<Task, ApiError> {
    let row = sqlx::query(
        r#"
        select id, target_id, asset_ids, task_type, effect_type, status, approval_status, time_on_target
        from tasks
        where id = $1
        "#,
    )
    .bind(task_id)
    .fetch_optional(db)
    .await
    .map_err(ApiError::internal)?;

    let row = row.ok_or_else(|| ApiError::not_found(format!("task {task_id} was not found")))?;
    map_task_row(row)
}

async fn load_task_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    task_id: &str,
) -> Result<Task, ApiError> {
    let row = sqlx::query(
        r#"
        select id, target_id, asset_ids, task_type, effect_type, status, approval_status, time_on_target
        from tasks
        where id = $1
        "#,
    )
    .bind(task_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(ApiError::internal)?;

    let row = row.ok_or_else(|| ApiError::not_found(format!("task {task_id} was not found")))?;
    map_task_row(row)
}

fn map_task_row(row: sqlx::postgres::PgRow) -> Result<Task, ApiError> {
    let asset_ids: SqlJson<Vec<String>> = row.try_get("asset_ids").map_err(ApiError::internal)?;
    let status: String = row.try_get("status").map_err(ApiError::internal)?;
    let approval_status: String = row.try_get("approval_status").map_err(ApiError::internal)?;
    Ok(Task {
        id: row.try_get("id").map_err(ApiError::internal)?,
        target_id: row.try_get("target_id").map_err(ApiError::internal)?,
        asset_ids: asset_ids.0,
        task_type: row.try_get("task_type").map_err(ApiError::internal)?,
        effect_type: row.try_get("effect_type").map_err(ApiError::internal)?,
        status: parse_task_status(&status)?,
        approval_status: parse_approval_status(&approval_status)?,
        time_on_target: row.try_get("time_on_target").map_err(ApiError::internal)?,
    })
}

fn serialize_task_status(value: &TaskStatus) -> &'static str {
    match value {
        TaskStatus::Draft => "DRAFT",
        TaskStatus::PendingApproval => "PENDING_APPROVAL",
        TaskStatus::Approved => "APPROVED",
        TaskStatus::InExecution => "IN_EXECUTION",
        TaskStatus::Completed => "COMPLETED",
        TaskStatus::Cancelled => "CANCELLED",
    }
}

fn parse_task_status(value: &str) -> Result<TaskStatus, ApiError> {
    match value {
        "DRAFT" => Ok(TaskStatus::Draft),
        "PENDING_APPROVAL" => Ok(TaskStatus::PendingApproval),
        "APPROVED" => Ok(TaskStatus::Approved),
        "IN_EXECUTION" => Ok(TaskStatus::InExecution),
        "COMPLETED" => Ok(TaskStatus::Completed),
        "CANCELLED" => Ok(TaskStatus::Cancelled),
        _ => Err(ApiError::internal(format!("unknown task status: {value}"))),
    }
}

fn parse_approval_status(value: &str) -> Result<ApprovalStatus, ApiError> {
    match value {
        "NOT_REQUIRED" => Ok(ApprovalStatus::NotRequired),
        "REQUIRED" => Ok(ApprovalStatus::Required),
        "APPROVED" => Ok(ApprovalStatus::Approved),
        "REJECTED" => Ok(ApprovalStatus::Rejected),
        _ => Err(ApiError::internal(format!(
            "unknown approval status: {value}"
        ))),
    }
}

fn serialize_target_status(value: &TargetStatus) -> &'static str {
    match value {
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
