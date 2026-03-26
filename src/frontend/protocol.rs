use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

/// Default bounded channel capacity between the runtime and any frontend adapter.
pub const DEFAULT_FRONTEND_CHANNEL_CAPACITY: usize = 64;

/// Reserved turn id for session-start snapshots and pre-turn metadata.
pub const SESSION_START_TURN_ID: u64 = 0;

/// Typed commands sent from an adapter to the shared runtime.
pub type FrontendCommandSender = mpsc::Sender<FrontendCommand>;
pub type FrontendCommandReceiver = mpsc::Receiver<FrontendCommand>;

/// Typed events emitted by the runtime to a frontend adapter.
pub type FrontendEventReceiver = mpsc::Receiver<FrontendEvent>;

/// Owned session summary shared with adapters when a session ends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontendSessionSummary {
    pub turns: u32,
    pub message_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrontendTodoStatus {
    Pending,
    InProgress,
    Completed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontendTodoItem {
    pub id: u32,
    pub text: String,
    pub status: FrontendTodoStatus,
}

/// Minimal user intent surface for v1 adapters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrontendCommand {
    SubmitMessage(String),
    Interrupt,
    Exit,
}

/// Delivery class for runtime-to-frontend events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryMode {
    MustDeliver,
    BestEffort,
}

/// Runtime-to-frontend protocol.
///
/// Delivery policy:
/// - must-deliver events use `send().await`
/// - best-effort events use `try_send`
/// - when a best-effort event hits a full bounded queue, the sender coalesces it
///   into the latest reducer-visible value so adapters can flush it later
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrontendEvent {
    SessionStarted {
        provider: String,
        model: String,
    },
    UserMessageCommitted {
        turn_id: u64,
        text: String,
    },
    AssistantMessageDelta {
        turn_id: u64,
        delta: String,
    },
    AssistantMessageCompleted {
        turn_id: u64,
        text: String,
    },
    Thinking {
        turn_id: u64,
        text: String,
    },
    ToolCallStarted {
        turn_id: u64,
        call_id: String,
        name: String,
        args_preview: String,
    },
    ToolCallFinished {
        turn_id: u64,
        call_id: String,
        name: String,
        result_preview: String,
    },
    RetryScheduled {
        turn_id: u64,
        attempt: u32,
        max_retries: u32,
        delay_ms: u64,
        reason: String,
    },
    Reminder {
        turn_id: u64,
        message: String,
    },
    TodoSnapshot {
        turn_id: u64,
        items: Vec<FrontendTodoItem>,
    },
    Status {
        message: String,
    },
    Error {
        turn_id: u64,
        message: String,
    },
    SessionEnded {
        summary: FrontendSessionSummary,
    },
}

