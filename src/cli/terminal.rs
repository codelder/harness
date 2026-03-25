use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyEvent, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode,
    enable_raw_mode,
    EnterAlternateScreen,
    LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::{Frame, Terminal};
use std::io::{self, Stdout};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

pub type CliTerminal = Terminal<CrosstermBackend<Stdout>>;

/// Centralized terminal lifecycle guard for raw mode and alternate screen ownership.
pub struct TerminalGuard {
    terminal: Option<CliTerminal>,
}

impl TerminalGuard {
    pub fn new() -> io::Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;

        Ok(Self {
            terminal: Some(terminal),
        })
    }

    pub fn draw<F>(&mut self, render: F) -> io::Result<()>
    where
        F: FnOnce(&mut Frame),
    {
        if let Some(terminal) = self.terminal.as_mut() {
            terminal.draw(render)?;
        }
        Ok(())
    }

    pub fn restore(&mut self) -> io::Result<()> {
        if let Some(mut terminal) = self.terminal.take() {
            disable_raw_mode()?;
            execute!(
                terminal.backend_mut(),
                LeaveAlternateScreen,
                DisableMouseCapture
            )?;
            terminal.show_cursor()?;
        }
        Ok(())
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

/// Dedicated blocking input task that feeds crossterm key events into the async UI loop.
pub fn spawn_input_listener(
    stop_flag: Arc<AtomicBool>,
) -> (JoinHandle<Result<(), String>>, mpsc::UnboundedReceiver<KeyEvent>) {
    let (tx, rx) = mpsc::unbounded_channel();

    let handle = tokio::task::spawn_blocking(move || -> Result<(), String> {
        while !stop_flag.load(Ordering::SeqCst) {
            if event::poll(Duration::from_millis(50)).map_err(|error| error.to_string())? {
                if let Event::Key(key) = event::read().map_err(|error| error.to_string())? {
                    if key.kind == KeyEventKind::Press && tx.send(key).is_err() {
                        break;
                    }
                }
            }
        }
        Ok(())
    });

    (handle, rx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restore_is_idempotent_without_active_terminal() {
        let mut guard = TerminalGuard { terminal: None };
        assert!(guard.restore().is_ok());
        assert!(guard.restore().is_ok());
    }
}
