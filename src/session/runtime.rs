use crate::agent::{agent_loop, AgentTurn, Message, TodoUsageHook};
use crate::error::{AgentError, ProviderError};
use crate::frontend::{
    FrontendCommand, FrontendEvent, FrontendEventSender, FrontendSessionSummary, FrontendTodoItem,
    FrontendTodoStatus, HARNESS_VERSION, SESSION_START_TURN_ID,
};
use crate::llm::{create_parent_provider, LlmProvider, ProviderType};
use crate::planning::{TodoItem, TodoManager, TodoStatus};
use crate::subagent::SharedSubagentCallQueue;
use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::task::JoinHandle;
use tokio::sync::Mutex;

const SYSTEM_PROMPT: &str = "You are an AI agent with the ability to have a conversation. \
Respond naturally to user messages.";
const TODO_REMINDER: &str = "You have pending todos. Use the 'todo' tool to update your task list.";

#[allow(dead_code)]
type TurnFuture<'a> = Pin<Box<dyn Future<Output = Result<AgentTurn, AgentError>> + Send + 'a>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionSummary {
    pub turns: u32,
    pub messages: usize,
}

#[derive(Debug, Clone)]
pub struct SessionRuntimeConfig {
    pub provider_type: ProviderType,
    pub model: String,
    pub base_url: Option<String>,
    pub thinking: bool,
    pub thinking_budget: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionRuntimeOutcome {
    Continue,
    Interrupted,
    Exit(FrontendSessionSummary),
}

/// Headless runtime that owns conversation state independently of any UI.
pub struct SessionRuntime {
    messages: Vec<Message>,
    turn_count: u32,
    todo_manager: Arc<Mutex<TodoManager>>,
    rounds_since_todo: u32,
    provider: Option<Arc<LlmProvider>>,
    pending_subagent_calls: SharedSubagentCallQueue,
}

pub(crate) struct PendingTurn {
    pub(crate) turn_id: u64,
    pub(crate) current_input: String,
    pub(crate) previous_todos: Vec<TodoItem>,
    pub(crate) history: Vec<Message>,
    pub(crate) provider: Arc<LlmProvider>,
}

pub(crate) struct CompletedTurn {
    pub(crate) turn_id: u64,
    pub(crate) turn: AgentTurn,
    pub(crate) todo_used: bool,
    pub(crate) previous_todos: Vec<TodoItem>,
}

impl SessionRuntime {
    pub fn new() -> Self {
        Self {
            messages: vec![Message::system(SYSTEM_PROMPT)],
            turn_count: 0,
            todo_manager: Arc::new(Mutex::new(TodoManager::new())),
            rounds_since_todo: 0,
            provider: None,
            pending_subagent_calls: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    pub fn turn_count(&self) -> u32 {
        self.turn_count
    }

    pub fn rounds_since_todo(&self) -> u32 {
        self.rounds_since_todo
    }

    pub fn message_count(&self) -> u32 {
        self.messages.len() as u32
    }

    pub fn todo_manager(&self) -> Arc<Mutex<TodoManager>> {
        self.todo_manager.clone()
    }

    pub(crate) fn pending_subagent_calls(&self) -> SharedSubagentCallQueue {
        self.pending_subagent_calls.clone()
    }

    pub fn summary(&self) -> SessionSummary {
        SessionSummary {
            turns: self.turn_count,
            messages: self.messages.len(),
        }
    }

    pub async fn start(
        &mut self,
        config: SessionRuntimeConfig,
        event_tx: &FrontendEventSender,
    ) -> Result<(), AgentError> {
        let provider = match create_parent_provider(
            config.provider_type,
            &config.model,
            config.base_url.as_deref(),
            config.thinking,
            config.thinking_budget,
            self.todo_manager(),
            self.pending_subagent_calls.clone(),
        ) {
            Ok(provider) => provider,
            Err(error) => {
                let _ = self
                    .emit_event(
                        event_tx,
                        FrontendEvent::Error {
                            turn_id: SESSION_START_TURN_ID,
                            message: error.to_string(),
                        },
                    )
                    .await;
                return Err(error.into());
            }
        };

        self.start_with_provider(
            provider,
            config.provider_type.to_string(),
            config.model,
            event_tx,
        )
        .await
    }

    pub async fn start_with_provider(
        &mut self,
        provider: LlmProvider,
        provider_name: impl Into<String>,
        model: impl Into<String>,
        event_tx: &FrontendEventSender,
    ) -> Result<(), AgentError> {
        self.provider = Some(Arc::new(provider));
        let working_directory = std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| ".".to_string());
        self.emit_event(
            event_tx,
            FrontendEvent::SessionStarted {
                provider: provider_name.into(),
                model: model.into(),
                working_directory,
                version: HARNESS_VERSION.to_string(),
            },
        )
        .await?;
        self.emit_todo_snapshot(event_tx, SESSION_START_TURN_ID)
            .await
    }

    pub async fn handle_command(
        &mut self,
        command: FrontendCommand,
        event_tx: &FrontendEventSender,
    ) -> Result<SessionRuntimeOutcome, AgentError> {
        match command {
            FrontendCommand::SubmitMessage(text) => {
                let error_turn_id = self.active_turn_id();
                if let Err(error) = self.submit_message(text, event_tx).await {
                    self.emit_event(
                        event_tx,
                        FrontendEvent::Error {
                            turn_id: error_turn_id,
                            message: error.to_string(),
                        },
                    )
                    .await?;
                }
                Ok(SessionRuntimeOutcome::Continue)
            }
            FrontendCommand::Interrupt => {
                // If the UI sends an interrupt while we're idle (no active turn),
                // provide user-visible feedback instead of failing silently.
                let _ = self
                    .emit_event(
                        event_tx,
                        FrontendEvent::Status {
                            message: "Nothing to interrupt".to_string(),
                        },
                    )
                    .await;
                Ok(SessionRuntimeOutcome::Interrupted)
            }
            FrontendCommand::Exit => Ok(SessionRuntimeOutcome::Exit(self.end(event_tx).await?)),
        }
    }

    pub async fn submit_message(
        &mut self,
        current_input: impl Into<String>,
        event_tx: &FrontendEventSender,
    ) -> Result<(), AgentError> {
        let pending = self.begin_turn(current_input.into(), event_tx).await?;
        let completed = Self::execute_turn(pending, event_tx.clone(), self.pending_subagent_calls.clone())
            .await?;
        self.finish_turn(completed, event_tx).await
    }

    pub async fn end(
        &mut self,
        event_tx: &FrontendEventSender,
    ) -> Result<FrontendSessionSummary, AgentError> {
        let summary = FrontendSessionSummary {
            turns: self.turn_count,
            message_count: self.message_count(),
        };
        self.emit_event(
            event_tx,
            FrontendEvent::SessionEnded {
                summary: summary.clone(),
            },
        )
        .await?;
        Ok(summary)
    }

    #[allow(dead_code)]
    async fn submit_message_with<F>(
        &mut self,
        current_input: String,
        event_tx: &FrontendEventSender,
        executor: F,
    ) -> Result<(), AgentError>
    where
        F: for<'a> FnOnce(&'a [Message], &'a str, &'a LlmProvider, TodoUsageHook) -> TurnFuture<'a>,
    {
        let pending = self.begin_turn(current_input, event_tx).await?;
        let (hook, used_todo_flag) = TodoUsageHook::new(
            pending.turn_id,
            Some(event_tx.clone()),
            Some(self.pending_subagent_calls.clone()),
        );
        let turn = executor(
            &pending.history,
            &pending.current_input,
            pending.provider.as_ref(),
            hook,
        )
        .await?;
        let completed = CompletedTurn {
            turn_id: pending.turn_id,
            turn,
            todo_used: used_todo_flag.load(Ordering::SeqCst),
            previous_todos: pending.previous_todos,
        };
        self.finish_turn(completed, event_tx).await
    }

