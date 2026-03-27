pub mod error;
pub mod llm;
pub mod agent;
pub mod cli;
pub mod frontend;
pub mod session;
pub mod tools;
pub mod planning;
pub mod subagent;

// Re-exports for convenience
pub use error::{AgentError, ProviderError};
pub use llm::{create_provider, create_parent_provider, LlmProvider, ProviderType};
pub use agent::{agent_loop, with_retry, Message, Role};
pub use cli::{Args, Provider, Session};
pub use frontend::{
    frontend_command_channel,
    frontend_event_channel,
    FrontendCommand,
    FrontendCommandReceiver,
    FrontendCommandSender,
    FrontendEvent,
    FrontendEventReceiver,
    FrontendEventSender,
    FrontendSessionSummary,
    FrontendTodoItem,
    FrontendTodoStatus,
    SESSION_START_TURN_ID,
    send_frontend_command,
};
pub use session::{SessionRuntime, SessionRuntimeConfig, SessionRuntimeOutcome, SessionSummary};
pub use tools::BashTool;
pub use subagent::{SubagentTool, SubagentConfig, SubagentError, TaskArgs};
