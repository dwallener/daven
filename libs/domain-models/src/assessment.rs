use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Assessment {
    pub id: String,
    pub task_id: String,
    pub target_id: String,
    pub result: AssessmentResult,
    pub confidence: f32,
    pub created_at: DateTime<Utc>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AssessmentResult {
    Destroyed,
    Damaged,
    NoEffect,
    Inconclusive,
}
