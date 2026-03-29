mod classify;

pub use classify::{classify_prompt_error, is_retryable, AgentError, ProviderError};

/// Result type alias for agent operations
pub type AgentResult<T> = Result<T, AgentError>;

/// Result type alias for provider operations
pub type ProviderResult<T> = Result<T, ProviderError>;
