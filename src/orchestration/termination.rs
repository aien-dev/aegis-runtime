use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminationReason {
    Completed,
    StepLimitExceeded,
    InferenceLimitExceeded,
    TimeLimitExceeded,
    UserCancelled,
    DoctrineDenied,
    ApprovalRequired,
    InferenceFailure(String),
    ExecutionFailure(String),
    Suspended,
}
