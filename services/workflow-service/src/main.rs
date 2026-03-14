use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use anyhow::Result;
use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use chrono::Utc;
use config::AppConfig;
use domain_models::{Board, PointGeometry, Target, TargetStateTransition, TargetStatus};
use event_contracts::{EventEnvelope, TargetNominated, TargetTransitioned};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use uuid::Uuid;

#[derive(Clone)]
struct AppState {
    store: Arc<RwLock<WorkflowStore>>,
}

#[derive(Debug, Default)]
struct WorkflowStore {
    boards: HashMap<String, Board>,
    targets: HashMap<String, Target>,
}

impl WorkflowStore {
    fn with_seed_data() -> Self {
        let board = Board {
            id: "dynamic-main".to_string(),
            name: "Dynamic Main".to_string(),
            statuses: vec![
                "DELIBERATE".to_string(),
                "PENDING_PAIRING".to_string(),
                "PAIRED".to_string(),
                "IN_EXECUTION".to_string(),
                "PENDING_BDA".to_string(),
                "COMPLETE".to_string(),
                "REJECTED_ARCHIVED".to_string(),
            ],
        };

        let mut boards = HashMap::new();
        boards.insert(board.id.clone(), board);

        Self {
            boards,
            targets: HashMap::new(),
        }
    }
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
    pub location: PointGeometry,
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

    let state = AppState {
        store: Arc::new(RwLock::new(WorkflowStore::with_seed_data())),
    };

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

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        service: "workflow-service",
        status: "ok",
    })
}

async fn create_target(
    State(state): State<AppState>,
    Json(request): Json<CreateTargetRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let board_id = request
        .board_id
        .unwrap_or_else(|| "dynamic-main".to_string());

    let mut store = state.store.write().await;
    ensure_board_exists(&store, &board_id)?;

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

    store.targets.insert(target.id.clone(), target.clone());

    Ok((StatusCode::CREATED, Json(target)))
}

async fn get_target(
    Path(id): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<Target>, ApiError> {
    let store = state.store.read().await;
    let target = store
        .targets
        .get(&id)
        .cloned()
        .ok_or_else(|| ApiError::not_found(format!("target {id} was not found")))?;
    Ok(Json(target))
}

async fn transition_target(
    Path(id): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<TransitionTargetRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let mut store = state.store.write().await;
    let target = store
        .targets
        .get_mut(&id)
        .ok_or_else(|| ApiError::not_found(format!("target {id} was not found")))?;

    let current = target.status.clone();
    validate_transition(&current, &request.to_status)?;

    let now = Utc::now();
    target.status = request.to_status.clone();
    target.updated_at = now;
    target.state_history.push(TargetStateTransition {
        from: Some(current.clone()),
        to: request.to_status.clone(),
        at: now,
        by: request.actor.clone(),
    });

    let event = EventEnvelope::new(
        "TargetTransitioned",
        "workflow-service",
        TargetTransitioned {
            target_id: target.id.clone(),
            from: current,
            to: request.to_status,
            actor: request.actor,
        },
    );

    Ok((
        StatusCode::OK,
        Json(EventResponse {
            resource: target.clone(),
            event: serde_json::to_value(event).map_err(ApiError::internal)?,
        }),
    ))
}

async fn list_boards(State(state): State<AppState>) -> Json<Vec<Board>> {
    let store = state.store.read().await;
    Json(store.boards.values().cloned().collect())
}

async fn get_board(
    Path(id): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<Board>, ApiError> {
    let store = state.store.read().await;
    let board = store
        .boards
        .get(&id)
        .cloned()
        .ok_or_else(|| ApiError::not_found(format!("board {id} was not found")))?;
    Ok(Json(board))
}

async fn get_board_targets(
    Path(id): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<BoardTargetsResponse>, ApiError> {
    let store = state.store.read().await;
    let board = store
        .boards
        .get(&id)
        .cloned()
        .ok_or_else(|| ApiError::not_found(format!("board {id} was not found")))?;

    let mut columns: BTreeMap<String, Vec<Target>> = board
        .statuses
        .iter()
        .map(|status| (status.clone(), Vec::new()))
        .collect();

    for target in store
        .targets
        .values()
        .filter(|target| target.board_id == id)
    {
        columns
            .entry(target.status.as_board_column().to_string())
            .or_default()
            .push(target.clone());
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
        .unwrap_or_else(|| "dynamic-main".to_string());

    let mut store = state.store.write().await;
    ensure_board_exists(&store, &board_id)?;

    let now = Utc::now();
    let target = Target {
        id: format!("tgt_{}", Uuid::new_v4().simple()),
        board_id,
        title: request
            .title
            .unwrap_or_else(|| format!("Detection {}", detection_id)),
        status: TargetStatus::Nominated,
        classification: request.classification,
        priority: 50,
        location: request.location,
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

    store.targets.insert(target.id.clone(), target.clone());

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

fn ensure_board_exists(store: &WorkflowStore, board_id: &str) -> Result<(), ApiError> {
    if store.boards.contains_key(board_id) {
        Ok(())
    } else {
        Err(ApiError::not_found(format!(
            "board {board_id} was not found"
        )))
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
