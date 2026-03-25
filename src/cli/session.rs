use crate::cli::app::CliApp;
use crate::cli::terminal::{spawn_input_listener, TerminalGuard};
use crate::error::{AgentError, ProviderError};
use crate::frontend::{
    frontend_command_channel,
    frontend_event_channel,
    send_frontend_command,
    FrontendCommand,
    FrontendCommandReceiver,
    FrontendCommandSender,
    FrontendEventReceiver,
};
use crate::session::{SessionRuntime, SessionRuntimeConfig, SessionRuntimeOutcome};
use crossterm::event::KeyEvent;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, Mutex};
use tracing::{error, info};

/// Interactive ratatui-backed session.
pub struct Session {
    runtime: SessionRuntime,
}

impl Session {
    pub fn new() -> Self {
        Self {
            runtime: SessionRuntime::new(),
        }
    }

    pub fn todo_manager(&self) -> Arc<Mutex<crate::planning::TodoManager>> {
        self.runtime.todo_manager()
    }

    pub async fn run(
        &mut self,
        provider_type: crate::llm::ProviderType,
        model: &str,
        base_url: Option<&str>,
        thinking: bool,
        thinking_budget: u64,
    ) -> Result<(), AgentError> {
        let (event_tx, mut event_rx) = frontend_event_channel(64);
        let (command_tx, command_rx) = frontend_command_channel(32);

        let mut runtime = std::mem::take(&mut self.runtime);
        runtime
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

        let runtime_handle = tokio::spawn(run_runtime_loop(runtime, command_rx, event_tx.clone()));
        let stop_flag = Arc::new(AtomicBool::new(false));
        let (input_handle, mut input_rx) = spawn_input_listener(stop_flag.clone());
        let mut terminal = TerminalGuard::new().map_err(terminal_error)?;
        let mut app = CliApp::new();

        let loop_result = self
            .run_ui_loop(
                &mut app,
                &mut terminal,
                &command_tx,
                &mut event_rx,
                &mut input_rx,
            )
            .await;

        if loop_result.is_err() && !app.exit_requested() {
            let _ = send_frontend_command(&command_tx, FrontendCommand::Exit).await;
        }

        stop_flag.store(true, Ordering::SeqCst);
        drop(input_rx);
        drop(command_tx);

        let _ = input_handle.await.map_err(join_error)?.map_err(terminal_task_error);
        self.runtime = runtime_handle
            .await
            .map_err(join_error)?
            .map_err(|error| {
                error!(error = %error, "Runtime loop failed");
                error
            })?;

        loop_result
    }

    async fn run_ui_loop(
        &mut self,
        app: &mut CliApp,
        terminal: &mut TerminalGuard,
        command_tx: &FrontendCommandSender,
        event_rx: &mut FrontendEventReceiver,
        input_rx: &mut mpsc::UnboundedReceiver<KeyEvent>,
    ) -> Result<(), AgentError> {
        loop {
            Self::drain_events(app, event_rx);
            terminal.draw(|frame| app.render(frame)).map_err(terminal_error)?;

            if app.should_exit() {
                return Ok(());
            }

            tokio::select! {
                maybe_key = input_rx.recv() => {
                    match maybe_key {
                        Some(key) => {
                            if let Some(command) = app.handle_key_event(key) {
                                if matches!(command, FrontendCommand::Exit) {
                                    app.mark_exit_requested();
                                }
                                send_frontend_command(command_tx, command)
                                    .await
                                    .map_err(command_channel_closed)?;
                            }
                        }
                        None => return Ok(()),
                    }
                }
                _ = tokio::time::sleep(Duration::from_millis(50)) => {}
            }
        }
    }

    fn drain_events(app: &mut CliApp, event_rx: &mut FrontendEventReceiver) {
        while let Ok(event) = event_rx.try_recv() {
            app.apply_event(event);
        }
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

async fn run_runtime_loop(
    mut runtime: SessionRuntime,
    mut command_rx: FrontendCommandReceiver,
    event_tx: crate::frontend::FrontendEventSender,
) -> Result<SessionRuntime, AgentError> {
    while let Some(command) = command_rx.recv().await {
        if matches!(
            runtime.handle_command(command, &event_tx).await?,
            SessionRuntimeOutcome::Exit(_)
        ) {
            break;
        }
    }

    Ok(runtime)
}

fn terminal_error(error: std::io::Error) -> AgentError {
    AgentError::Provider(ProviderError::RequestFailed(format!(
        "Terminal error: {}",
        error
    )))
}

fn terminal_task_error(error: String) -> AgentError {
    AgentError::Provider(ProviderError::RequestFailed(format!(
        "Terminal input task error: {}",
        error
    )))
}

fn join_error(error: tokio::task::JoinError) -> AgentError {
    AgentError::Provider(ProviderError::RequestFailed(error.to_string()))
}

fn command_channel_closed(
    error: tokio::sync::mpsc::error::SendError<FrontendCommand>,
) -> AgentError {
    AgentError::Provider(ProviderError::RequestFailed(format!(
        "Frontend command channel closed: {}",
        error
    )))
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
