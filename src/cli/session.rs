use crate::cli::app::CliApp;
use crate::cli::terminal::{spawn_input_listener, InputEvent, TerminalGuard};
use crate::error::{AgentError, ProviderError};
use crate::frontend::{
    frontend_command_channel, frontend_event_channel, send_frontend_command, FrontendCommand,
    FrontendCommandReceiver, FrontendCommandSender, FrontendEventReceiver, FrontendEventSender,
};
use crate::session::{SessionRuntime, SessionRuntimeConfig, SessionRuntimeOutcome};
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
        let runtime_event_tx = event_tx.clone();
        let ui_event_tx = event_tx.clone();

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

        let runtime_handle = tokio::spawn(run_runtime_loop(runtime, command_rx, runtime_event_tx));
        let stop_flag = Arc::new(AtomicBool::new(false));
        let (input_handle, mut input_rx) = spawn_input_listener(stop_flag.clone());
        let mut terminal = TerminalGuard::new().map_err(terminal_error)?;
        let mut app = CliApp::new();

        let loop_result = self
            .run_ui_loop(
                &mut app,
                &mut terminal,
                &command_tx,
                &ui_event_tx,
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
        event_tx: &FrontendEventSender,
        event_rx: &mut FrontendEventReceiver,
        input_rx: &mut mpsc::UnboundedReceiver<InputEvent>,
    ) -> Result<(), AgentError> {
        loop {
            Self::drain_events_and_flush(app, event_rx, event_tx).await?;
            terminal.draw(|frame| app.render(frame)).map_err(terminal_error)?;

            if app.should_exit() {
                return Ok(());
            }

            flush_best_effort(event_tx).await?;

            tokio::select! {
                maybe_event = input_rx.recv() => {
                    match maybe_event {
                        Some(InputEvent::Key(key)) => {
                            if let Some(command) = app.handle_key_event(key) {
                                if matches!(command, FrontendCommand::Exit) {
                                    app.mark_exit_requested();
                                }
                                send_frontend_command(command_tx, command)
                                    .await
                                    .map_err(command_channel_closed)?;
                            }
                        }
                        Some(InputEvent::Resize(_columns, _rows)) => {
                            // Resize is handled by terminal guard - just trigger re-render
                            let _ = terminal.handle_resize();
                        }
                        None => return Ok(()),
                    }
                }
                _ = tokio::time::sleep(Duration::from_millis(50)) => {
                    flush_best_effort(event_tx).await?;
                }
            }
        }
    }

    fn drain_events(app: &mut CliApp, event_rx: &mut FrontendEventReceiver) {
        while let Ok(event) = event_rx.try_recv() {
            app.apply_event(event);
        }
    }

    async fn drain_events_and_flush(
        app: &mut CliApp,
        event_rx: &mut FrontendEventReceiver,
        event_tx: &FrontendEventSender,
    ) -> Result<(), AgentError> {
        Self::drain_events(app, event_rx);
        flush_best_effort(event_tx).await
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

fn frontend_event_channel_closed(
    error: tokio::sync::mpsc::error::SendError<crate::frontend::FrontendEvent>,
) -> AgentError {
    AgentError::Provider(ProviderError::RequestFailed(format!(
        "Frontend event channel closed: {}",
        error
    )))
}

async fn flush_best_effort(event_tx: &FrontendEventSender) -> Result<(), AgentError> {
    event_tx
        .flush_best_effort()
        .await
        .map(|_| ())
        .map_err(frontend_event_channel_closed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontend::{
        EmitOutcome, FrontendEvent, FrontendSessionSummary, SESSION_START_TURN_ID,
    };
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn render_lines(app: &mut CliApp) -> Vec<String> {
        let backend = TestBackend::new(60, 12);
        let mut terminal = Terminal::new(backend).expect("test terminal should initialize");
        terminal
            .draw(|frame| app.render(frame))
            .expect("render should succeed");

        let buffer = terminal.backend().buffer().clone();
        (0..buffer.area.height)
            .map(|row| {
                (0..buffer.area.width)
                    .map(|col| buffer[(col, row)].symbol())
                    .collect::<String>()
            })
            .collect()
    }

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

    #[tokio::test]
    async fn drain_events_and_flush_delivers_best_effort_backlog() {
        let (event_tx, mut event_rx) = frontend_event_channel(1);
        let mut app = CliApp::new();

        event_tx
            .emit(FrontendEvent::SessionStarted {
                provider: "ollama".to_string(),
                model: "test-model".to_string(),
                working_directory: "/test".to_string(),
                version: "0.1.0".to_string(),
            })
            .await
            .expect("session started should fit");

        let outcome = event_tx
            .emit(FrontendEvent::AssistantMessageDelta {
                turn_id: SESSION_START_TURN_ID + 1,
                delta: "backlogged delta".to_string(),
            })
            .await
            .expect("best-effort delta should not fail");

        assert_eq!(outcome, EmitOutcome::Coalesced);

        Session::drain_events_and_flush(&mut app, &mut event_rx, &event_tx)
            .await
            .expect("ui loop flush should succeed");
        Session::drain_events(&mut app, &mut event_rx);

        let lines = render_lines(&mut app);
        // Status shows spinner when streaming is active (not "Connected" label)
        assert!(lines.iter().any(|line| line.contains("Thinking") || line.contains("Processing")));
        assert!(lines.iter().any(|line| line.contains("<<< Assistant")));
        assert!(lines.iter().any(|line| line.contains("backlogged delta")));
    }

    #[tokio::test]
    async fn session_end_still_uses_runtime_event_path() {
        let (event_tx, mut event_rx) = frontend_event_channel(8);
        let mut runtime = SessionRuntime::new();

        runtime
            .start_with_provider(crate::llm::LlmProvider::Ollama, "ollama", "test-model", &event_tx)
            .await
            .expect("runtime should start");
        let _ = event_rx.recv().await;

        let outcome = runtime
            .handle_command(FrontendCommand::Exit, &event_tx)
            .await
            .expect("exit should succeed");

        assert_eq!(
            outcome,
            SessionRuntimeOutcome::Exit(FrontendSessionSummary {
                turns: 0,
                message_count: 1,
            })
        );

        let mut saw_session_end = false;
        while let Some(event) = event_rx.recv().await {
            if matches!(event, FrontendEvent::SessionEnded { .. }) {
                saw_session_end = true;
                break;
            }
        }

        assert!(saw_session_end, "runtime should emit SessionEnded");
    }
}
