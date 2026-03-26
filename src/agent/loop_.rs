use super::{AgentTurn, Message, Role};
use crate::error::{classify_prompt_error, AgentError};
use crate::frontend::{FrontendEvent, FrontendEventSender, SESSION_START_TURN_ID};
use rig::agent::{PromptHook, ToolCallHookAction, HookAction};
use rig::completion::{AssistantContent, CompletionModel};
use rig::message::ReasoningContent;
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
    /// Turn context for every emitted event during the active provider call.
    turn_id: u64,
    /// Optional side channel for structured runtime/frontend events.
    event_tx: Option<FrontendEventSender>,
}

impl TodoUsageHook {
    /// Create a new hook with a shared flag.
    ///
    /// Returns the hook and a clone of the flag that can be checked
    /// after agent execution completes.
    pub fn new(turn_id: u64, event_tx: Option<FrontendEventSender>) -> (Self, Arc<AtomicBool>) {
        let used_todo = Arc::new(AtomicBool::new(false));
        let hook = Self {
            used_todo: used_todo.clone(),
            turn_id,
            event_tx,
        };
        (hook, used_todo)
    }

    async fn record_tool_call(
        &self,
        tool_name: &str,
        tool_call_id: Option<String>,
        internal_call_id: &str,
        args: &str,
    ) {
        if tool_name == "todo" {
            self.used_todo.store(true, Ordering::SeqCst);
        }

        let call_id = tool_call_id.unwrap_or_else(|| internal_call_id.to_string());
        let args_preview = if args.len() > 100 {
            format!("{}...", &args[..100])
        } else {
            args.to_string()
        };

        self.emit_event(FrontendEvent::ToolCallStarted {
            turn_id: self.turn_id,
            call_id,
            name: tool_name.to_string(),
            args_preview,
        })
        .await;
    }

    async fn record_tool_result(
        &self,
        tool_name: &str,
        tool_call_id: Option<String>,
        internal_call_id: &str,
        result: &str,
    ) {
        let decoded_result =
            serde_json::from_str::<String>(result).unwrap_or_else(|_| result.to_string());

        let call_id = tool_call_id.unwrap_or_else(|| internal_call_id.to_string());
        let result_preview = if decoded_result.len() > 200 {
            format!("{}...", &decoded_result[..200])
        } else if decoded_result.is_empty() {
            "(empty)".to_string()
        } else {
            decoded_result
        };

        self.emit_event(FrontendEvent::ToolCallFinished {
            turn_id: self.turn_id,
            call_id,
            name: tool_name.to_string(),
            result_preview,
        })
        .await;
    }

    async fn record_thinking(&self, thinking_text: String) {
        if thinking_text.is_empty() {
            return;
        }

        self.emit_event(FrontendEvent::Thinking {
            turn_id: self.turn_id,
            text: thinking_text,
        })
        .await;
    }

    async fn record_text_delta(&self, text_delta: &str) {
        if text_delta.is_empty() {
            return;
        }

        self.emit_event(FrontendEvent::AssistantMessageDelta {
            turn_id: self.turn_id,
            delta: text_delta.to_string(),
        })
        .await;
    }

    pub async fn emit_retry_scheduled(
        &self,
        attempt: u32,
        max_retries: u32,
        delay: Duration,
        reason: String,
    ) {
        self.emit_event(FrontendEvent::RetryScheduled {
            turn_id: self.turn_id,
            attempt,
            max_retries,
            delay_ms: delay.as_millis() as u64,
            reason,
        })
        .await;
    }

    async fn emit_event(&self, event: FrontendEvent) {
        if let Some(event_tx) = &self.event_tx {
            let _ = event_tx.emit(event).await;
        }
    }
}

impl Default for TodoUsageHook {
    fn default() -> Self {
        Self::new(SESSION_START_TURN_ID, None).0
    }
}

