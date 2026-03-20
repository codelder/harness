use std::time::Duration;
use thiserror::Error;

/// Errors that can occur during agent execution
#[derive(Debug, Error)]
pub enum AgentError {
    #[error("Network error: {0}")]
    Network(String),

    #[error("Rate limited: retry after {0:?}")]
    RateLimited(Duration),

    #[error("Authentication failed: {0}")]
    Auth(String),

    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("Context limit exceeded")]
    ContextLimit,

    #[error("Tools not implemented (Phase 2)")]
    ToolsNotImplemented,

    #[error("Provider error: {0}")]
    Provider(#[from] ProviderError),
}

impl AgentError {
    /// Returns true if this error is retryable with exponential backoff
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            AgentError::Network(_) | AgentError::RateLimited(_)
        )
    }

    /// Returns true if this error indicates context limit was reached
    pub fn is_context_limit(&self) -> bool {
        matches!(self, AgentError::ContextLimit)
    }
}

/// Errors specific to LLM provider operations
#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("Unknown provider: {0}")]
    UnknownProvider(String),

    #[error("Missing API key for {0}")]
    MissingApiKey(String),

    #[error("Provider request failed: {0}")]
    RequestFailed(String),

    #[error("Invalid model: {0}")]
    InvalidModel(String),
}

/// Standalone function to check if any error is retryable
pub fn is_retryable(error: &AgentError) -> bool {
    error.is_retryable()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_is_retryable_network() {
        let error = AgentError::Network("connection timeout".to_string());
        assert!(error.is_retryable());
        assert!(is_retryable(&error));
    }

    #[test]
    fn test_is_retryable_rate_limited() {
        let error = AgentError::RateLimited(Duration::from_secs(30));
        assert!(error.is_retryable());
    }

    #[test]
    fn test_not_retryable_auth() {
        let error = AgentError::Auth("invalid API key".to_string());
        assert!(!error.is_retryable());
    }

    #[test]
    fn test_not_retryable_invalid_request() {
        let error = AgentError::InvalidRequest("malformed request".to_string());
        assert!(!error.is_retryable());
    }

    #[test]
    fn test_not_retryable_context_limit() {
        let error = AgentError::ContextLimit;
        assert!(!error.is_retryable());
        assert!(error.is_context_limit());
    }

    #[test]
    fn test_provider_error_conversion() {
        let provider_err = ProviderError::UnknownProvider("test".to_string());
        let agent_err: AgentError = provider_err.into();
        matches!(agent_err, AgentError::Provider(_));
    }
}
