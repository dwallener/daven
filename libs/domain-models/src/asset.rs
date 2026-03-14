use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::geo::PointGeometry;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Asset {
    pub id: String,
    pub callsign: String,
    pub platform_type: String,
    pub domain: String,
    pub location: PointGeometry,
    pub availability: AssetAvailability,
    pub capabilities: Vec<String>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AssetAvailability {
    Available,
    Tasked,
    Unavailable,
}