impl FrontendEvent {
    pub fn delivery_mode(&self) -> DeliveryMode {
        match self {
            FrontendEvent::AssistantMessageDelta { .. }
            | FrontendEvent::Thinking { .. }
            | FrontendEvent::Status { .. } => DeliveryMode::BestEffort,
            FrontendEvent::SessionStarted { .. }
            | FrontendEvent::UserMessageCommitted { .. }
            | FrontendEvent::AssistantMessageCompleted { .. }
            | FrontendEvent::ToolCallStarted { .. }
            | FrontendEvent::ToolCallFinished { .. }
            | FrontendEvent::RetryScheduled { .. }
            | FrontendEvent::Reminder { .. }
            | FrontendEvent::TodoSnapshot { .. }
            | FrontendEvent::Error { .. }
            | FrontendEvent::SessionEnded { .. } => DeliveryMode::MustDeliver,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmitOutcome {
    Delivered,
    Coalesced,
}

#[derive(Debug, Default)]
struct BestEffortBacklog {
    assistant_delta: Option<(u64, String)>,
    thinking: Option<(u64, String)>,
    status: Option<String>,
}

impl BestEffortBacklog {
    fn push(&mut self, event: FrontendEvent) {
        match event {
            FrontendEvent::AssistantMessageDelta { turn_id, delta } => {
                self.assistant_delta = Some((turn_id, delta));
            }
            FrontendEvent::Thinking { turn_id, text } => {
                self.thinking = Some((turn_id, text));
            }
            FrontendEvent::Status { message } => {
                self.status = Some(message);
            }
            _ => {}
        }
    }

    fn pop_next(&mut self) -> Option<FrontendEvent> {
        if let Some((turn_id, delta)) = self.assistant_delta.take() {
            return Some(FrontendEvent::AssistantMessageDelta { turn_id, delta });
        }

        if let Some((turn_id, text)) = self.thinking.take() {
            return Some(FrontendEvent::Thinking { turn_id, text });
        }

        self.status
            .take()
            .map(|message| FrontendEvent::Status { message })
    }
}

#[derive(Debug, Clone)]
pub struct FrontendEventSender {
    tx: mpsc::Sender<FrontendEvent>,
    backlog: Arc<Mutex<BestEffortBacklog>>,
}

impl FrontendEventSender {
    pub fn new(tx: mpsc::Sender<FrontendEvent>) -> Self {
        Self {
            tx,
            backlog: Arc::new(Mutex::new(BestEffortBacklog::default())),
        }
    }

    pub async fn emit(
        &self,
        event: FrontendEvent,
    ) -> Result<EmitOutcome, mpsc::error::SendError<FrontendEvent>> {
        match event.delivery_mode() {
            DeliveryMode::MustDeliver => {
                self.tx.send(event).await?;
                Ok(EmitOutcome::Delivered)
            }
            DeliveryMode::BestEffort => self.try_emit_best_effort(event).await,
        }
    }

    async fn try_emit_best_effort(
        &self,
        event: FrontendEvent,
    ) -> Result<EmitOutcome, mpsc::error::SendError<FrontendEvent>> {
        match self.tx.try_send(event) {
            Ok(()) => Ok(EmitOutcome::Delivered),
            Err(mpsc::error::TrySendError::Full(event)) => {
                self.backlog.lock().await.push(event);
                Ok(EmitOutcome::Coalesced)
            }
            Err(mpsc::error::TrySendError::Closed(event)) => Err(mpsc::error::SendError(event)),
        }
    }

    pub async fn flush_best_effort(
        &self,
    ) -> Result<usize, mpsc::error::SendError<FrontendEvent>> {
        let mut flushed = 0;

        loop {
            let next_event = {
                let mut backlog = self.backlog.lock().await;
                backlog.pop_next()
            };

            let Some(event) = next_event else {
                break;
            };

            match self.tx.try_send(event) {
                Ok(()) => flushed += 1,
                Err(mpsc::error::TrySendError::Full(event)) => {
                    self.backlog.lock().await.push(event);
                    break;
                }
                Err(mpsc::error::TrySendError::Closed(event)) => {
                    return Err(mpsc::error::SendError(event));
                }
            }
        }

        Ok(flushed)
    }
}

/// Commands are always sent with `send().await` so adapters backpressure user input
/// instead of dropping it.
pub async fn send_frontend_command(
    tx: &FrontendCommandSender,
    command: FrontendCommand,
) -> Result<(), mpsc::error::SendError<FrontendCommand>> {
    tx.send(command).await
}

pub fn frontend_command_channel(
    capacity: usize,
) -> (FrontendCommandSender, FrontendCommandReceiver) {
    mpsc::channel(capacity)
}

pub fn frontend_event_channel(
    capacity: usize,
) -> (FrontendEventSender, FrontendEventReceiver) {
    let (tx, rx) = mpsc::channel(capacity);
    (FrontendEventSender::new(tx), rx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{timeout, Duration};

    #[test]
    fn command_and_event_types_are_constructible() {
        let command = FrontendCommand::Interrupt;
        assert!(matches!(command, FrontendCommand::Interrupt));

        let started = FrontendEvent::ToolCallStarted {
            turn_id: 7,
            call_id: "tool-1".to_string(),
            name: "read".to_string(),
            args_preview: "README.md".to_string(),
        };
        assert!(matches!(started, FrontendEvent::ToolCallStarted { .. }));

        let finished = FrontendEvent::ToolCallFinished {
            turn_id: 7,
            call_id: "tool-1".to_string(),
            name: "read".to_string(),
            result_preview: "done".to_string(),
        };
        assert!(matches!(finished, FrontendEvent::ToolCallFinished { .. }));

        let snapshot = FrontendEvent::TodoSnapshot {
            turn_id: SESSION_START_TURN_ID,
            items: vec![FrontendTodoItem {
                id: 1,
                text: "Ship protocol".to_string(),
                status: FrontendTodoStatus::InProgress,
            }],
        };
        assert!(matches!(snapshot, FrontendEvent::TodoSnapshot { .. }));
    }

    #[test]
    fn protocol_module_is_cli_framework_agnostic() {
        let source = include_str!("protocol.rs");
        let production_source = source.split("#[cfg(test)]").next().unwrap_or(source);
        let forbidden_terms = [
            ["rata", "tui"].concat(),
            ["reed", "line"].concat(),
            ["std", "out"].concat(),
            ["std", "err"].concat(),
            "crossterm".to_string(),
        ];

        for forbidden in forbidden_terms {
            assert!(!production_source.contains(&forbidden));
        }
    }

    #[test]
    fn todo_snapshot_is_must_deliver() {
        assert_eq!(
            FrontendEvent::TodoSnapshot {
                turn_id: SESSION_START_TURN_ID,
                items: Vec::new(),
            }
            .delivery_mode(),
            DeliveryMode::MustDeliver
        );
    }

    #[tokio::test]
    async fn best_effort_events_do_not_block_when_channel_is_full() {
        let (sender, mut receiver) = frontend_event_channel(1);

        sender
            .emit(FrontendEvent::SessionStarted {
                provider: "anthropic".to_string(),
                model: "claude".to_string(),
            })
            .await
            .expect("must-deliver event should fit");

        let result = timeout(
            Duration::from_millis(50),
            sender.emit(FrontendEvent::AssistantMessageDelta {
                turn_id: 5,
                delta: "busy".to_string(),
            }),
        )
        .await;

        let outcome = result.expect("best-effort emit should not block");
        assert_eq!(outcome.expect("channel open"), EmitOutcome::Coalesced);

        let first = receiver.recv().await.expect("first event should be queued");
        assert!(matches!(first, FrontendEvent::SessionStarted { .. }));

        let flushed = sender.flush_best_effort().await.expect("flush should succeed");
        assert_eq!(flushed, 1);

        let second = receiver.recv().await.expect("coalesced event should flush");
        assert_eq!(
            second,
            FrontendEvent::AssistantMessageDelta {
                turn_id: 5,
                delta: "busy".to_string(),
            }
        );
    }
}
