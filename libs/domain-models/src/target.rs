use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::geo::PointGeometry;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Target {
    pub id: String,
    pub board_id: String,
    pub title: String,
    pub status: TargetStatus,
    pub classification: Option<String>,
    pub priority: i32,
    pub location: PointGeometry,
    pub source_detection_id: Option<String>,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub labels: Vec<String>,
    pub state_history: Vec<TargetStateTransition>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TargetStateTransition {
    pub from: Option<TargetStatus>,
    pub to: TargetStatus,
    pub at: DateTime<Utc>,
    pub by: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TargetStatus {
    Nominated,
    Triaged,
    PendingPairing,
    Paired,
    PlanDrafted,
    PendingApproval,
    Approved,
    InExecution,
    PendingBda,
    AssessedComplete,
    Rejected,
    Archived,
}

impl TargetStatus {
    pub fn as_board_column(&self) -> &'static str {
        match self {
            Self::Nominated | Self::Triaged => "DELIBERATE",
            Self::PendingPairing => "PENDING_PAIRING",
            Self::Paired | Self::PlanDrafted | Self::PendingApproval => "PAIRED",
            Self::Approved | Self::InExecution => "IN_EXECUTION",
            Self::PendingBda => "PENDING_BDA",
            Self::AssessedComplete => "COMPLETE",
            Self::Rejected | Self::Archived => "REJECTED_ARCHIVED",
        }
    }
}
