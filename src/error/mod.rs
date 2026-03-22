mod classify;

pub use classify::{is_retryable, classify_prompt_error, AgentError, ProviderError};

/// Result type alias for agent operations
pub type AgentResult<T> = Result<T, AgentError>;

/// Result type alias for provider operations
pub type ProviderResult<T> = Result<T, ProviderError>;
