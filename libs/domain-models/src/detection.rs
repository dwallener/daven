use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::geo::PointGeometry;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Detection {
    pub id: String,
    pub source_type: String,
    pub source_id: String,
    pub external_ref: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub geometry: PointGeometry,
    pub classification: Option<String>,
    pub confidence: Option<f32>,
}
