mod classify;

pub use classify::{AgentError, ProviderError, is_retryable};

/// Result type alias for agent operations
pub type AgentResult<T> = Result<T, AgentError>;

/// Result type alias for provider operations
pub type ProviderResult<T> = Result<T, ProviderError>;
