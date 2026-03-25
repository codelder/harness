use crate::agent::{agent_loop, AgentTurn, Message, TodoUsageHook};
use crate::error::{AgentError, ProviderError};
use crate::frontend::{
    FrontendEvent,
    FrontendEventSender,
    FrontendSessionSummary,
};
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionTurnOutcome {
    pub display_response: String,
    pub committed_response: String,
    pub reminder: Option<String>,
    pub todo_used: bool,
}

/// Headless runtime that owns conversation state independently of any UI.
pub struct SessionRuntime {
    messages: Vec<Message>,
    turn_count: u32,
    todo_manager: Arc<Mutex<TodoManager>>,
    rounds_since_todo: u32,
}

impl SessionRuntime {
    pub fn new() -> Self {
        Self {
            messages: vec![Message::system(SYSTEM_PROMPT)],
            turn_count: 0,
            todo_manager: Arc::new(Mutex::new(TodoManager::new())),
            rounds_since_todo: 0,
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

    pub fn todo_manager(&self) -> Arc<Mutex<TodoManager>> {
        self.todo_manager.clone()
    }

    pub fn summary(&self) -> SessionSummary {
        SessionSummary {
            turns: self.turn_count,
            messages: self.messages.len(),
        }
    }

    pub fn create_provider(
        &self,
        provider_type: ProviderType,
        model: &str,
        base_url: Option<&str>,
        thinking: bool,
        thinking_budget: u64,
    ) -> Result<LlmProvider, ProviderError> {
        create_provider(
            provider_type,
            model,
            base_url,
            thinking,
            thinking_budget,
            self.todo_manager(),
        )
    }

    pub async fn emit_session_started(
        &self,
        provider_type: ProviderType,
        model: &str,
        event_tx: Option<&FrontendEventSender>,
    ) {
        self.emit_event(
            event_tx,
            FrontendEvent::SessionStarted {
                provider: provider_type.to_string(),
                model: model.to_string(),
            },
        )
        .await;
    }

    pub async fn emit_session_ended(&self, event_tx: Option<&FrontendEventSender>) {
        let summary = self.summary();
        self.emit_event(
            event_tx,
            FrontendEvent::SessionEnded {
                summary: FrontendSessionSummary {
                    turns: summary.turns,
                    message_count: summary.messages as u32,
                },
            },
        )
        .await;
    }

    pub async fn submit_message(
        &mut self,
        provider: &LlmProvider,
        current_input: &str,
        event_tx: Option<&FrontendEventSender>,
    ) -> Result<SessionTurnOutcome, AgentError> {
        self.emit_event(
            event_tx,
            FrontendEvent::UserMessageCommitted {
                text: current_input.to_string(),
            },
        )
        .await;

        let (hook, used_todo_flag) = TodoUsageHook::new();
        let turn = agent_loop(&self.messages, current_input, provider, hook).await?;
        let todo_used = used_todo_flag.load(Ordering::SeqCst);

        if todo_used {
            self.rounds_since_todo = 0;
            used_todo_flag.store(false, Ordering::SeqCst);
        }

        let reminder = self.todo_reminder_message().await;
        if let Some(ref message) = reminder {
            self.emit_event(
                event_tx,
                FrontendEvent::Reminder {
                    message: message.clone(),
                },
            )
            .await;
        }

        self.emit_event(
            event_tx,
            FrontendEvent::AssistantMessageCompleted {
                text: turn.response.clone(),
            },
        )
        .await;

        let committed_response = turn.response.clone();
        let display_response = if let Some(ref reminder_text) = reminder {
            format!("<reminder>{}</reminder>\n\n{}", reminder_text, committed_response)
        } else {
            committed_response.clone()
        };

        self.commit_turn(turn);

        Ok(SessionTurnOutcome {
            display_response,
            committed_response,
            reminder,
            todo_used,
        })
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
        event_tx: Option<&FrontendEventSender>,
        event: FrontendEvent,
    ) {
        if let Some(tx) = event_tx {
            let _ = tx.emit(event).await;
        }
    }
}

impl Default for SessionRuntime {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontend::frontend_event_channel;
    use crate::planning::{TodoItem, TodoStatus};

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
                |_history, _input, _provider, _hook| async {
                    Err(AgentError::ToolsNotImplemented)
                },
            )
            .await;
        assert!(error.is_err());
        assert_eq!(runtime.turn_count(), 0);

        runtime
            .submit_message_with(
                "hello".to_string(),
                &event_tx,
                |_history, _input, _provider, _hook| async {
                    Ok(AgentTurn {
                        user_input: "hello".to_string(),
                        response: "world".to_string(),
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
        let ended = event_rx.recv().await.expect("session ended event should be emitted");
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
                |_history, _input, _provider, _hook| async {
                    Ok(AgentTurn {
                        user_input: "status".to_string(),
                        response: "Working on it".to_string(),
                    })
                },
            )
            .await
            .unwrap();

        let mut saw_reminder = false;
        let mut saw_embedded_reminder = false;

        while let Some(event) = event_rx.recv().await {
            match event {
                FrontendEvent::Reminder { message } => {
                    saw_reminder = message.contains("pending todos");
                    break;
                }
                FrontendEvent::AssistantMessageCompleted { text } => {
                    saw_embedded_reminder = text.contains("<reminder>");
                }
                _ => {}
            }
        }

        assert!(saw_reminder);
        assert!(saw_embedded_reminder);
    }
}
