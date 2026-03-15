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
use domain_models::{ApprovalStatus, TargetStatus, Task, TaskStatus};
use event_contracts::{EventEnvelope, TaskApproved, TaskProposed};
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
struct ProposeTaskRequest {
    pub target_id: String,
    pub asset_ids: Vec<String>,
    pub task_type: String,
    pub effect_type: String,
    pub created_by: String,
    pub time_on_target: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
struct ApproveTaskRequest {
    pub actor: String,
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
        .with_context(|| "failed to connect planning-service to postgres")?;

    bootstrap_database(&db).await?;

    let state = AppState { db };
    let app = Router::new()
        .route("/health", get(health))
        .route("/tasks/propose", post(propose_task))
        .route("/tasks/{id}", get(get_task))
        .route("/tasks/targets/{target_id}", get(list_tasks_for_target))
        .route("/tasks/{id}/approve", post(approve_task))
        .route("/tasks/{id}/reject", post(reject_task))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    tracing::info!(address = %addr, "planning-service listening");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn bootstrap_database(db: &PgPool) -> Result<()> {
    sqlx::raw_sql(include_str!("../../../infra/db/migrations/0001_init.sql"))
        .execute(db)
        .await
        .with_context(|| "failed to execute planning bootstrap schema")?;
    Ok(())
}

async fn health(State(state): State<AppState>) -> Result<Json<HealthResponse>, ApiError> {
    sqlx::query("select 1")
        .execute(&state.db)
        .await
        .map_err(ApiError::internal)?;

    Ok(Json(HealthResponse {
        service: "planning-service",
        status: "ok",
    }))
}

async fn propose_task(
    State(state): State<AppState>,
    Json(request): Json<ProposeTaskRequest>,
) -> Result<impl IntoResponse, ApiError> {
    ensure_target_exists(&state.db, &request.target_id).await?;
    ensure_assets_exist(&state.db, &request.asset_ids).await?;

    let now = Utc::now();
    let task = Task {
        id: format!("task_{}", Uuid::new_v4().simple()),
        target_id: request.target_id.clone(),
        asset_ids: request.asset_ids,
        task_type: request.task_type,
        effect_type: request.effect_type,
        status: TaskStatus::PendingApproval,
        approval_status: ApprovalStatus::Required,
        time_on_target: request.time_on_target,
    };

    let mut tx = state.db.begin().await.map_err(ApiError::internal)?;
    sqlx::query(
        r#"
        insert into tasks (
            id, target_id, asset_ids, task_type, effect_type, status, approval_status,
            time_on_target, created_by, created_at, updated_at
        )
        values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
        "#,
    )
    .bind(&task.id)
    .bind(&task.target_id)
    .bind(SqlJson(task.asset_ids.clone()))
    .bind(&task.task_type)
    .bind(&task.effect_type)
    .bind(serialize_task_status(&task.status))
    .bind(serialize_approval_status(&task.approval_status))
    .bind(task.time_on_target)
    .bind(&request.created_by)
    .bind(now)
    .bind(now)
    .execute(&mut *tx)
    .await
    .map_err(ApiError::internal)?;

    update_target_status(
        &mut tx,
        &task.target_id,
        TargetStatus::PendingApproval,
        &request.created_by,
    )
    .await?;
    tx.commit().await.map_err(ApiError::internal)?;

    let event = EventEnvelope::new(
        "TaskProposed",
        "planning-service",
        TaskProposed { task: task.clone() },
    );

    Ok((
        StatusCode::CREATED,
        Json(EventResponse {
            resource: task,
            event: serde_json::to_value(event).map_err(ApiError::internal)?,
        }),
    ))
}

async fn get_task(
    Path(id): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<Task>, ApiError> {
    Ok(Json(load_task(&state.db, &id).await?))
}

async fn list_tasks_for_target(
    Path(target_id): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<Vec<Task>>, ApiError> {
    let rows = sqlx::query(
        r#"
        select id, target_id, asset_ids, task_type, effect_type, status, approval_status, time_on_target
        from tasks
        where target_id = $1
        order by created_at desc
        "#,
    )
    .bind(&target_id)
    .fetch_all(&state.db)
    .await
    .map_err(ApiError::internal)?;

    let tasks = rows
        .into_iter()
        .map(map_task_row)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Json(tasks))
}

async fn approve_task(
    Path(id): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<ApproveTaskRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let task = update_task_state(
        &state.db,
        &id,
        TaskStatus::Approved,
        ApprovalStatus::Approved,
        TargetStatus::Approved,
        &request.actor,
    )
    .await?;

    let event = EventEnvelope::new(
        "TaskApproved",
        "planning-service",
        TaskApproved {
            task: task.clone(),
            actor: request.actor,
        },
    );

    Ok(Json(EventResponse {
        resource: task,
        event: serde_json::to_value(event).map_err(ApiError::internal)?,
    }))
}

async fn reject_task(
    Path(id): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<ApproveTaskRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let task = update_task_state(
        &state.db,
        &id,
        TaskStatus::Cancelled,
        ApprovalStatus::Rejected,
        TargetStatus::Rejected,
        &request.actor,
    )
    .await?;

    Ok(Json(EventResponse {
        resource: task,
        event: serde_json::json!({
            "event_type": "TaskRejected",
            "producer": "planning-service",
            "actor": request.actor,
        }),
    }))
}

async fn update_task_state(
    db: &PgPool,
    task_id: &str,
    task_status: TaskStatus,
    approval_status: ApprovalStatus,
    target_status: TargetStatus,
    actor: &str,
) -> Result<Task, ApiError> {
    let mut tx = db.begin().await.map_err(ApiError::internal)?;
    let task = load_task_in_tx(&mut tx, task_id).await?;

    sqlx::query(
        r#"
        update tasks
        set status = $2,
            approval_status = $3,
            updated_at = $4
        where id = $1
        "#,
    )
    .bind(task_id)
    .bind(serialize_task_status(&task_status))
    .bind(serialize_approval_status(&approval_status))
    .bind(Utc::now())
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

async fn ensure_target_exists(db: &PgPool, target_id: &str) -> Result<(), ApiError> {
    let exists =
        sqlx::query_scalar::<_, bool>("select exists(select 1 from targets where id = $1)")
            .bind(target_id)
            .fetch_one(db)
            .await
            .map_err(ApiError::internal)?;
    if exists {
        Ok(())
    } else {
        Err(ApiError::not_found(format!(
            "target {target_id} was not found"
        )))
    }
}

async fn ensure_assets_exist(db: &PgPool, asset_ids: &[String]) -> Result<(), ApiError> {
    for asset_id in asset_ids {
        let exists =
            sqlx::query_scalar::<_, bool>("select exists(select 1 from assets where id = $1)")
                .bind(asset_id)
                .fetch_one(db)
                .await
                .map_err(ApiError::internal)?;
        if !exists {
            return Err(ApiError::not_found(format!(
                "asset {asset_id} was not found"
            )));
        }
    }
    Ok(())
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

fn serialize_approval_status(value: &ApprovalStatus) -> &'static str {
    match value {
        ApprovalStatus::NotRequired => "NOT_REQUIRED",
        ApprovalStatus::Required => "REQUIRED",
        ApprovalStatus::Approved => "APPROVED",
        ApprovalStatus::Rejected => "REJECTED",
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
