pub mod agent;
pub mod cli;
pub mod error;
pub mod frontend;
pub mod llm;
pub mod planning;
pub mod session;
pub mod subagent;
pub mod tools;

// Re-exports for convenience
pub use agent::{agent_loop, with_retry, Message, Role};
pub use cli::{Args, Provider, Session};
pub use error::{AgentError, ProviderError};
pub use frontend::{
    frontend_command_channel, frontend_event_channel, send_frontend_command, FrontendCommand,
    FrontendCommandReceiver, FrontendCommandSender, FrontendEvent, FrontendEventReceiver,
    FrontendEventSender, FrontendSessionSummary, FrontendSubagentToolUse, FrontendTodoItem,
    FrontendTodoStatus, SESSION_START_TURN_ID,
};
pub use llm::{create_parent_provider, create_provider, LlmProvider, ProviderType};
pub use session::{SessionRuntime, SessionRuntimeConfig, SessionRuntimeOutcome, SessionSummary};
pub use subagent::{
    PendingSubagentCall, SharedSubagentCallQueue, SubagentConfig, SubagentError, SubagentTool,
    TaskArgs,
};
pub use tools::BashTool;