    pub(crate) async fn begin_turn(
        &mut self,
        current_input: String,
        event_tx: &FrontendEventSender,
    ) -> Result<PendingTurn, AgentError> {
        let provider = self.provider.clone().ok_or_else(not_started_error)?;
        let turn_id = self.next_turn_id();
        let previous_todos = self.todo_snapshot().await;
        self.emit_event(
            event_tx,
            FrontendEvent::UserMessageCommitted {
                turn_id,
                text: current_input.clone(),
            },
        )
        .await?;

        Ok(PendingTurn {
            turn_id,
            current_input,
            previous_todos,
            history: self.messages.clone(),
            provider,
        })
    }

    pub(crate) async fn execute_turn(
        pending: PendingTurn,
        event_tx: FrontendEventSender,
        pending_subagent_calls: SharedSubagentCallQueue,
    ) -> Result<CompletedTurn, AgentError> {
        let (hook, used_todo_flag) = TodoUsageHook::new(
            pending.turn_id,
            Some(event_tx),
            Some(pending_subagent_calls),
        );
        let turn = agent_loop(
            &pending.history,
            &pending.current_input,
            pending.provider.as_ref(),
            hook,
        )
        .await?;

        Ok(CompletedTurn {
            turn_id: pending.turn_id,
            turn,
            todo_used: used_todo_flag.load(Ordering::SeqCst),
            previous_todos: pending.previous_todos,
        })
    }

