use super::{AgentTurn, Message, Role};
use crate::error::{classify_prompt_error, AgentError};
use crate::frontend::{
    FrontendEvent, FrontendEventSender, FrontendTodoItem, FrontendTodoStatus, SESSION_START_TURN_ID,
};
use rig::agent::{HookAction, PromptHook, ToolCallHookAction};
use rig::completion::{AssistantContent, CompletionModel};
use rig::message::ReasoningContent;
use serde::Deserialize;
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
        let args_preview = args.to_string();

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
        args: &str,
        result: &str,
    ) {
        tracing::debug!(
            tool_name,
            result_len = result.len(),
            result_preview_80 = truncate_preview(result, 80),
            "Raw tool result received by hook"
        );

        let decoded_result =
            serde_json::from_str::<String>(result).unwrap_or_else(|_| result.to_string());

        tracing::debug!(
            tool_name,
            decoded_len = decoded_result.len(),
            decoded_lines = decoded_result.lines().count(),
            decoded_preview_80 = truncate_preview(&decoded_result, 80),
            "Decoded tool result for UI"
        );

        let call_id = tool_call_id.unwrap_or_else(|| internal_call_id.to_string());
        let mut result_preview = if decoded_result.is_empty() {
            "(empty)".to_string()
        } else {
            decoded_result
        };
        let is_error = result_preview.starts_with("Toolset error: ")
            || result_preview.starts_with("ToolCallError: ");
        if is_error {
            result_preview = strip_error_chain(&result_preview);
        }

        self.emit_event(FrontendEvent::ToolCallFinished {
            turn_id: self.turn_id,
            call_id,
            name: tool_name.to_string(),
            result_preview,
            is_error,
        })
        .await;

        if tool_name == "todo" && !is_error {
            if let Some(items) = parse_todo_snapshot_args(args) {
                self.emit_event(FrontendEvent::TodoSnapshot {
                    turn_id: self.turn_id,
                    items,
                })
                .await;
            }
        }
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
        args: &str,
        result: &str,
    ) -> HookAction {
        self.record_tool_result(tool_name, tool_call_id, internal_call_id, args, result)
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
    async fn on_text_delta(&self, text_delta: &str, _aggregated_text: &str) -> HookAction {
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
    let (response_text, usage) = with_retry(
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
        response: response_text,
        usage,
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
        assert!(
            elapsed >= Duration::from_millis(100),
            "Expected at least 100ms wait, got {:?}",
            elapsed
        );
    }

    #[tokio::test]
    async fn hook_runtime_emits_tool_and_retry_events() {
        let (event_tx, mut event_rx) = frontend_event_channel(8);
        let (hook, used_todo) = TodoUsageHook::new(7, Some(event_tx));

        hook.record_tool_call(
            "todo",
            Some("tool-call".to_string()),
            "internal-1",
            "{\"items\":[]}",
        )
        .await;
        hook.record_tool_result(
            "todo",
            Some("tool-call".to_string()),
            "internal-1",
            "{\"items\":[]}",
            "\"updated\"",
        )
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
                is_error: false,
            })
        );
        assert_eq!(
            event_rx.recv().await,
            Some(FrontendEvent::TodoSnapshot {
                turn_id: 7,
                items: vec![],
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

    #[tokio::test]
    async fn hook_runtime_falls_back_to_internal_call_ids() {
        let (event_tx, mut event_rx) = frontend_event_channel(8);
        let (hook, _used_todo) = TodoUsageHook::new(9, Some(event_tx));

        hook.record_tool_call("read", None, "internal-call", "{\"path\":\"README.md\"}")
            .await;
        hook.record_tool_result(
            "read",
            None,
            "internal-call",
            "{\"path\":\"README.md\"}",
            "\"contents\"",
        )
        .await;

        assert_eq!(
            event_rx.recv().await,
            Some(FrontendEvent::ToolCallStarted {
                turn_id: 9,
                call_id: "internal-call".to_string(),
                name: "read".to_string(),
                args_preview: "{\"path\":\"README.md\"}".to_string(),
            })
        );
        assert_eq!(
            event_rx.recv().await,
            Some(FrontendEvent::ToolCallFinished {
                turn_id: 9,
                call_id: "internal-call".to_string(),
                name: "read".to_string(),
                result_preview: "contents".to_string(),
                is_error: false,
            })
        );
    }

    #[tokio::test]
    async fn hook_runtime_emits_turn_scoped_streaming_events() {
        let (event_tx, mut event_rx) = frontend_event_channel(8);
        let (hook, _used_todo) = TodoUsageHook::new(5, Some(event_tx));

        hook.record_thinking("reasoning".to_string()).await;
        hook.record_text_delta("delta").await;

        assert_eq!(
            event_rx.recv().await,
            Some(FrontendEvent::Thinking {
                turn_id: 5,
                text: "reasoning".to_string(),
            })
        );
        assert_eq!(
            event_rx.recv().await,
            Some(FrontendEvent::AssistantMessageDelta {
                turn_id: 5,
                delta: "delta".to_string(),
            })
        );
    }

    #[test]
    fn runtime_default_hook_uses_session_start_turn_id() {
        let hook = TodoUsageHook::default();
        assert_eq!(hook.turn_id, SESSION_START_TURN_ID);
    }

    /// Verify that the JSON encode/decode round-trip preserves newlines.
    /// This mirrors what rig's ToolDyn::call does (serde_json::to_string)
    /// and what record_tool_result does (serde_json::from_str::<String>).
    #[test]
    fn json_round_trip_preserves_multiline_content() {
        let original = "line 1\nline 2\nline 3\nline 4\nline 5".to_string();
        assert_eq!(original.lines().count(), 5, "sanity check");

        // Simulate rig's ToolDyn::call: serde_json::to_string(&output)
        let encoded = serde_json::to_string(&original).unwrap();
        tracing::info!("Encoded: {:?}", encoded);

        // Simulate hook's record_tool_result: serde_json::from_str::<String>(result)
        let decoded: String = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, original, "round-trip should preserve content");
        assert_eq!(decoded.lines().count(), 5, "lines should be preserved");
    }

    /// Verify that if serde_json::from_str fails, the fallback produces a
    /// single-line string (which would explain the "Read 1 lines" bug).
    #[test]
    fn json_fallback_preserves_raw_input_on_failure() {
        let non_json = "this is not json".to_string();
        let decoded =
            serde_json::from_str::<String>(&non_json).unwrap_or_else(|_| non_json.clone());
        assert_eq!(decoded, "this is not json");

        // Multi-line non-JSON: fallback preserves newlines
        let multiline = "line 1\nline 2\nline 3".to_string();
        let decoded =
            serde_json::from_str::<String>(&multiline).unwrap_or_else(|_| multiline.clone());
        assert_eq!(decoded.lines().count(), 3);
    }

    #[test]
    fn strip_error_chain_extracts_root_message() {
        // Full rig-core error chain — only strips rig's own wrappers
        assert_eq!(
            strip_error_chain("Toolset error: ToolCallError: ToolCallError: Failed to read file: No such file or directory (os error 2)"),
            "Failed to read file: No such file or directory (os error 2)"
        );

        // Tool's own message is preserved as-is
        assert_eq!(
            strip_error_chain(
                "Toolset error: ToolCallError: Command execution failed: Permission denied"
            ),
            "Command execution failed: Permission denied"
        );

        // No rig-core prefix — returned unchanged
        assert_eq!(
            strip_error_chain("something unexpected happened"),
            "something unexpected happened"
        );
    }

    #[test]
    fn truncate_preview_handles_multibyte_utf8() {
        let text = "你好，世界";
        let preview = truncate_preview(text, 5);
        assert!(preview.is_char_boundary(preview.len()));
        assert!(!preview.is_empty());
    }
}

/// Strip rig-core's error chain wrappers to extract the root error message.
///
/// rig-core wraps tool errors with fixed prefixes (see rig-core 0.31):
/// - `ToolSetToolError` adds `"ToolCallError: "`   (tool/mod.rs:408)
/// - `ToolError::Display` adds `"ToolCallError: "`  (tool/mod.rs:48)
/// - `ToolSetError` adds `"Toolset error: "`        (tool/server.rs:413)
/// - `request::ToolCallError` adds `"ToolCallError: "` (request.rs:129)
///
/// These are structural wrappers that always appear in the chain.
/// After stripping them, what remains is the tool's own error message.
fn strip_error_chain(error: &str) -> String {
    let prefixes = ["Toolset error: ", "ToolCallError: "];

    let mut result = error.to_string();
    loop {
        let original = result.clone();
        for prefix in &prefixes {
            if result.starts_with(prefix) {
                result = result[prefix.len()..].to_string();
            }
        }
        if result == original {
            break;
        }
    }
    result.trim().to_string()
}

fn truncate_preview(text: &str, max: usize) -> &str {
    let end = text.floor_char_boundary(max.min(text.len()));
    &text[..end]
}

#[derive(Debug, Deserialize)]
struct TodoSnapshotArgs {
    items: Vec<TodoSnapshotItem>,
}

#[derive(Debug, Deserialize)]
struct TodoSnapshotItem {
    id: u32,
    text: String,
    status: TodoSnapshotStatus,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum TodoSnapshotStatus {
    Pending,
    InProgress,
    Completed,
}

fn parse_todo_snapshot_args(args: &str) -> Option<Vec<FrontendTodoItem>> {
    let parsed = serde_json::from_str::<TodoSnapshotArgs>(args).ok()?;
    Some(
        parsed
            .items
            .into_iter()
            .map(|item| FrontendTodoItem {
                id: item.id,
                text: item.text,
                status: match item.status {
                    TodoSnapshotStatus::Pending => FrontendTodoStatus::Pending,
                    TodoSnapshotStatus::InProgress => FrontendTodoStatus::InProgress,
                    TodoSnapshotStatus::Completed => FrontendTodoStatus::Completed,
                },
            })
            .collect(),
    )
}
