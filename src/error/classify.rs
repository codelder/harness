use http::StatusCode;
use rig::completion::{CompletionError, PromptError};
use rig::http_client::Error as HttpError;
use std::time::Duration;
use thiserror::Error;

/// Default retry delay for rate limiting
const DEFAULT_RETRY_DELAY: Duration = Duration::from_secs(60);

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

/// Classify a rig PromptError into an appropriate AgentError
///
/// This function inspects the structured error from rig-core and maps it
/// to the appropriate AgentError variant based on:
/// - HTTP status codes (429 -> RateLimited, 401/403 -> Auth, etc.)
/// - Error types (network timeouts -> Network)
/// - Response content (rate limit messages, context limits, etc.)
///
/// # Arguments
/// * `err` - The PromptError from rig-core
///
/// # Returns
/// An appropriately classified AgentError
pub fn classify_prompt_error(err: PromptError) -> AgentError {
    match err {
        PromptError::CompletionError(completion_err) => {
            classify_completion_error(completion_err)
        }
        PromptError::ToolError(e) => {
            AgentError::Provider(ProviderError::RequestFailed(format!("Tool error: {}", e)))
        }
        PromptError::ToolServerError(e) => {
            AgentError::Provider(ProviderError::RequestFailed(format!(
                "Tool server error: {}",
                e
            )))
        }
        PromptError::MaxTurnsError { max_turns, .. } => {
            AgentError::Provider(ProviderError::RequestFailed(format!(
                "Max turns exceeded: {}",
                max_turns
            )))
        }
        PromptError::PromptCancelled { reason, .. } => {
            AgentError::Provider(ProviderError::RequestFailed(format!(
                "Prompt cancelled: {}",
                reason
            )))
        }
    }
}

/// Classify a CompletionError from rig-core
fn classify_completion_error(err: CompletionError) -> AgentError {
    match err {
        CompletionError::HttpError(http_err) => classify_http_error(http_err),
        CompletionError::ProviderError(msg) => {
            // Provider errors might contain useful info in the message
            classify_provider_message(&msg).unwrap_or_else(|| {
                AgentError::Provider(ProviderError::RequestFailed(msg))
            })
        }
        CompletionError::ResponseError(msg) => {
            // Response errors from API (e.g., Anthropic error response)
            classify_response_message(&msg).unwrap_or_else(|| {
                AgentError::Provider(ProviderError::RequestFailed(msg))
            })
        }
        CompletionError::RequestError(e) => {
            AgentError::Provider(ProviderError::RequestFailed(e.to_string()))
        }
        CompletionError::JsonError(e) => {
            AgentError::InvalidRequest(format!("JSON error: {}", e))
        }
        CompletionError::UrlError(e) => {
            AgentError::InvalidRequest(format!("URL error: {}", e))
        }
    }
}

/// Classify an HTTP error from rig-core's http_client
fn classify_http_error(err: HttpError) -> AgentError {
    match err {
        HttpError::InvalidStatusCodeWithMessage(status, body) => {
            classify_status_code(status, &body)
        }
        HttpError::Instance(e) => {
            // Underlying reqwest error (timeout, connection failed, etc.)
            let err_str = e.to_string();
            if is_timeout_error(&err_str) {
                AgentError::Network(err_str)
            } else {
                AgentError::Network(err_str)
            }
        }
        HttpError::Protocol(e) => {
            AgentError::Network(format!("HTTP protocol error: {}", e))
        }
        _ => AgentError::Network(err.to_string()),
    }
}

