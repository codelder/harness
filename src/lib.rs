pub mod error;
pub mod llm;
pub mod agent;
pub mod cli;
pub mod frontend;
pub mod tools;
pub mod planning;

// Re-exports for convenience
pub use error::{AgentError, ProviderError};
pub use llm::{create_provider, LlmProvider, ProviderType};
pub use agent::{agent_loop, with_retry, Message, Role};
pub use cli::{Args, Provider, Session};
pub use frontend::{
    FrontendCommand,
    FrontendCommandReceiver,
    FrontendCommandSender,
    FrontendEvent,
    FrontendEventReceiver,
    FrontendEventSender,
    FrontendSessionSummary,
    send_frontend_command,
};
pub use tools::BashTool;
