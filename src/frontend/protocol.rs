use tokio::sync::mpsc;

/// Default bounded channel capacity between the runtime and any frontend adapter.
pub const DEFAULT_FRONTEND_CHANNEL_CAPACITY: usize = 64;

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
/// Must-deliver events are delivered with `send().await`.
/// Best-effort events currently share the same path and will be refined in GREEN.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrontendEvent {
    SessionStarted { provider: String, model: String },
    UserMessageCommitted { text: String },
    AssistantMessageDelta { delta: String },
    AssistantMessageCompleted { text: String },
    Thinking { text: String },
    ToolCallStarted { name: String, args_preview: String },
    ToolCallFinished { name: String, result_preview: String },
    RetryScheduled {
        attempt: u32,
        max_retries: u32,
        delay_ms: u64,
        reason: String,
    },
    Reminder { message: String },
    Status { message: String },
    Error { message: String },
    SessionEnded { summary: FrontendSessionSummary },
}

impl FrontendEvent {
    pub fn delivery_mode(&self) -> DeliveryMode {
        DeliveryMode::MustDeliver
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmitOutcome {
    Delivered,
    Coalesced,
}

#[derive(Debug, Clone)]
pub struct FrontendEventSender {
    tx: mpsc::Sender<FrontendEvent>,
}

impl FrontendEventSender {
    pub fn new(tx: mpsc::Sender<FrontendEvent>) -> Self {
        Self { tx }
    }

    pub async fn emit(
        &self,
        event: FrontendEvent,
    ) -> Result<EmitOutcome, mpsc::error::SendError<FrontendEvent>> {
        self.tx.send(event).await?;
        Ok(EmitOutcome::Delivered)
    }

    pub async fn flush_best_effort(
        &self,
    ) -> Result<usize, mpsc::error::SendError<FrontendEvent>> {
        Ok(0)
    }
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
        let command = FrontendCommand::SubmitMessage("hello".to_string());
        assert!(matches!(command, FrontendCommand::SubmitMessage(_)));

        let event = FrontendEvent::SessionEnded {
            summary: FrontendSessionSummary {
                turns: 3,
                message_count: 7,
            },
        };
        assert!(matches!(event, FrontendEvent::SessionEnded { .. }));
    }

    #[test]
    fn protocol_module_is_cli_framework_agnostic() {
        let source = include_str!("protocol.rs");

        assert!(!source.contains("ratatui"));
        assert!(!source.contains("reedline"));
        assert!(!source.contains("stdout"));
        assert!(!source.contains("stderr"));
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
            sender.emit(FrontendEvent::Status {
                message: "busy".to_string(),
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
            FrontendEvent::Status {
                message: "busy".to_string(),
            }
        );
    }
}