/// Classify based on HTTP status code
fn classify_status_code(status: StatusCode, body: &str) -> AgentError {
    match status {
        // 408 Request Timeout - retryable
        StatusCode::REQUEST_TIMEOUT => AgentError::Network(format!(
            "Request timeout (408): {}",
            truncate_error_body(body)
        )),

        // 429 Too Many Requests - retryable with backoff
        StatusCode::TOO_MANY_REQUESTS => {
            let retry_after = extract_retry_after(body).unwrap_or(DEFAULT_RETRY_DELAY);
            AgentError::RateLimited(retry_after)
        }

        // 500 Internal Server Error - retryable
        StatusCode::INTERNAL_SERVER_ERROR => AgentError::Network(format!(
            "Server error (500): {}",
            truncate_error_body(body)
        )),

        // 502 Bad Gateway - retryable
        StatusCode::BAD_GATEWAY => AgentError::Network(format!(
            "Bad gateway (502): {}",
            truncate_error_body(body)
        )),

        // 503 Service Unavailable - retryable
        StatusCode::SERVICE_UNAVAILABLE => AgentError::Network(format!(
            "Service unavailable (503): {}",
            truncate_error_body(body)
        )),

        // 504 Gateway Timeout - retryable
        StatusCode::GATEWAY_TIMEOUT => AgentError::Network(format!(
            "Gateway timeout (504): {}",
            truncate_error_body(body)
        )),

        // 401 Unauthorized - not retryable
        StatusCode::UNAUTHORIZED => AgentError::Auth(format!(
            "Unauthorized (401): {}",
            truncate_error_body(body)
        )),

        // 403 Forbidden - not retryable
        StatusCode::FORBIDDEN => AgentError::Auth(format!(
            "Forbidden (403): {}",
            truncate_error_body(body)
        )),

        // 400 Bad Request - check for context limit first
        StatusCode::BAD_REQUEST => {
            // Many providers report context/token limit as 400 with specific message
            if let Some(classified) = classify_context_limit(body) {
                classified
            } else {
                AgentError::InvalidRequest(format!(
                    "Bad request (400): {}",
                    truncate_error_body(body)
                ))
            }
        }

        // 404 Not Found - not retryable
        StatusCode::NOT_FOUND => AgentError::InvalidRequest(format!(
            "Not found (404): {}",
            truncate_error_body(body)
        )),

        // 413 Payload Too Large - context limit
        StatusCode::PAYLOAD_TOO_LARGE => AgentError::ContextLimit,

        // Other client errors (4xx) - not retryable
        code if code.is_client_error() => AgentError::Provider(ProviderError::RequestFailed(
            format!("Client error ({}): {}", code, truncate_error_body(body)),
        )),

        // Other server errors (5xx) - retryable
        code if code.is_server_error() => AgentError::Network(format!(
            "Server error ({}): {}",
            code,
            truncate_error_body(body)
        )),

        // Unknown status codes
        _ => AgentError::Provider(ProviderError::RequestFailed(format!(
            "HTTP {}: {}",
            status,
            truncate_error_body(body)
        ))),
    }
}

/// Check if an error string indicates a context limit (for 400 responses)
fn classify_context_limit(body: &str) -> Option<AgentError> {
    let lower = body.to_lowercase();

    // Common context limit indicators in provider error messages
    if lower.contains("context_length_exceeded")
        || lower.contains("context length")
        || lower.contains("token limit")
        || lower.contains("maximum context")
        || lower.contains("prompt is too long")
        || lower.contains("max_tokens")
        || lower.contains("input length")
    {
        return Some(AgentError::ContextLimit);
    }

    None
}

/// Check if an error string indicates a timeout
fn is_timeout_error(err_str: &str) -> bool {
    let lower = err_str.to_lowercase();
    lower.contains("timeout") || lower.contains("timed out")
}

/// Try to classify based on provider error message content
fn classify_provider_message(msg: &str) -> Option<AgentError> {
    let lower = msg.to_lowercase();

    // Check for rate limit indicators
    if lower.contains("rate limit")
        || lower.contains("too many requests")
        || lower.contains("429")
    {
        return Some(AgentError::RateLimited(DEFAULT_RETRY_DELAY));
    }

    // Check for context limit indicators
    if lower.contains("context length")
        || lower.contains("token limit")
        || lower.contains("maximum context")
        || lower.contains("prompt is too long")
    {
        return Some(AgentError::ContextLimit);
    }

    // Check for auth indicators
    if lower.contains("invalid api key")
        || lower.contains("unauthorized")
        || lower.contains("authentication")
    {
        return Some(AgentError::Auth(msg.to_string()));
    }

    None
}

/// Try to classify based on API response message content
fn classify_response_message(msg: &str) -> Option<AgentError> {
    let lower = msg.to_lowercase();

    // Anthropic-specific error types
    if lower.contains("rate_limit_error") || lower.contains("overloaded_error") {
        return Some(AgentError::RateLimited(DEFAULT_RETRY_DELAY));
    }

    if lower.contains("context_length_exceeded") {
        return Some(AgentError::ContextLimit);
    }

    if lower.contains("authentication_error") || lower.contains("permission_error") {
        return Some(AgentError::Auth(msg.to_string()));
    }

    if lower.contains("invalid_request_error") {
        return Some(AgentError::InvalidRequest(msg.to_string()));
    }

    // Generic patterns
    classify_provider_message(msg)
}

/// Extract retry-after duration from error response body
fn extract_retry_after(body: &str) -> Option<Duration> {
    // Try to parse JSON for retry_after field
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(body) {
        // Check nested error.retry_after: {"error": {"retry_after": 60}}
        if let Some(seconds) = json
            .get("error")
            .and_then(|e| e.get("retry_after"))
            .and_then(|v| v.as_u64())
        {
            return Some(Duration::from_secs(seconds));
        }
        // Check top-level retry_after: {"retry_after": 30}
        if let Some(seconds) = json.get("retry_after").and_then(|v| v.as_u64()) {
            return Some(Duration::from_secs(seconds));
        }
    }

    // Try to find seconds in plain text
    let lower = body.to_lowercase();
    if lower.contains("retry after") {
        // Try to extract number
        for part in lower.split("retry after") {
            if let Some(seconds) = extract_number_from_string(part) {
                return Some(Duration::from_secs(seconds));
            }
        }
    }

    None
}