    pub(crate) async fn finish_turn(
        &mut self,
        completed: CompletedTurn,
        event_tx: &FrontendEventSender,
    ) -> Result<(), AgentError> {
        if completed.todo_used {
            self.rounds_since_todo = 0;
        }

        let current_todos = self.todo_snapshot().await;
        let todo_snapshot_changed = current_todos != completed.previous_todos;
        let reminder = self.todo_reminder_message().await;

        self.emit_event(
            event_tx,
            FrontendEvent::AssistantMessageCompleted {
                turn_id: completed.turn_id,
                text: completed.turn.response.clone(),
            },
        )
        .await?;

        if let Some(ref message) = reminder {
            self.emit_event(
                event_tx,
                FrontendEvent::Reminder {
                    turn_id: completed.turn_id,
                    message: message.clone(),
                },
            )
            .await?;
        }

        if todo_snapshot_changed || (reminder.is_some() && !current_todos.is_empty()) {
            self.emit_todo_snapshot_items(event_tx, completed.turn_id, current_todos)
                .await?;
        }

        self.commit_turn(completed.turn);
        Ok(())
    }

    pub(crate) async fn interrupt_active_turn(
        &mut self,
        turn_id: u64,
        event_tx: &FrontendEventSender,
        handle: JoinHandle<Result<CompletedTurn, AgentError>>,
    ) -> Result<SessionRuntimeOutcome, AgentError> {
        handle.abort();
        self.emit_event(
            event_tx,
            FrontendEvent::Status {
                message: "Interrupted".to_string(),
            },
        )
        .await?;
        self.emit_event(
            event_tx,
            FrontendEvent::Error {
                turn_id,
                message: "Interrupted".to_string(),
            },
        )
        .await?;
        Ok(SessionRuntimeOutcome::Interrupted)
    }

    fn commit_turn(&mut self, turn: AgentTurn) {
        self.messages.push(Message::user(turn.user_input));
        self.messages.push(Message::assistant(turn.response));
        self.turn_count += 1;
        self.rounds_since_todo += 1;
    }

    async fn todo_reminder_message(&self) -> Option<String> {
        if self.rounds_since_todo < 3 {
            return None;
        }

        let manager = self.todo_manager.lock().await;
        if manager.is_empty() {
            None
        } else {
            Some(TODO_REMINDER.to_string())
        }
    }

    async fn emit_event(
        &self,
        event_tx: &FrontendEventSender,
        event: FrontendEvent,
    ) -> Result<(), AgentError> {
        event_tx
            .emit(event)
            .await
            .map(|_| ())
            .map_err(frontend_channel_closed)
    }

