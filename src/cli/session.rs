use crate::cli::app::CliApp;
use crate::cli::terminal::{spawn_input_listener, InputEvent, TerminalGuard};
use crate::error::{AgentError, ProviderError};
use crate::frontend::{
    frontend_command_channel, frontend_event_channel, send_frontend_command, FrontendCommand,
    FrontendCommandReceiver, FrontendCommandSender, FrontendEvent, FrontendEventReceiver,
    FrontendEventSender,
};
use crate::session::{CompletedTurn, SessionRuntime, SessionRuntimeConfig};
#[cfg(test)]
use crate::session::SessionRuntimeOutcome;
use ratatui::prelude::Widget;
use ratatui::widgets::Paragraph;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, Mutex};
use tracing::{error, info};
use unicode_width::UnicodeWidthStr;

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

        // Print banner before entering raw mode so it scrolls with the terminal
        {
            let display_dir = std::env::current_dir()
                .map(|p| p.display().to_string())
                .unwrap_or_default();
            // Replace home directory with ~/
            let home = std::env::var("HOME").unwrap_or_default();
            let display_dir = if !home.is_empty() && display_dir.starts_with(&home) {
                format!("~/{}", &display_dir[home.len()..].trim_start_matches('/'))
            } else {
                display_dir
            };
            let dir = if display_dir.len() > 40 {
                format!("...{}", &display_dir[display_dir.len() - 37..])
            } else {
                display_dir
            };

            // Banner — ANSI Shadow style ASCII art for "HARNESS"
            let version = env!("CARGO_PKG_VERSION");
            // ANSI escape codes: bold + white, dim/gray, reset
            let bold = "\x1b[1m";
            let dim = "\x1b[2m";
            let reset = "\x1b[0m";
            println!("\r");
            println!("\r  ██╗  ██╗ █████╗ ██████╗ ███╗   ██╗ ███████╗███████╗███████╗");
            println!("\r  ██║  ██║██╔══██╗██╔══██╗████╗  ██║ ██╔════╝██╔════╝██╔════╝");
            println!("\r  ███████║███████║███████║██╔██╗ ██║ █████╗  ███████╗███████╗");
            println!("\r  ██╔══██║██╔══██║██╔══██║██║╚██╗██║ ██╔══╝  ╚════██║╚════██║");
            println!("\r  ██║  ██║██║  ██║██║  ██║██║ ╚████║ ███████╗███████║███████║");
            println!("\r  ╚═╝  ╚═╝╚═╝  ╚═╝╚═╝  ╚═╝╚═╝  ╚═══╝ ╚══════╝╚══════╝╚══════╝");
            println!("\r  {bold}Rust AI Agent Harness{reset} {dim}· v{version}{reset}");
            println!("\r  {dim}{model} · {dir}{reset}");
            println!();
        }

        // Enable raw mode and create inline viewport BEFORE starting input listener.
        // This ensures crossterm's event reader initializes in raw mode,
        // which is required for correct input handling in Warp and other terminals.
        let mut terminal = TerminalGuard::new().map_err(terminal_error)?;
        let stop_flag = Arc::new(AtomicBool::new(false));
        let (input_handle, mut input_rx) = spawn_input_listener(stop_flag.clone());

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

        let _ = input_handle
            .await
            .map_err(join_error)?
            .map_err(terminal_task_error);
        self.runtime = runtime_handle.await.map_err(join_error)?.map_err(|error| {
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
        let mut needs_redraw = true;
        let mut viewport_height = terminal.viewport_height();
        loop {
            let drained_events = Self::drain_events_and_flush(app, event_rx, event_tx).await?;
            let tick_changed = app.tick();
            let desired_viewport_height = app.desired_viewport_height();
            let target_viewport_height = if desired_viewport_height > viewport_height {
                desired_viewport_height
            } else if desired_viewport_height < viewport_height && app.allow_viewport_shrink() {
                desired_viewport_height
            } else {
                viewport_height
            };
            let viewport_changed = terminal
                .set_viewport_height(target_viewport_height)
                .map_err(terminal_error)?;
            viewport_height = target_viewport_height;

            // Insert new timeline content above the viewport
            let new_lines = app.drain_new_lines();
            let inserted_new_lines = !new_lines.is_empty();
            if inserted_new_lines {
                let height = new_lines.len() as u16;
                terminal
                    .insert_before(height, |buf| {
                        let paragraph = Paragraph::new(new_lines);
                        paragraph.render(buf.area, buf);

                        // Fix CJK wide-character placeholder cells.
                        let width = buf.area.width as usize;
                        for y in 0..buf.area.height {
                            let mut x: usize = 0;
                            while x < width {
                                let cell = &buf[(x as u16, y)];
                                let w = UnicodeWidthStr::width(cell.symbol()).max(1);
                                if w > 1 {
                                    for dx in 1..w {
                                        if x + dx < width {
                                            buf[((x + dx) as u16, y)].set_symbol("");
                                        }
                                    }
                                }
                                x += w;
                            }
                        }
                    })
                    .map_err(terminal_error)?;
            }

            needs_redraw |= drained_events || tick_changed || viewport_changed || inserted_new_lines;

            if needs_redraw {
                terminal
                    .draw(|frame| app.render(frame))
                    .map_err(terminal_error)?;
                needs_redraw = false;
            }

            if app.should_exit() {
                return Ok(());
            }

            flush_best_effort(event_tx).await?;

            tokio::select! {
                maybe_event = input_rx.recv() => {
                    match maybe_event {
                        Some(InputEvent::Key(key)) => {
                            needs_redraw = true;
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
                            needs_redraw = true;
                            let _ = terminal.handle_resize();
                        }
                        None => return Ok(()),
                    }
                }
                _ = tokio::time::sleep(Duration::from_millis(80)) => {
                    // Keep spinner and token interpolation smooth without over-redrawing.
                    needs_redraw |= app.tick();
                    flush_best_effort(event_tx).await?;
                }
            }
        }
    }

    fn drain_events(app: &mut CliApp, event_rx: &mut FrontendEventReceiver) -> bool {
        let mut count = 0;
        while let Ok(event) = event_rx.try_recv() {
            tracing::debug!("UI received event: {:?}", std::mem::discriminant(&event));
            app.apply_event(event);
            count += 1;
        }
        if count > 0 {
            tracing::debug!(
                "drain_events: {} events, executing: {}",
                count,
                app.executing_tool_count()
            );
        }
        count > 0
    }

    async fn drain_events_and_flush(
        app: &mut CliApp,
        event_rx: &mut FrontendEventReceiver,
        event_tx: &FrontendEventSender,
    ) -> Result<bool, AgentError> {
        let drained = Self::drain_events(app, event_rx);
        flush_best_effort(event_tx).await?;
        Ok(drained)
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
    let mut active_turn: Option<(
        u64,
        tokio::task::JoinHandle<Result<CompletedTurn, AgentError>>,
    )> = None;

    loop {
        if let Some((_, handle)) = active_turn.as_mut() {
            tokio::select! {
                result = handle => {
                    let result = result.map_err(join_error)?;
                    active_turn = None;
                    runtime.finish_turn(result?, &event_tx).await?;
                }
                maybe_command = command_rx.recv() => {
                    match maybe_command {
                        Some(FrontendCommand::Interrupt) => {
                            let (turn_id, handle) = active_turn.take().expect("active turn");
                            runtime.interrupt_active_turn(turn_id, &event_tx, handle).await?;
                        }
                        Some(FrontendCommand::Exit) => {
                            let (turn_id, handle) = active_turn.take().expect("active turn");
                            runtime.interrupt_active_turn(turn_id, &event_tx, handle).await?;
                            break;
                        }
                        Some(FrontendCommand::SubmitMessage(_)) => {
                            event_tx
                                .emit(FrontendEvent::Status {
                                    message: "Already processing a request".to_string(),
                                })
                                .await
                                .map_err(frontend_event_channel_closed)?;
                        }
                        None => break,
                    }
                }
            }
        } else {
            let Some(command) = command_rx.recv().await else {
                break;
            };

            match command {
                FrontendCommand::SubmitMessage(text) => {
                    let error_turn_id = runtime.active_turn_id();
                    match runtime.begin_turn(text, &event_tx).await {
                        Ok(pending) => {
                            let turn_id = pending.turn_id;
                            let event_tx_clone = event_tx.clone();
                            let pending_calls = runtime.pending_subagent_calls();
                            let handle = tokio::spawn(async move {
                                SessionRuntime::execute_turn(pending, event_tx_clone, pending_calls).await
                            });
                            active_turn = Some((turn_id, handle));
                        }
                        Err(error) => {
                            event_tx
                                .emit(FrontendEvent::Error {
                                    turn_id: error_turn_id,
                                    message: error.to_string(),
                                })
                                .await
                                .map_err(frontend_event_channel_closed)?;
                        }
                    }
                }
                FrontendCommand::Interrupt => {
                    runtime.handle_command(FrontendCommand::Interrupt, &event_tx).await?;
                }
                FrontendCommand::Exit => {
                    break;
                }
            }
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
        let backend = TestBackend::new(60, app.desired_viewport_height());
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
        // Streaming text shows in composer area
        assert!(lines.iter().any(|line| line.contains("backlogged delta")
            || line.contains("Thinking")
            || line.contains("Processing")));
    }

    #[tokio::test]
    async fn session_end_still_uses_runtime_event_path() {
        let (event_tx, mut event_rx) = frontend_event_channel(8);
        let mut runtime = SessionRuntime::new();

        runtime
            .start_with_provider(
                crate::llm::LlmProvider::Ollama,
                "ollama",
                "test-model",
                &event_tx,
            )
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