impl<M> PromptHook<M> for TodoUsageHook
where
    M: CompletionModel,
{
    /// Called before a tool is invoked - display tool call in real-time
    async fn on_tool_call(
        &self,
        tool_name: &str,
        tool_call_id: Option<String>,
        internal_call_id: &str,
        args: &str,
    ) -> ToolCallHookAction {
        self.record_tool_call(tool_name, tool_call_id, internal_call_id, args)
            .await;
        ToolCallHookAction::cont()
    }

    /// Called after a tool is invoked - display tool result
    async fn on_tool_result(
        &self,
        tool_name: &str,
        tool_call_id: Option<String>,
        internal_call_id: &str,
        _args: &str,
        result: &str,
    ) -> HookAction {
        self.record_tool_result(tool_name, tool_call_id, internal_call_id, result)
            .await;
        HookAction::cont()
    }

    /// Called after receiving a completion response - display thinking content
    async fn on_completion_response(
        &self,
        _prompt: &rig::message::Message,
        response: &rig::completion::CompletionResponse<M::Response>,
    ) -> HookAction {
        // Extract and display reasoning/thinking content
        for content in response.choice.iter() {
            if let AssistantContent::Reasoning(reasoning) = content {
                // Get the reasoning text
                let thinking_text: String = reasoning
                    .content
                    .iter()
                    .filter_map(|c| match c {
                        ReasoningContent::Text { text, .. } => Some(text.clone()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n");

                self.record_thinking(thinking_text).await;
            }
        }
        HookAction::cont()
    }

    /// Called when receiving text delta (streaming) - optional real-time text display
    async fn on_text_delta(
        &self,
        text_delta: &str,
        _aggregated_text: &str,
    ) -> HookAction {
        self.record_text_delta(text_delta).await;
        HookAction::cont()
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
pub async fn with_retry<T, F, Fut, N, NFut>(
    max_retries: u32,
    mut operation: F,
    mut on_retry: N,
) -> Result<T, AgentError>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, AgentError>>,
    N: FnMut(u32, u32, Duration, &AgentError) -> NFut,
    NFut: std::future::Future<Output = ()>,
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

                on_retry(attempt + 1, max_retries, retry_delay, &e).await;
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
/// * `hook` - Hook for observing tool calls (e.g., TodoUsageHook)
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
    hook: TodoUsageHook,
) -> Result<AgentTurn, AgentError> {
    // Convert history to rig's Message type for chat history.
    // Note: chat_history should NOT include the current input because it is
    // passed separately as the prompt parameter.
    let rig_messages: Vec<rig::completion::Message> = history
        .iter()
        .filter_map(|m| match m.role {
            Role::User => Some(rig::completion::Message::user(&m.content)),
            Role::Assistant => Some(rig::completion::Message::assistant(&m.content)),
            Role::System => None,
        })
        .collect();

    let retry_hook = hook.clone();
    let response = with_retry(
        MAX_RETRIES,
        || {
            let hook_clone = hook.clone();
            async {
                provider
                    .chat_with_history_and_hook(
                        current_input.to_string(),
                        rig_messages.clone(),
                        hook_clone,
                    )
                    .await
                    .map_err(classify_prompt_error)
            }
        },
        move |attempt, max_retries, retry_delay, error| {
            let hook = retry_hook.clone();
            let reason = error.to_string();
            async move {
                hook.emit_retry_scheduled(attempt, max_retries, retry_delay, reason)
                    .await;
            }
        },
    )
    .await?;

    Ok(AgentTurn {
        user_input: current_input.to_string(),
        response,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontend::{frontend_event_channel, FrontendEvent, SESSION_START_TURN_ID};
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    #[tokio::test]
    async fn test_retry_success_on_first_try() {
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let result = with_retry(
            3,
            move || {
                let counter = counter_clone.clone();
                async move {
                    counter.fetch_add(1, Ordering::SeqCst);
                    Ok::<_, AgentError>(42)
                }
            },
            |_attempt, _max_retries, _retry_delay, _error| async {},
        )
        .await;

        assert_eq!(result.unwrap(), 42);
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_retry_succeeds_after_failures() {
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let result = with_retry(
            3,
            move || {
                let counter = counter_clone.clone();
                async move {
                    let attempt = counter.fetch_add(1, Ordering::SeqCst);
                    if attempt < 2 {
                        Err(AgentError::Network("timeout".to_string()))
                    } else {
                        Ok::<_, AgentError>(42)
                    }
                }
            },
            |_attempt, _max_retries, _retry_delay, _error| async {},
        )
        .await;

        assert_eq!(result.unwrap(), 42);
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_retry_non_retryable_error_fails_immediately() {
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let result: Result<i32, AgentError> = with_retry(
            3,
            move || {
                let counter = counter_clone.clone();
                async move {
                    counter.fetch_add(1, Ordering::SeqCst);
                    Err(AgentError::Auth("invalid key".to_string()))
                }
            },
            |_attempt, _max_retries, _retry_delay, _error| async {},
        )
        .await;

        assert!(result.is_err());
        matches!(result.unwrap_err(), AgentError::Auth(_));
        assert_eq!(counter.load(Ordering::SeqCst), 1); // Only called once
    }

    #[tokio::test]
    async fn test_retry_exhausts_max_retries() {
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let result: Result<i32, AgentError> = with_retry(
            2,
            move || {
                let counter = counter_clone.clone();
                async move {
                    counter.fetch_add(1, Ordering::SeqCst);
                    Err(AgentError::Network("timeout".to_string()))
                }
            },
            |_attempt, _max_retries, _retry_delay, _error| async {},
        )
        .await;

        assert!(result.is_err());
        assert_eq!(counter.load(Ordering::SeqCst), 3); // Initial + 2 retries
    }

    #[tokio::test]
    async fn test_rate_limit_uses_retry_after_duration() {
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();
        let start = std::time::Instant::now();

        let result: Result<i32, AgentError> = with_retry(
            1,
            move || {
                let counter = counter_clone.clone();
                async move {
                    let attempt = counter.fetch_add(1, Ordering::SeqCst);
                    if attempt == 0 {
                        Err(AgentError::RateLimited(Duration::from_millis(100)))
                    } else {
                        Ok(42)
                    }
                }
            },
            |_attempt, _max_retries, _retry_delay, _error| async {},
        )
        .await;

        let elapsed = start.elapsed();
        assert_eq!(result.unwrap(), 42);
        // Should have waited at least 100ms (the retry-after duration)
        assert!(elapsed >= Duration::from_millis(100), "Expected at least 100ms wait, got {:?}", elapsed);
    }

    #[tokio::test]
    async fn test_hook_emits_tool_and_retry_events() {
        let (event_tx, mut event_rx) = frontend_event_channel(8);
        let (hook, used_todo) = TodoUsageHook::new(7, Some(event_tx));

        hook.record_tool_call(
            "todo",
            Some("tool-call".to_string()),
            "internal-1",
            "{\"items\":[]}",
        )
        .await;
        hook.record_tool_result("todo", Some("tool-call".to_string()), "internal-1", "\"updated\"")
            .await;
        hook.emit_retry_scheduled(
            1,
            3,
            Duration::from_millis(250),
            "temporary network issue".to_string(),
        )
        .await;

        assert!(used_todo.load(Ordering::SeqCst));
        assert_eq!(
            event_rx.recv().await,
            Some(FrontendEvent::ToolCallStarted {
                turn_id: 7,
                call_id: "tool-call".to_string(),
                name: "todo".to_string(),
                args_preview: "{\"items\":[]}".to_string(),
            })
        );
        assert_eq!(
            event_rx.recv().await,
            Some(FrontendEvent::ToolCallFinished {
                turn_id: 7,
                call_id: "tool-call".to_string(),
                name: "todo".to_string(),
                result_preview: "updated".to_string(),
            })
        );
        assert_eq!(
            event_rx.recv().await,
            Some(FrontendEvent::RetryScheduled {
                turn_id: 7,
                attempt: 1,
                max_retries: 3,
                delay_ms: 250,
                reason: "temporary network issue".to_string(),
            })
        );
    }

    #[test]
    fn default_hook_uses_session_start_turn_id() {
        let hook = TodoUsageHook::default();
        assert_eq!(hook.turn_id, SESSION_START_TURN_ID);
    }
}