/// Extract first number from a string
fn extract_number_from_string(s: &str) -> Option<u64> {
    s.chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(|c| c.is_ascii_digit())
        .collect::<String>()
        .parse()
        .ok()
}

/// Truncate error body for display (UTF-8 safe)
fn truncate_error_body(body: &str) -> &str {
    // Limit to first 200 chars for readability, using character boundaries
    match body.char_indices().nth(200) {
        Some((idx, _)) => &body[..idx],
        None => body,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::StatusCode;
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

    // ================================================================
    // classify_status_code tests
    // ================================================================

    #[test]
    fn test_classify_status_429_rate_limit() {
        let body = r#"{"error": {"type": "rate_limit_error", "message": "Too many requests"}}"#;
        let result = classify_status_code(StatusCode::TOO_MANY_REQUESTS, body);
        assert!(matches!(result, AgentError::RateLimited(_)));
        assert!(result.is_retryable());
    }

    #[test]
    fn test_classify_status_408_timeout() {
        let body = "Request timeout";
        let result = classify_status_code(StatusCode::REQUEST_TIMEOUT, body);
        assert!(matches!(result, AgentError::Network(_)));
        assert!(result.is_retryable());
    }

    #[test]
    fn test_classify_status_500_server_error() {
        let body = "Internal server error";
        let result = classify_status_code(StatusCode::INTERNAL_SERVER_ERROR, body);
        assert!(matches!(result, AgentError::Network(_)));
        assert!(result.is_retryable());
    }

    #[test]
    fn test_classify_status_502_bad_gateway() {
        let body = "Bad gateway";
        let result = classify_status_code(StatusCode::BAD_GATEWAY, body);
        assert!(matches!(result, AgentError::Network(_)));
        assert!(result.is_retryable());
    }

    #[test]
    fn test_classify_status_503_unavailable() {
        let body = "Service unavailable";
        let result = classify_status_code(StatusCode::SERVICE_UNAVAILABLE, body);
        assert!(matches!(result, AgentError::Network(_)));
        assert!(result.is_retryable());
    }

    #[test]
    fn test_classify_status_504_gateway_timeout() {
        let body = "Gateway timeout";
        let result = classify_status_code(StatusCode::GATEWAY_TIMEOUT, body);
        assert!(matches!(result, AgentError::Network(_)));
        assert!(result.is_retryable());
    }

    #[test]
    fn test_classify_status_401_unauthorized() {
        let body = "Invalid API key";
        let result = classify_status_code(StatusCode::UNAUTHORIZED, body);
        assert!(matches!(result, AgentError::Auth(_)));
        assert!(!result.is_retryable());
    }

    #[test]
    fn test_classify_status_403_forbidden() {
        let body = "Access denied";
        let result = classify_status_code(StatusCode::FORBIDDEN, body);
        assert!(matches!(result, AgentError::Auth(_)));
        assert!(!result.is_retryable());
    }

    #[test]
    fn test_classify_status_400_bad_request() {
        let body = "Invalid parameter";
        let result = classify_status_code(StatusCode::BAD_REQUEST, body);
        assert!(matches!(result, AgentError::InvalidRequest(_)));
        assert!(!result.is_retryable());
    }

    #[test]
    fn test_classify_status_400_context_limit() {
        // Anthropic-style context limit error (reported as 400)
        let body = r#"{"error": {"type": "context_length_exceeded", "message": "prompt is too long"}}"#;
        let result = classify_status_code(StatusCode::BAD_REQUEST, body);
        assert!(matches!(result, AgentError::ContextLimit));
        assert!(result.is_context_limit());
        assert!(!result.is_retryable());
    }

    #[test]
    fn test_classify_status_400_token_limit() {
        // OpenAI-style token limit error (reported as 400)
        let body = "This model's maximum context length is 4096 tokens";
        let result = classify_status_code(StatusCode::BAD_REQUEST, body);
        assert!(matches!(result, AgentError::ContextLimit));
    }

    #[test]
    fn test_classify_status_404_not_found() {
        let body = "Model not found";
        let result = classify_status_code(StatusCode::NOT_FOUND, body);
        assert!(matches!(result, AgentError::InvalidRequest(_)));
        assert!(!result.is_retryable());
    }

    #[test]
    fn test_classify_status_413_payload_too_large() {
        let body = "Request too large";
        let result = classify_status_code(StatusCode::PAYLOAD_TOO_LARGE, body);
        assert!(matches!(result, AgentError::ContextLimit));
        assert!(!result.is_retryable());
    }

    // ================================================================
    // extract_retry_after tests
    // ================================================================

    #[test]
    fn test_extract_retry_after_json() {
        let body = r#"{"retry_after": 30}"#;
        let result = extract_retry_after(body);
        assert_eq!(result, Some(Duration::from_secs(30)));
    }

    #[test]
    fn test_extract_retry_after_nested_json() {
        let body = r#"{"error": {"retry_after": 60}}"#;
        let result = extract_retry_after(body);
        // Now correctly handles nested error.retry_after
        assert_eq!(result, Some(Duration::from_secs(60)));
    }

    #[test]
    fn test_extract_retry_after_plain_text() {
        let body = "Please retry after 45 seconds";
        let result = extract_retry_after(body);
        assert_eq!(result, Some(Duration::from_secs(45)));
    }

    #[test]
    fn test_extract_retry_after_no_match() {
        let body = "Rate limit exceeded";
        let result = extract_retry_after(body);
        assert!(result.is_none());
    }

    // ================================================================
    // classify_provider_message tests
    // ================================================================

    #[test]
    fn test_classify_provider_rate_limit() {
        let msg = "Rate limit exceeded. Please try again later.";
        let result = classify_provider_message(msg);
        assert!(matches!(result, Some(AgentError::RateLimited(_))));
    }

    #[test]
    fn test_classify_provider_context_limit() {
        let msg = "context length exceeded maximum";
        let result = classify_provider_message(msg);
        assert!(matches!(result, Some(AgentError::ContextLimit)));
    }

    #[test]
    fn test_classify_provider_auth_error() {
        let msg = "Invalid API key provided";
        let result = classify_provider_message(msg);
        assert!(matches!(result, Some(AgentError::Auth(_))));
    }

    #[test]
    fn test_classify_provider_unknown() {
        let msg = "Some random error message";
        let result = classify_provider_message(msg);
        assert!(result.is_none());
    }

    // ================================================================
    // classify_response_message tests (API-specific errors)
    // ================================================================

    #[test]
    fn test_classify_anthropic_rate_limit() {
        let msg = r#"{"type": "error", "error": {"type": "rate_limit_error"}}"#;
        let result = classify_response_message(msg);
        assert!(matches!(result, Some(AgentError::RateLimited(_))));
    }

    #[test]
    fn test_classify_anthropic_overloaded() {
        let msg = r#"{"type": "error", "error": {"type": "overloaded_error"}}"#;
        let result = classify_response_message(msg);
        assert!(matches!(result, Some(AgentError::RateLimited(_))));
    }

    #[test]
    fn test_classify_anthropic_context_exceeded() {
        let msg = r#"{"type": "error", "error": {"type": "context_length_exceeded"}}"#;
        let result = classify_response_message(msg);
        assert!(matches!(result, Some(AgentError::ContextLimit)));
    }

    #[test]
    fn test_classify_anthropic_auth_error() {
        let msg = r#"{"type": "error", "error": {"type": "authentication_error"}}"#;
        let result = classify_response_message(msg);
        assert!(matches!(result, Some(AgentError::Auth(_))));
    }

    // ================================================================
    // truncate_error_body tests
    // ================================================================

    #[test]
    fn test_truncate_short_body() {
        let body = "short error";
        let result = truncate_error_body(body);
        assert_eq!(result, "short error");
    }

    #[test]
    fn test_truncate_long_body() {
        let body = "x".repeat(300);
        let result = truncate_error_body(&body);
        assert_eq!(result.chars().count(), 200);
    }

    #[test]
    fn test_truncate_multibyte_utf8() {
        // Test UTF-8 safe truncation with multibyte characters
        // Each emoji is 4 bytes, Chinese chars are 3 bytes
        let body = "错误信息：".to_string() + &"🔥".repeat(100);
        let result = truncate_error_body(&body);
        // Should not panic and should be valid UTF-8
        assert!(result.is_char_boundary(result.len()));
        assert!(result.chars().count() <= 200);
    }

    #[test]
    fn test_truncate_exactly_200_chars() {
        let body = "x".repeat(200);
        let result = truncate_error_body(&body);
        assert_eq!(result.len(), 200);
    }

    // ================================================================
    // is_timeout_error tests
    // ================================================================

    #[test]
    fn test_is_timeout_timeout() {
        assert!(is_timeout_error("connection timeout"));
    }

    #[test]
    fn test_is_timeout_timed_out() {
        assert!(is_timeout_error("request timed out after 30s"));
    }

    #[test]
    fn test_is_timeout_not_timeout() {
        assert!(!is_timeout_error("connection refused"));
    }
}
