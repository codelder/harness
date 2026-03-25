use crate::error::AgentError;
use crate::frontend::{
    frontend_event_channel,
    FrontendCommand,
    FrontendEvent,
    FrontendEventReceiver,
};
use crate::session::{SessionRuntime, SessionRuntimeConfig, SessionRuntimeOutcome};
use reedline::{DefaultPrompt, DefaultPromptSegment, Reedline, Signal};
use std::io::Write;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

/// Interactive REPL session
pub struct Session {
    runtime: SessionRuntime,
}

impl Session {
    /// Create a new session with minimal system prompt
    pub fn new() -> Self {
        Self {
            runtime: SessionRuntime::new(),
        }
    }

    /// Get a clone of the shared TodoManager for tool creation
    pub fn todo_manager(&self) -> Arc<Mutex<crate::planning::TodoManager>> {
        self.runtime.todo_manager()
    }

    /// Run the interactive REPL session
    pub async fn run(
        &mut self,
        provider_type: crate::llm::ProviderType,
        model: &str,
        base_url: Option<&str>,
        thinking: bool,
        thinking_budget: u64,
    ) -> Result<(), AgentError> {
        let (event_tx, mut event_rx) = frontend_event_channel(64);
        self.runtime
            .start(
                SessionRuntimeConfig {
                    provider_type,
                    model: model.to_string(),
                    base_url: base_url.map(str::to_string),
                    thinking,
                    thinking_budget,
                },
                &event_tx,
            )
            .await?;
        info!(provider = ?provider_type, model = model, thinking = thinking, "Session initialized");

        println!("Agent Harness v0.1.0");
        println!("Provider: {} | Model: {}", provider_type, model);
        if thinking {
            println!("Extended thinking: enabled (budget: {} tokens)", thinking_budget);
        }
        println!("Type your message and press Enter. Ctrl+C or Ctrl+D to exit.\n");
        Self::drain_events(&mut event_rx);

        let prompt = DefaultPrompt::new(
            DefaultPromptSegment::Basic("You".to_string()),
            DefaultPromptSegment::Empty,
        );

        loop {
            debug!("Waiting for user input");
            let prompt = prompt.clone();
            let result = tokio::task::spawn_blocking(move || Reedline::create().read_line(&prompt)).await;

            match result {
                Ok(Ok(Signal::Success(line))) => {
                    let line = line.trim();
                    if line.is_empty() {
                        continue;
                    }

                    if line == "exit" || line == "quit" {
                        info!("User requested exit via command");
                        let _ = self.runtime.handle_command(FrontendCommand::Exit, &event_tx).await?;
                        Self::drain_events(&mut event_rx);
                        return Ok(());
                    }

                    info!(input = line, "User input received");

                    print!("Agent: ");
                    if let Err(e) = std::io::stdout().flush() {
                        warn!("Failed to flush stdout: {}", e);
                    }

                    match self
                        .runtime
                        .handle_command(FrontendCommand::SubmitMessage(line.to_string()), &event_tx)
                        .await
                    {
                        Ok(SessionRuntimeOutcome::Continue | SessionRuntimeOutcome::Interrupted) => {}
                        Ok(SessionRuntimeOutcome::Exit(_)) => {
                            Self::drain_events(&mut event_rx);
                            return Ok(());
                        }
                        Err(e) => {
                            error!(error = %e, "Runtime command error");
                            eprintln!("\nError: {}", e);
                        }
                    }

                    Self::drain_events(&mut event_rx);
                }
                Ok(Ok(Signal::CtrlC)) => {
                    info!("User pressed Ctrl+C, exiting");
                    let _ = self.runtime.handle_command(FrontendCommand::Exit, &event_tx).await?;
                    Self::drain_events(&mut event_rx);
                    return Ok(());
                }
                Ok(Ok(Signal::CtrlD)) => {
                    info!("User pressed Ctrl+D, exiting");
                    let _ = self.runtime.handle_command(FrontendCommand::Exit, &event_tx).await?;
                    Self::drain_events(&mut event_rx);
                    return Ok(());
                }
                Ok(Err(err)) => {
                    error!(error = %err, "Reedline error");
                    eprintln!("\nTerminal error: {}", err);
                    eprintln!("This may be a terminal compatibility issue. Trying to continue...\n");

                    if err.to_string().contains("cursor position") {
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    }
                    continue;
                }
                Err(err) => {
                    error!(error = %err, "spawn_blocking join error");
                    eprintln!("Error: {}", err);
                    let _ = self.runtime.handle_command(FrontendCommand::Exit, &event_tx).await?;
                    Self::drain_events(&mut event_rx);
                    return Err(AgentError::Provider(crate::error::ProviderError::RequestFailed(
                        err.to_string(),
                    )));
                }
            }
        }
    }

    fn drain_events(event_rx: &mut FrontendEventReceiver) {
        while let Ok(event) = event_rx.try_recv() {
            Self::render_event(event);
        }
    }

    fn render_event(event: FrontendEvent) {
        match event {
            FrontendEvent::AssistantMessageCompleted { text } => println!("{}", text),
            FrontendEvent::Error { message } => eprintln!("\nError: {}", message),
            FrontendEvent::SessionEnded { summary } => {
                println!("\nSession Summary:");
                println!("  Turns: {}", summary.turns);
                println!("  Messages: {}", summary.message_count);
            }
            FrontendEvent::Status { message } => eprintln!("\n{}", message),
            FrontendEvent::Reminder { .. }
            | FrontendEvent::SessionStarted { .. }
            | FrontendEvent::UserMessageCommitted { .. }
            | FrontendEvent::AssistantMessageDelta { .. }
            | FrontendEvent::Thinking { .. }
            | FrontendEvent::ToolCallStarted { .. }
            | FrontendEvent::ToolCallFinished { .. }
            | FrontendEvent::RetryScheduled { .. } => {}
        }
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_initialization() {
        let session = Session::new();
        assert_eq!(session.runtime.message_count(), 1);
        assert_eq!(session.runtime.turn_count(), 0);
    }

    #[test]
    fn test_session_default() {
        let session = Session::default();
        assert_eq!(session.runtime.message_count(), 1);
    }

    #[test]
    fn test_system_message_content() {
        let session = Session::new();
        assert!(session.runtime.messages()[0].content.contains("AI agent"));
    }

    #[test]
    fn test_session_rounds_since_todo_initializes_to_zero() {
        let session = Session::new();
        assert_eq!(session.runtime.rounds_since_todo(), 0);
    }

    #[tokio::test]
    async fn test_session_todo_manager_initializes_empty() {
        let session = Session::new();
        let todo_manager = session.todo_manager();
        let manager = todo_manager.lock().await;
        assert!(manager.is_empty());
    }

    #[test]
    fn test_session_todo_manager_accessor_returns_shared_reference() {
        let session = Session::new();
        let manager1 = session.todo_manager();
        let manager2 = session.todo_manager();

        assert!(Arc::ptr_eq(&manager1, &manager2));
    }
}
