use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Board {
    pub id: String,
    pub name: String,
    pub statuses: Vec<String>,
}
