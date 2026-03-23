use super::{AgentTurn, Message, Role};
use crate::error::{classify_prompt_error, AgentError};
use rig::completion::CompletionModel;
use rig::agent::{PromptHook, ToolCallHookAction};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

/// Maximum number of retries for retryable errors
const MAX_RETRIES: u32 = 3;

/// Initial delay for exponential backoff (1 second)
const INITIAL_DELAY: Duration = Duration::from_secs(1);

/// Hook that tracks whether the todo tool was called during agent execution.
///
/// This implements direct tool call detection (per Python reference implementation)
/// instead of indirect detection via TodoManager state changes.
///
/// The PromptHook trait is implemented generically for all CompletionModel types,
/// allowing this hook to work with both Anthropic and OpenAI providers.
#[derive(Clone)]
pub struct TodoUsageHook {
    /// Flag set to true when todo tool is called
    used_todo: Arc<AtomicBool>,
}

impl TodoUsageHook {
    /// Create a new hook with a shared flag.
    ///
    /// Returns the hook and a clone of the flag that can be checked
    /// after agent execution completes.
    pub fn new() -> (Self, Arc<AtomicBool>) {
        let used_todo = Arc::new(AtomicBool::new(false));
        let hook = Self {
            used_todo: used_todo.clone(),
        };
        (hook, used_todo)
    }
}

impl Default for TodoUsageHook {
    fn default() -> Self {
        Self::new().0
    }
}

impl<M> PromptHook<M> for TodoUsageHook
where
    M: CompletionModel,
{
    /// Called when a tool is invoked during agent execution.
    ///
    /// This is the DIRECT tool usage detection mechanism (per Python approach).
    /// When the todo tool is called, we set the flag to true.
    async fn on_tool_call(
        &self,
        tool_name: &str,
        _tool_call_id: Option<String>,
        _internal_call_id: &str,
        _args: &str,
    ) -> ToolCallHookAction {
        // Direct detection: if todo tool is called, set the flag
        // This is equivalent to Python's: used_todo = True
        if tool_name == "todo" {
            self.used_todo.store(true, Ordering::SeqCst);
        }
        // Always continue with tool execution
        ToolCallHookAction::cont()
    }
}

/// Execute an operation with automatic retry on retryable errors
///
/// For rate limiting: uses the retry-after duration specified by the API
/// For other errors: uses exponential backoff: 1s, 2s, 4s delays between retries
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
    let mut backoff_delay = INITIAL_DELAY;

    for attempt in 0..=max_retries {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) if e.is_retryable() && attempt < max_retries => {
                // Use API-specified retry-after for rate limiting, otherwise exponential backoff
                let retry_delay = match &e {
                    AgentError::RateLimited(duration) => *duration,
                    _ => backoff_delay,
                };

                eprintln!("Retry {}/{}: {} (waiting {:?})", attempt + 1, max_retries, e, retry_delay);
                sleep(retry_delay).await;

                // Only increase backoff for non-rate-limit errors
                if !matches!(e, AgentError::RateLimited(_)) {
                    backoff_delay *= 2;
                }
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

    #[tokio::test]
    async fn test_rate_limit_uses_retry_after_duration() {
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();
        let start = std::time::Instant::now();

        let result: Result<i32, AgentError> = with_retry(1, move || {
            let counter = counter_clone.clone();
            async move {
                let attempt = counter.fetch_add(1, Ordering::SeqCst);
                if attempt == 0 {
                    // Simulate rate limit with 100ms retry-after
                    Err(AgentError::RateLimited(Duration::from_millis(100)))
                } else {
                    Ok(42)
                }
            }
        })
        .await;

        let elapsed = start.elapsed();
        assert_eq!(result.unwrap(), 42);
        // Should have waited at least 100ms (the retry-after duration)
        assert!(elapsed >= Duration::from_millis(100), "Expected at least 100ms wait, got {:?}", elapsed);
    }
}
