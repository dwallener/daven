use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Recommendation {
    pub id: String,
    pub target_id: String,
    pub generated_at: DateTime<Utc>,
    pub candidates: Vec<RecommendationCandidate>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RecommendationCandidate {
    pub asset_id: String,
    pub score: f32,
    pub rank: u32,
    pub explanation: serde_json::Value,
}
