use crate::agent::{agent_loop, AgentTurn, Message, TodoUsageHook};
use crate::error::{AgentError, ProviderError};
use crate::frontend::{FrontendCommand, FrontendEvent, FrontendEventSender, FrontendSessionSummary};
use crate::llm::{create_provider, LlmProvider, ProviderType};
use crate::planning::TodoManager;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::sync::Mutex;

const SYSTEM_PROMPT: &str = "You are an AI agent with the ability to have a conversation. \
Respond naturally to user messages.";
const TODO_REMINDER: &str =
    "You have pending todos. Use the 'todo' tool to update your task list.";

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
    provider: Option<LlmProvider>,
}

impl SessionRuntime {
    pub fn new() -> Self {
        Self {
            messages: vec![Message::system(SYSTEM_PROMPT)],
            turn_count: 0,
            todo_manager: Arc::new(Mutex::new(TodoManager::new())),
            rounds_since_todo: 0,
            provider: None,
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
        let provider = create_provider(
            config.provider_type,
            &config.model,
            config.base_url.as_deref(),
            config.thinking,
            config.thinking_budget,
            self.todo_manager(),
        )?;

        self.start_with_provider(provider, config.provider_type.to_string(), config.model, event_tx)
            .await
    }

    pub async fn start_with_provider(
        &mut self,
        provider: LlmProvider,
        provider_name: impl Into<String>,
        model: impl Into<String>,
        event_tx: &FrontendEventSender,
    ) -> Result<(), AgentError> {
        self.provider = Some(provider);
        self.emit_event(
            event_tx,
            FrontendEvent::SessionStarted {
                provider: provider_name.into(),
                model: model.into(),
            },
        )
        .await
    }

    pub async fn handle_command(
        &mut self,
        command: FrontendCommand,
        event_tx: &FrontendEventSender,
    ) -> Result<SessionRuntimeOutcome, AgentError> {
        match command {
            FrontendCommand::SubmitMessage(text) => {
                if let Err(error) = self.submit_message(text, event_tx).await {
                    self.emit_event(
                        event_tx,
                        FrontendEvent::Error {
                            message: error.to_string(),
                        },
                    )
                    .await?;
                }

                Ok(SessionRuntimeOutcome::Continue)
            }
            FrontendCommand::Interrupt => {
                self.emit_event(
                    event_tx,
                    FrontendEvent::Status {
                        message: "Interrupt requested but not yet implemented".to_string(),
                    },
                )
                .await?;
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
        let current_input = current_input.into();
        let provider = self.provider.as_ref().ok_or_else(not_started_error)?;
        let (hook, used_todo_flag) = TodoUsageHook::new();
        let turn = agent_loop(&self.messages, &current_input, provider, hook).await?;
        let todo_used = used_todo_flag.load(Ordering::SeqCst);

        if todo_used {
            self.rounds_since_todo = 0;
        }

        let reminder = self.todo_reminder_message().await;
        let display_response = if let Some(ref reminder_text) = reminder {
            format!(
                "<reminder>{}</reminder>\n\n{}",
                reminder_text, turn.response
            )
        } else {
            turn.response.clone()
        };

        self.emit_event(
            event_tx,
            FrontendEvent::UserMessageCommitted {
                text: turn.user_input.clone(),
            },
        )
        .await?;

        if let Some(ref message) = reminder {
            self.emit_event(
                event_tx,
                FrontendEvent::Reminder {
                    message: message.clone(),
                },
            )
            .await?;
        }

        self.emit_event(
            event_tx,
            FrontendEvent::AssistantMessageCompleted {
                text: display_response,
            },
        )
        .await?;

        self.commit_turn(turn);
        Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontend::frontend_event_channel;
    use crate::planning::{TodoItem, TodoStatus};

    #[test]
    fn test_runtime_initialization() {
        let runtime = SessionRuntime::new();

        assert_eq!(runtime.messages().len(), 1);
        assert_eq!(runtime.turn_count(), 0);
        assert_eq!(runtime.rounds_since_todo(), 0);
    }

    #[tokio::test]
    async fn test_runtime_todo_manager_initializes_empty() {
        let runtime = SessionRuntime::new();
        let manager = runtime.todo_manager();
        let manager = manager.lock().await;

        assert!(manager.is_empty());
    }

    #[test]
    fn test_runtime_summary_reflects_initial_state() {
        let runtime = SessionRuntime::new();

        assert_eq!(
            runtime.summary(),
            SessionSummary {
                turns: 0,
                messages: 1,
            }
        );
    }

    #[tokio::test]
    async fn test_runtime_emits_session_lifecycle_events() {
        let (event_tx, mut event_rx) = frontend_event_channel(8);
        let mut runtime = SessionRuntime::new();

        runtime
            .start_with_provider(
                LlmProvider::Ollama,
                ProviderType::Ollama.to_string(),
                "test-model",
                &event_tx,
            )
            .await
            .unwrap();
        let summary = runtime.end(&event_tx).await.unwrap();

        let started = event_rx.recv().await.expect("session started event");
        let ended = event_rx.recv().await.expect("session ended event");

        assert_eq!(
            summary,
            FrontendSessionSummary {
                turns: 0,
                message_count: 1,
            }
        );

        assert_eq!(
            started,
            FrontendEvent::SessionStarted {
                provider: ProviderType::Ollama.to_string(),
                model: "test-model".to_string(),
            }
        );
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
    async fn test_runtime_reminder_threshold_is_runtime_owned() {
        let mut runtime = SessionRuntime::new();
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

        runtime.rounds_since_todo = 2;
        assert_eq!(runtime.todo_reminder_message().await, None);

        runtime.rounds_since_todo = 3;
        assert_eq!(
            runtime.todo_reminder_message().await,
            Some(TODO_REMINDER.to_string())
        );
    }
}
