use super::{AgentTurn, Message, Role};
use crate::error::{classify_prompt_error, AgentError};
use std::time::Duration;
use tokio::time::sleep;

/// Maximum number of retries for retryable errors
const MAX_RETRIES: u32 = 3;

/// Initial delay for exponential backoff (1 second)
const INITIAL_DELAY: Duration = Duration::from_secs(1);

/// Execute an operation with automatic retry on retryable errors
///
/// Uses exponential backoff: 1s, 2s, 4s delays between retries
///
/// # Arguments
/// * `max_retries` - Maximum number of retry attempts
/// * `operation` - Async operation to execute
///
/// # Returns
/// The result of the operation if successful, or the final error if all retries exhausted
pub async fn with_retry<T, F, Fut>(
    max_retries: u32,
    mut operation: F,
) -> Result<T, AgentError>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, AgentError>>,
{
    let mut delay = INITIAL_DELAY;

    for attempt in 0..=max_retries {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) if e.is_retryable() && attempt < max_retries => {
                eprintln!("Retry {}/{}: {}", attempt + 1, max_retries, e);
                sleep(delay).await;
                delay *= 2; // Exponential backoff
            }
            Err(e) => return Err(e),
        }
    }

    // This should never be reached due to loop logic, but satisfies type checker
    Err(AgentError::Network("Max retries exceeded".to_string()))
}

/// Core agent loop that processes LLM responses until text output
///
/// This is the foundational agent pattern that transforms an LLM into an agent.
/// The tool loop is handled internally by rig-core's Agent when tools are configured.
///
/// # Arguments
/// * `history` - Conversation history (read-only, not modified)
/// * `current_input` - The user's current input
/// * `provider` - LLM provider to use for chat completions
///
/// # Returns
/// An `AgentTurn` containing the user input and agent response, or an error
///
/// # Errors
/// - AgentError on provider failures
///
/// # Note
/// The provider parameter uses LlmProvider enum instead of &dyn Chat because
/// rig-core's Chat trait is not object-safe (uses impl Trait in return types).
/// System messages are handled via agent preamble in rig, so we skip them here.
/// Tool calling is handled internally by rig-core's Agent with multi-turn support.
pub async fn agent_loop(
    history: &[Message],
    current_input: &str,
    provider: &crate::llm::LlmProvider,
) -> Result<AgentTurn, AgentError> {
    loop {
        // Convert history to rig's Message type for chat history
        // Note: chat_history should NOT include the current input,
        // because it will be passed separately as the prompt parameter
        let rig_messages: Vec<rig::completion::Message> = history
            .iter()
            .filter_map(|m| match m.role {
                Role::User => Some(rig::completion::Message::user(&m.content)),
                Role::Assistant => Some(rig::completion::Message::assistant(&m.content)),
                Role::System => {
                    // System messages are handled via agent preamble in rig
                    None
                }
            })
            .collect();

        // Call LLM with retry logic
        let response = with_retry(MAX_RETRIES, || async {
            provider
                .chat_with_history(current_input.to_string(), rig_messages.clone())
                .await
                .map_err(classify_prompt_error)
        })
        .await?;

        // Return the structured turn result
        return Ok(AgentTurn {
            user_input: current_input.to_string(),
            response,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    #[tokio::test]
    async fn test_retry_success_on_first_try() {
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let result = with_retry(3, move || {
            let counter = counter_clone.clone();
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                Ok::<_, AgentError>(42)
            }
        })
        .await;

        assert_eq!(result.unwrap(), 42);
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_retry_succeeds_after_failures() {
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let result = with_retry(3, move || {
            let counter = counter_clone.clone();
            async move {
                let attempt = counter.fetch_add(1, Ordering::SeqCst);
                if attempt < 2 {
                    Err(AgentError::Network("timeout".to_string()))
                } else {
                    Ok::<_, AgentError>(42)
                }
            }
        })
        .await;

        assert_eq!(result.unwrap(), 42);
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_retry_non_retryable_error_fails_immediately() {
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let result: Result<i32, AgentError> = with_retry(3, move || {
            let counter = counter_clone.clone();
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                Err(AgentError::Auth("invalid key".to_string()))
            }
        })
        .await;

        assert!(result.is_err());
        matches!(result.unwrap_err(), AgentError::Auth(_));
        assert_eq!(counter.load(Ordering::SeqCst), 1); // Only called once
    }

    #[tokio::test]
    async fn test_retry_exhausts_max_retries() {
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let result: Result<i32, AgentError> = with_retry(2, move || {
            let counter = counter_clone.clone();
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                Err(AgentError::Network("timeout".to_string()))
            }
        })
        .await;

        assert!(result.is_err());
        assert_eq!(counter.load(Ordering::SeqCst), 3); // Initial + 2 retries
    }
}
