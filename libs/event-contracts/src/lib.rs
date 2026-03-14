use chrono::{DateTime, Utc};
use domain_models::{Assessment, Asset, Detection, Recommendation, Target, TargetStatus, Task};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EventEnvelope<T> {
    pub event_id: Uuid,
    pub event_type: String,
    pub occurred_at: DateTime<Utc>,
    pub producer: String,
    pub payload: T,
}

impl<T> EventEnvelope<T> {
    pub fn new(event_type: impl Into<String>, producer: impl Into<String>, payload: T) -> Self {
        Self {
            event_id: Uuid::new_v4(),
            event_type: event_type.into(),
            occurred_at: Utc::now(),
            producer: producer.into(),
            payload,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DetectionCreated {
    pub detection: Detection,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AssetUpdated {
    pub asset: Asset,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TargetNominated {
    pub target: Target,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TargetTransitioned {
    pub target_id: String,
    pub from: TargetStatus,
    pub to: TargetStatus,
    pub actor: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RecommendationGenerated {
    pub recommendation: Recommendation,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TaskProposed {
    pub task: Task,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TaskApproved {
    pub task: Task,
    pub actor: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TaskExecuted {
    pub task: Task,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AssessmentCreated {
    pub assessment: Assessment,
}
