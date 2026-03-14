use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Task {
    pub id: String,
    pub target_id: String,
    pub asset_ids: Vec<String>,
    pub task_type: String,
    pub effect_type: String,
    pub status: TaskStatus,
    pub approval_status: ApprovalStatus,
    pub time_on_target: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TaskStatus {
    Draft,
    PendingApproval,
    Approved,
    InExecution,
    Completed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ApprovalStatus {
    NotRequired,
    Required,
    Approved,
    Rejected,
}
