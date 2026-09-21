pub mod agent;
pub mod defense;
pub mod events;
pub mod execution;
pub mod gateway;
pub mod heartbeat;
pub mod inference;
pub mod mojo_bridge;
pub mod orchestration;
pub mod persistence;
pub mod security;
pub mod sessions;
pub mod skills;
pub mod vault;

pub use agent::{AgentEngine, AgentExecutionResult, AgentStep};
pub use defense::{ContainmentLevel, DefenseReceipt, DoctrineDecision};
pub use events::{AegisEvent, EventBus, EventEnvelope, EventId, EventSubscriber};
pub use execution::ActionReceipt;
pub use gateway::{create_router, start_gateway, GatewayState, HealthResponse};
pub use heartbeat::{HeartbeatEngine, PulseReceipt};
pub use inference::{
    ChatStream, ChatTurnResponse, EmbeddedInferenceBackend, EmbeddedModel, HttpInferenceBackend,
    InferenceEngine, ToolCallFunction, ToolCallItem,
};
pub use mojo_bridge::MojoSimdBridge;
pub use orchestration::{Run, RunBudget, RunState, TerminationReason, Trigger};
pub use persistence::{CrumbRecord, Database, TaskRecord, TurnRecord};
pub use security::{SecurityError, WorkspaceCapability};
pub use sessions::{
    ActionId, ApprovalId, AssistantMessage, Message, MessageId, MessageStore, Role, RunId, Session,
    SessionBudget, SessionId, SessionStatus, SessionStore, SystemMessage, ToolResultMessage,
    UserMessage,
};
pub use skills::{SkillDefinition, SkillExecutionRequest, SkillExecutionResponse, SkillRegistry};
pub use vault::{VaultResolver, REDACTED_MARKER};
