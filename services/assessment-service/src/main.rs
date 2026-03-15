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
use domain_models::{Assessment, AssessmentResult, TargetStatus, Task, TaskStatus};
use event_contracts::{AssessmentCreated, EventEnvelope};
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
struct CreateAssessmentRequest {
    pub result: AssessmentResult,
    pub confidence: f32,
    pub notes: Option<String>,
    pub media_refs: Option<Vec<String>>,
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
        .with_context(|| "failed to connect assessment-service to postgres")?;

    bootstrap_database(&db).await?;

    let state = AppState { db };
    let app = Router::new()
        .route("/health", get(health))
        .route("/assessments/{id}", get(get_assessment))
        .route("/tasks/{id}/assess", post(create_assessment))
        .route(
            "/targets/{id}/assessments",
            get(list_assessments_for_target),
        )
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    tracing::info!(address = %addr, "assessment-service listening");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn bootstrap_database(db: &PgPool) -> Result<()> {
    sqlx::raw_sql(include_str!("../../../infra/db/migrations/0001_init.sql"))
        .execute(db)
        .await
        .with_context(|| "failed to execute assessment bootstrap schema")?;
    Ok(())
}

async fn health(State(state): State<AppState>) -> Result<Json<HealthResponse>, ApiError> {
    sqlx::query("select 1")
        .execute(&state.db)
        .await
        .map_err(ApiError::internal)?;

    Ok(Json(HealthResponse {
        service: "assessment-service",
        status: "ok",
    }))
}

async fn get_assessment(
    Path(id): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<Assessment>, ApiError> {
    Ok(Json(load_assessment(&state.db, &id).await?))
}

async fn list_assessments_for_target(
    Path(id): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<Vec<Assessment>>, ApiError> {
    let rows = sqlx::query(
        r#"
        select id, task_id, target_id, result, confidence, assessed_by, notes, media_refs, created_at
        from assessments
        where target_id = $1
        order by created_at desc
        "#,
    )
    .bind(&id)
    .fetch_all(&state.db)
    .await
    .map_err(ApiError::internal)?;

    let assessments = rows
        .into_iter()
        .map(map_assessment_row)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(Json(assessments))
}

async fn create_assessment(
    Path(task_id): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<CreateAssessmentRequest>,
) -> Result<impl IntoResponse, ApiError> {
    if !(0.0..=1.0).contains(&request.confidence) {
        return Err(ApiError::bad_request(
            "confidence must be between 0.0 and 1.0".to_string(),
        ));
    }

    let mut tx = state.db.begin().await.map_err(ApiError::internal)?;
    let task = load_task_in_tx(&mut tx, &task_id).await?;
    let current_target_status = load_target_status_in_tx(&mut tx, &task.target_id).await?;

    if task.status != TaskStatus::Completed {
        return Err(ApiError::bad_request(
            "task must be completed before assessment".to_string(),
        ));
    }

    if current_target_status != TargetStatus::PendingBda {
        return Err(ApiError::bad_request(
            "target must be in PENDING_BDA before assessment".to_string(),
        ));
    }

    let now = Utc::now();
    let assessment = Assessment {
        id: format!("asmt_{}", Uuid::new_v4().simple()),
        task_id: task.id.clone(),
        target_id: task.target_id.clone(),
        result: request.result.clone(),
        confidence: request.confidence,
        assessed_by: request.actor.clone(),
        created_at: now,
        notes: request.notes,
        media_refs: request.media_refs.unwrap_or_default(),
    };

    sqlx::query(
        r#"
        insert into assessments (
            id, task_id, target_id, result, confidence, assessed_by, notes, media_refs, created_at
        )
        values ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        "#,
    )
    .bind(&assessment.id)
    .bind(&assessment.task_id)
    .bind(&assessment.target_id)
    .bind(serialize_assessment_result(&assessment.result))
    .bind(assessment.confidence)
    .bind(&assessment.assessed_by)
    .bind(&assessment.notes)
    .bind(SqlJson(assessment.media_refs.clone()))
    .bind(assessment.created_at)
    .execute(&mut *tx)
    .await
    .map_err(ApiError::internal)?;

    if assessment.result != AssessmentResult::Inconclusive {
        update_target_status(
            &mut tx,
            &assessment.target_id,
            current_target_status,
            TargetStatus::AssessedComplete,
            &assessment.assessed_by,
        )
        .await?;
    }

    tx.commit().await.map_err(ApiError::internal)?;

    let event = EventEnvelope::new(
        "AssessmentCreated",
        "assessment-service",
        AssessmentCreated {
            assessment: assessment.clone(),
        },
    );

    Ok((
        StatusCode::CREATED,
        Json(EventResponse {
            resource: assessment,
            event: serde_json::to_value(event).map_err(ApiError::internal)?,
        }),
    ))
}

async fn load_assessment(db: &PgPool, id: &str) -> Result<Assessment, ApiError> {
    let row = sqlx::query(
        r#"
        select id, task_id, target_id, result, confidence, assessed_by, notes, media_refs, created_at
        from assessments
        where id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(db)
    .await
    .map_err(ApiError::internal)?
    .ok_or_else(|| ApiError::not_found(format!("assessment {id} was not found")))?;

    map_assessment_row(row)
}

async fn load_task_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    id: &str,
) -> Result<Task, ApiError> {
    let row = sqlx::query(
        r#"
        select id, target_id, asset_ids, task_type, effect_type, status, approval_status, time_on_target
        from tasks
        where id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(ApiError::internal)?
    .ok_or_else(|| ApiError::not_found(format!("task {id} was not found")))?;

    map_task_row(row)
}

async fn load_target_status_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    target_id: &str,
) -> Result<TargetStatus, ApiError> {
    let value = sqlx::query_scalar::<_, String>("select status from targets where id = $1")
        .bind(target_id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found(format!("target {target_id} was not found")))?;

    parse_target_status(&value)
}

async fn update_target_status(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    target_id: &str,
    from: TargetStatus,
    to: TargetStatus,
    actor: &str,
) -> Result<(), ApiError> {
    let now = Utc::now();

    sqlx::query(
        r#"
        update targets
        set status = $2,
            updated_at = $3
        where id = $1
        "#,
    )
    .bind(target_id)
    .bind(serialize_target_status(&to))
    .bind(now)
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
    .bind(Some(serialize_target_status(&from)))
    .bind(serialize_target_status(&to))
    .bind(actor)
    .bind(now)
    .execute(&mut **tx)
    .await
    .map_err(ApiError::internal)?;

    Ok(())
}

fn map_assessment_row(row: sqlx::postgres::PgRow) -> Result<Assessment, ApiError> {
    let result: String = row.try_get("result").map_err(ApiError::internal)?;
    let media_refs: SqlJson<Vec<String>> = row.try_get("media_refs").map_err(ApiError::internal)?;

    Ok(Assessment {
        id: row.try_get("id").map_err(ApiError::internal)?,
        task_id: row.try_get("task_id").map_err(ApiError::internal)?,
        target_id: row.try_get("target_id").map_err(ApiError::internal)?,
        result: parse_assessment_result(&result)?,
        confidence: row.try_get("confidence").map_err(ApiError::internal)?,
        assessed_by: row.try_get("assessed_by").map_err(ApiError::internal)?,
        created_at: row.try_get("created_at").map_err(ApiError::internal)?,
        notes: row.try_get("notes").map_err(ApiError::internal)?,
        media_refs: media_refs.0,
    })
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

fn serialize_assessment_result(value: &AssessmentResult) -> &'static str {
    match value {
        AssessmentResult::Destroyed => "DESTROYED",
        AssessmentResult::Damaged => "DAMAGED",
        AssessmentResult::NoEffect => "NO_EFFECT",
        AssessmentResult::Inconclusive => "INCONCLUSIVE",
    }
}

fn parse_assessment_result(value: &str) -> Result<AssessmentResult, ApiError> {
    match value {
        "DESTROYED" => Ok(AssessmentResult::Destroyed),
        "DAMAGED" => Ok(AssessmentResult::Damaged),
        "NO_EFFECT" => Ok(AssessmentResult::NoEffect),
        "INCONCLUSIVE" => Ok(AssessmentResult::Inconclusive),
        _ => Err(ApiError::internal(format!(
            "unknown assessment result: {value}"
        ))),
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

fn parse_approval_status(value: &str) -> Result<domain_models::ApprovalStatus, ApiError> {
    match value {
        "NOT_REQUIRED" => Ok(domain_models::ApprovalStatus::NotRequired),
        "REQUIRED" => Ok(domain_models::ApprovalStatus::Required),
        "APPROVED" => Ok(domain_models::ApprovalStatus::Approved),
        "REJECTED" => Ok(domain_models::ApprovalStatus::Rejected),
        _ => Err(ApiError::internal(format!(
            "unknown approval status: {value}"
        ))),
    }
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
