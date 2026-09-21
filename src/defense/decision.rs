use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContainmentLevel {
    Observe,
    Restrict,
    IsolateProcess,
    IsolateWorkspace,
    SuspendAgent,
    EmergencyStop,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", content = "details")]
pub enum DoctrineDecision {
    Allow,
    AllowRestricted(Vec<String>),
    RequireApproval,
    Deny(String),
    Contain(ContainmentLevel),
}