    fn next_turn_id(&self) -> u64 {
        self.turn_count as u64 + 1
    }

    pub(crate) fn active_turn_id(&self) -> u64 {
        if self.provider.is_some() {
            self.next_turn_id()
        } else {
            SESSION_START_TURN_ID
        }
    }

    async fn emit_todo_snapshot(
        &self,
        event_tx: &FrontendEventSender,
        turn_id: u64,
    ) -> Result<(), AgentError> {
        let items = self.todo_snapshot().await;
        self.emit_todo_snapshot_items(event_tx, turn_id, items)
            .await
    }

    async fn emit_todo_snapshot_items(
        &self,
        event_tx: &FrontendEventSender,
        turn_id: u64,
        items: Vec<TodoItem>,
    ) -> Result<(), AgentError> {
        let items = items.into_iter().map(map_todo_item).collect::<Vec<_>>();

        self.emit_event(event_tx, FrontendEvent::TodoSnapshot { turn_id, items })
            .await
    }

    async fn todo_snapshot(&self) -> Vec<TodoItem> {
        let manager = self.todo_manager.lock().await;
        manager.snapshot()
    }
}

impl Default for SessionRuntime {
    fn default() -> Self {
        Self::new()
    }
}

fn not_started_error() -> AgentError {
    AgentError::Provider(ProviderError::RequestFailed(
        "Session runtime has not been started".to_string(),
    ))
}

fn frontend_channel_closed<T>(error: tokio::sync::mpsc::error::SendError<T>) -> AgentError {
    AgentError::Provider(ProviderError::RequestFailed(format!(
        "Frontend event channel closed: {}",
        error
    )))
}

fn map_todo_item(item: TodoItem) -> FrontendTodoItem {
    FrontendTodoItem {
        id: item.id,
        text: item.text,
        status: match item.status {
            TodoStatus::Pending => FrontendTodoStatus::Pending,
            TodoStatus::InProgress => FrontendTodoStatus::InProgress,
            TodoStatus::Completed => FrontendTodoStatus::Completed,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::TodoUsageHook;
    use crate::frontend::{frontend_event_channel, FrontendTodoItem, FrontendTodoStatus};
    use crate::planning::{TodoItem, TodoStatus};
    use rig::agent::PromptHook;
    use rig::providers::openai::responses_api::ResponsesCompletionModel;
    use tokio::time::{timeout, Duration};

    #[test]
    fn runtime_initial_state_matches_session_defaults() {
        let runtime = SessionRuntime::new();

        assert_eq!(runtime.message_count(), 1);
        assert_eq!(runtime.turn_count(), 0);
        assert_eq!(runtime.rounds_since_todo(), 0);
    }

    #[tokio::test]
    async fn turn_counter_updates_only_for_successful_turns() {
        let (event_tx, _event_rx) = frontend_event_channel(8);
        let mut runtime = SessionRuntime::new();
        runtime
            .start_with_provider(LlmProvider::Ollama, "test", "model", &event_tx)
            .await
            .unwrap();

        let error = runtime
            .submit_message_with(
                "hello".to_string(),
                &event_tx,
                |_history, _input, _provider, _hook| {
                    Box::pin(async { Err(AgentError::ToolsNotImplemented) })
                },
            )
            .await;
        assert!(error.is_err());
        assert_eq!(runtime.turn_count(), 0);

        runtime
            .submit_message_with(
                "hello".to_string(),
                &event_tx,
                |_history, _input, _provider, _hook| {
                    Box::pin(async {
                        Ok(AgentTurn {
                            user_input: "hello".to_string(),
                            response: "world".to_string(),
                            usage: None,
                        })
                    })
                },
            )
            .await
            .unwrap();

        assert_eq!(runtime.turn_count(), 1);
        assert_eq!(runtime.message_count(), 3);
    }

    #[tokio::test]
    async fn session_end_emits_summary_payload() {
        let (event_tx, mut event_rx) = frontend_event_channel(8);
        let mut runtime = SessionRuntime::new();
        runtime
            .start_with_provider(LlmProvider::Ollama, "test", "model", &event_tx)
            .await
            .unwrap();

        let summary = runtime.end(&event_tx).await.unwrap();
        assert_eq!(
            summary,
            FrontendSessionSummary {
                turns: 0,
                message_count: 1,
            }
        );

        let _ = event_rx.recv().await;
        let _ = event_rx.recv().await;
        let ended = event_rx
            .recv()
            .await
            .expect("session ended event should be emitted");
        assert_eq!(
            ended,
            FrontendEvent::SessionEnded {
                summary: FrontendSessionSummary {
                    turns: 0,
                    message_count: 1,
                },
            }
        );
    }

    #[tokio::test]
    async fn reminder_emission_stays_runtime_owned() {
        let (event_tx, mut event_rx) = frontend_event_channel(8);
        let mut runtime = SessionRuntime::new();
        runtime
            .start_with_provider(LlmProvider::Ollama, "test", "model", &event_tx)
            .await
            .unwrap();

        runtime.rounds_since_todo = 3;
        {
            let mut manager = runtime.todo_manager.lock().await;
            manager
                .update(vec![TodoItem::new(
                    1,
                    "Implement runtime".to_string(),
                    TodoStatus::Pending,
                )])
                .unwrap();
        }

        runtime
            .submit_message_with(
                "status".to_string(),
                &event_tx,
                |_history, _input, _provider, _hook| {
                    Box::pin(async {
                        Ok(AgentTurn {
                            user_input: "status".to_string(),
                            response: "Working on it".to_string(),
                            usage: None,
                        })
                    })
                },
            )
            .await
            .unwrap();

        let mut saw_reminder = false;
        let mut saw_embedded_reminder = false;

        while let Some(event) = event_rx.recv().await {
            match event {
                FrontendEvent::Reminder { message, .. } => {
                    saw_reminder = message.contains("pending todos");
                }
                FrontendEvent::AssistantMessageCompleted { text, .. } => {
                    saw_embedded_reminder = text.contains("<reminder>");
                }
                FrontendEvent::TodoSnapshot { items, .. } => {
                    if !items.is_empty() {
                        assert_eq!(
                            items,
                            vec![FrontendTodoItem {
                                id: 1,
                                text: "Implement runtime".to_string(),
                                status: FrontendTodoStatus::Pending,
                            }]
                        );
                        break;
                    }
                }
                _ => {}
            }
        }

        assert!(saw_reminder);
        assert!(!saw_embedded_reminder);
    }

    #[tokio::test]
    async fn runtime_does_not_emit_todo_snapshot_without_state_change() {
        let (event_tx, mut event_rx) = frontend_event_channel(8);
        let mut runtime = SessionRuntime::new();
        runtime
            .start_with_provider(LlmProvider::Ollama, "test", "model", &event_tx)
            .await
            .unwrap();

        {
            let mut manager = runtime.todo_manager.lock().await;
            manager
                .update(vec![TodoItem::new(
                    1,
                    "Pinned footer stays stable".to_string(),
                    TodoStatus::Pending,
                )])
                .unwrap();
        }

        let _ = event_rx.recv().await;
        let _ = event_rx.recv().await;

        runtime
            .submit_message_with(
                "status".to_string(),
                &event_tx,
                |_history, _input, _provider, hook| {
                    Box::pin(async move {
                        <TodoUsageHook as PromptHook<ResponsesCompletionModel>>::on_tool_call(
                            &hook,
                            "todo",
                            None,
                            "internal-1",
                            "{\"items\":[{\"id\":1}]}",
                        )
                        .await;

                        Ok(AgentTurn {
                            user_input: "status".to_string(),
                            response: "No change".to_string(),
                            usage: None,
                        })
                    })
                },
            )
            .await
            .unwrap();

        let mut saw_turn_snapshot = false;
        while let Ok(Some(event)) = timeout(Duration::from_millis(20), event_rx.recv()).await {
            if let FrontendEvent::TodoSnapshot { turn_id: 1, .. } = event {
                saw_turn_snapshot = true;
                break;
            }
        }

        assert!(!saw_turn_snapshot);
    }

    #[tokio::test]
    async fn startup_emits_session_snapshot_with_reserved_turn_id() {
        let (event_tx, mut event_rx) = frontend_event_channel(8);
        let mut runtime = SessionRuntime::new();

        runtime
            .start_with_provider(LlmProvider::Ollama, "test", "model", &event_tx)
            .await
            .unwrap();

        assert!(matches!(
            event_rx.recv().await,
            Some(FrontendEvent::SessionStarted {
                provider,
                model,
                working_directory: _,
                version: _,
            }) if provider == "test" && model == "model"
        ));
        assert_eq!(
            event_rx.recv().await,
            Some(FrontendEvent::TodoSnapshot {
                turn_id: SESSION_START_TURN_ID,
                items: Vec::new(),
            })
        );
    }

    #[tokio::test]
    async fn runtime_reports_preturn_submit_errors_with_session_start_turn_id() {
        let (event_tx, mut event_rx) = frontend_event_channel(8);
        let mut runtime = SessionRuntime::new();

        let outcome = runtime
            .handle_command(
                FrontendCommand::SubmitMessage("hello".to_string()),
                &event_tx,
            )
            .await
            .unwrap();

        assert_eq!(outcome, SessionRuntimeOutcome::Continue);
        assert_eq!(
            event_rx.recv().await,
            Some(FrontendEvent::Error {
                turn_id: SESSION_START_TURN_ID,
                message: not_started_error().to_string(),
            })
        );
    }

    #[tokio::test]
    async fn runtime_interrupt_command_returns_interrupted_outcome() {
        let (event_tx, mut event_rx) = frontend_event_channel(8);
        let mut runtime = SessionRuntime::new();

        let outcome = runtime
            .handle_command(FrontendCommand::Interrupt, &event_tx)
            .await
            .unwrap();

        assert_eq!(outcome, SessionRuntimeOutcome::Interrupted);
        assert_eq!(
            event_rx.recv().await,
            Some(FrontendEvent::Status {
                message: "Nothing to interrupt".to_string()
            })
        );
    }

    #[tokio::test]
    async fn runtime_emits_token_usage_from_agent_turn() {
        let (event_tx, mut event_rx) = frontend_event_channel(16);
        let mut runtime = SessionRuntime::new();
        runtime
            .start_with_provider(LlmProvider::Ollama, "test", "model", &event_tx)
            .await
            .unwrap();

        // Drain SessionStarted + initial TodoSnapshot
        let _ = event_rx.recv().await;
        let _ = event_rx.recv().await;

        runtime
            .submit_message_with(
                "hello".to_string(),
                &event_tx,
                |_history, _input, _provider, _hook| {
                    Box::pin(async {
                        Ok(AgentTurn {
                            user_input: "hello".to_string(),
                            response: "world".to_string(),
                            usage: Some(rig::completion::Usage::new()),
                        })
                    })
                },
            )
            .await
            .unwrap();

        // Collect events from this turn
        let mut saw_user_committed = false;
        let mut saw_assistant_completed = false;

        while let Ok(Some(event)) =
            tokio::time::timeout(std::time::Duration::from_millis(50), event_rx.recv()).await
        {
            match event {
                FrontendEvent::UserMessageCommitted { turn_id, text } => {
                    assert_eq!(turn_id, 1);
                    assert_eq!(text, "hello");
                    saw_user_committed = true;
                }
                FrontendEvent::AssistantMessageCompleted { turn_id, text } => {
                    assert_eq!(turn_id, 1);
                    assert_eq!(text, "world");
                    saw_assistant_completed = true;
                }
                _ => {}
            }
        }

        assert!(saw_user_committed, "should see UserMessageCommitted");
        assert!(saw_assistant_completed, "should see AssistantMessageCompleted");
    }
}
