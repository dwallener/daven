use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PointGeometry {
    #[serde(rename = "type")]
    pub geometry_type: String,
    pub coordinates: [f64; 2],
}

impl PointGeometry {
    pub fn point(longitude: f64, latitude: f64) -> Self {
        Self {
            geometry_type: "Point".to_string(),
            coordinates: [longitude, latitude],
        }
    }
}
