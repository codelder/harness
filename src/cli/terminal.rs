use crossterm::event::{self, Event, KeyEvent, KeyEventKind};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use ratatui::backend::CrosstermBackend;
use ratatui::{Frame, Terminal, TerminalOptions, Viewport};
use std::io::{self, Stdout};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

pub type CliTerminal = Terminal<CrosstermBackend<Stdout>>;

/// Minimum viewport height for composer + status.
const VIEWPORT_HEIGHT: u16 = 4;

/// Calculate the baseline inline viewport height from terminal size.
/// The session may expand this later when todo/tools need more room.
/// Timeline content is printed directly to stdout and scrolls with the terminal.
pub fn calculate_inline_height(_terminal_height: u16) -> u16 {
    VIEWPORT_HEIGHT
}

/// Centralized terminal lifecycle guard for raw mode and inline viewport ownership.
pub struct TerminalGuard {
    terminal: Option<CliTerminal>,
    raw_mode_enabled: bool,
    inline_height: u16,
}

impl TerminalGuard {
    pub fn new() -> io::Result<Self> {
        let mut ops = SystemLifecycleOps;
        let mut state = LifecycleState::default();
        let terminal = initialize_terminal(&mut ops, &mut state)?;

        Ok(Self {
            terminal: Some(terminal),
            raw_mode_enabled: state.raw_mode_enabled,
            inline_height: state.inline_height,
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

    pub fn insert_before<F>(&mut self, height: u16, render_fn: F) -> io::Result<()>
    where
        F: FnOnce(&mut ratatui::buffer::Buffer),
    {
        if let Some(terminal) = self.terminal.as_mut() {
            terminal.insert_before(height, render_fn)?;
        }
        Ok(())
    }

    /// Handle terminal resize by recalculating inline height.
    /// Returns true if viewport height changed.
    pub fn handle_resize(&mut self) -> io::Result<bool> {
        if let Some(terminal) = &mut self.terminal {
            // Inline/fullscreen viewports are auto-resized by ratatui 0.30+.
            // Keep the current inline height; autoresize only reconciles width and
            // the viewport's on-screen position with the terminal's new size.
            terminal.autoresize()?;
            return Ok(false);
        }
        Ok(false)
    }

    /// Dynamically adjust viewport height (e.g. to show executing tools).
    /// Returns true if the height actually changed.
    #[allow(dead_code)]
    pub fn set_viewport_height(&mut self, height: u16) -> io::Result<bool> {
        if height == self.inline_height {
            tracing::trace!("set_viewport_height: unchanged at {}", height);
            return Ok(false);
        }
        let old_height = self.inline_height;
        if let Some(mut terminal) = self.terminal.take() {
            // `Viewport::Inline(height)` stores the height in the terminal itself.
            // Calling `resize(Rect)` only resizes buffers; it does not mutate the
            // inline viewport's configured height. Recreate the terminal instead.
            terminal.clear()?;
            drop(terminal);

            self.inline_height = height;
            let backend = CrosstermBackend::new(io::stdout());
            let terminal = Terminal::with_options(
                backend,
                TerminalOptions {
                    viewport: Viewport::Inline(height),
                },
            )?;
            self.terminal = Some(terminal);
            tracing::debug!("set_viewport_height: {} -> {}", old_height, height);
            return Ok(true);
        }
        Ok(false)
    }

    /// Get the current viewport height.
    #[allow(dead_code)]
    pub fn viewport_height(&self) -> u16 {
        self.inline_height
    }

    /// Get the terminal size (width, height).
    #[allow(dead_code)]
    pub fn size(&self) -> io::Result<(u16, u16)> {
        if let Some(terminal) = &self.terminal {
            let size = terminal.size()?;
            Ok((size.width, size.height))
        } else {
            Ok((80, 24)) // Default fallback
        }
    }

    pub fn restore(&mut self) -> io::Result<()> {
        let mut ops = SystemLifecycleOps;
        let mut terminal = self.terminal.take();
        let mut state = LifecycleState {
            raw_mode_enabled: self.raw_mode_enabled,
            inline_height: self.inline_height,
        };
        let result = restore_terminal(&mut ops, terminal.as_mut(), &mut state);

        self.raw_mode_enabled = state.raw_mode_enabled;
        self.inline_height = state.inline_height;

        result
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

/// Dedicated blocking input task that feeds crossterm key events into the async UI loop.
/// Also passes through resize events for viewport reconfiguration.
pub fn spawn_input_listener(
    stop_flag: Arc<AtomicBool>,
) -> (
    JoinHandle<Result<(), String>>,
    mpsc::UnboundedReceiver<InputEvent>,
) {
    let (tx, rx) = mpsc::unbounded_channel();

    let handle = tokio::task::spawn_blocking(move || -> Result<(), String> {
        while !stop_flag.load(Ordering::SeqCst) {
            if event::poll(Duration::from_millis(50)).map_err(|error| error.to_string())? {
                match event::read().map_err(|error| error.to_string())? {
                    Event::Key(key) => {
                        // Accept Press and Release events to support terminals like Warp
                        // that may send different key event kinds than iTerm2/Terminal.app
                        if key.kind != KeyEventKind::Repeat
                            && tx.send(InputEvent::Key(key)).is_err()
                        {
                            break;
                        }
                    }
                    Event::Resize(columns, rows) => {
                        if tx.send(InputEvent::Resize(columns, rows)).is_err() {
                            break;
                        }
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    });

    (handle, rx)
}

/// Input events from the terminal.
#[derive(Debug, Clone)]
pub enum InputEvent {
    Key(KeyEvent),
    Resize(u16, u16),
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct LifecycleState {
    raw_mode_enabled: bool,
    inline_height: u16,
}

trait TerminalLifecycleOps {
    type Terminal;

    fn enable_raw_mode(&mut self) -> io::Result<()>;
    fn disable_raw_mode(&mut self) -> io::Result<()>;
    fn get_terminal_height(&self) -> io::Result<u16>;
    fn create_inline_terminal(&mut self, height: u16) -> io::Result<Self::Terminal>;
    fn show_cursor(&mut self, terminal: &mut Self::Terminal) -> io::Result<()>;
}

struct SystemLifecycleOps;

impl TerminalLifecycleOps for SystemLifecycleOps {
    type Terminal = CliTerminal;

    fn enable_raw_mode(&mut self) -> io::Result<()> {
        enable_raw_mode()
    }

    fn disable_raw_mode(&mut self) -> io::Result<()> {
        disable_raw_mode()
    }

    fn get_terminal_height(&self) -> io::Result<u16> {
        let size = crossterm::terminal::size()?;
        Ok(size.1)
    }

    fn create_inline_terminal(&mut self, height: u16) -> io::Result<Self::Terminal> {
        let backend = CrosstermBackend::new(io::stdout());
        Terminal::with_options(
            backend,
            TerminalOptions {
                viewport: Viewport::Inline(height),
            },
        )
    }

    fn show_cursor(&mut self, terminal: &mut Self::Terminal) -> io::Result<()> {
        terminal.show_cursor()
    }
}

fn initialize_terminal<Ops: TerminalLifecycleOps>(
    ops: &mut Ops,
    state: &mut LifecycleState,
) -> io::Result<Ops::Terminal> {
    ops.enable_raw_mode()?;
    state.raw_mode_enabled = true;

    // Calculate initial inline height
    let height = calculate_inline_height(ops.get_terminal_height()?);
    state.inline_height = height;

    match ops.create_inline_terminal(height) {
        Ok(terminal) => Ok(terminal),
        Err(error) => {
            let _ = rollback_terminal_setup(ops, state);
            Err(error)
        }
    }
}

fn rollback_terminal_setup<Ops: TerminalLifecycleOps>(
    ops: &mut Ops,
    state: &mut LifecycleState,
) -> io::Result<()> {
    restore_terminal::<Ops>(ops, None, state)
}

fn restore_terminal<Ops: TerminalLifecycleOps>(
    ops: &mut Ops,
    terminal: Option<&mut Ops::Terminal>,
    state: &mut LifecycleState,
) -> io::Result<()> {
    let mut first_error = None;

    if let Some(active_terminal) = terminal {
        capture_cleanup_error(ops.show_cursor(active_terminal), &mut first_error);
    }

    if state.raw_mode_enabled {
        capture_cleanup_error(ops.disable_raw_mode(), &mut first_error);
        state.raw_mode_enabled = false;
    }

    // Reset inline_height on cleanup
    state.inline_height = 0;

    if let Some(error) = first_error {
        Err(error)
    } else {
        Ok(())
    }
}

fn capture_cleanup_error(result: io::Result<()>, first_error: &mut Option<io::Error>) {
    if let Err(error) = result {
        if first_error.is_none() {
            *first_error = Some(error);
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::panic::{self, AssertUnwindSafe};
    use std::sync::{Arc, Mutex};

    #[allow(dead_code)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum RestoreScenario {
        CleanExit,
        RuntimeError,
    }

    #[derive(Debug)]
    struct MockTerminal;

    #[derive(Debug, Clone)]
    struct MockLifecycleOps {
        log: Arc<Mutex<Vec<&'static str>>>,
        fail_step: Option<&'static str>,
        terminal_height: u16,
    }

    impl MockLifecycleOps {
        fn new(log: Arc<Mutex<Vec<&'static str>>>, fail_step: Option<&'static str>) -> Self {
            Self {
                log,
                fail_step,
                terminal_height: 24, // Default terminal height for tests
            }
        }

        fn push(&self, step: &'static str) {
            self.log.lock().expect("log should lock").push(step);
        }

        fn maybe_fail(&self, step: &'static str) -> io::Result<()> {
            self.push(step);
            if self.fail_step == Some(step) {
                Err(io::Error::other(format!("mock {step} failure")))
            } else {
                Ok(())
            }
        }
    }

    impl TerminalLifecycleOps for MockLifecycleOps {
        type Terminal = MockTerminal;

        fn enable_raw_mode(&mut self) -> io::Result<()> {
            self.maybe_fail("enable_raw_mode")
        }

        fn disable_raw_mode(&mut self) -> io::Result<()> {
            self.maybe_fail("disable_raw_mode")
        }

        fn get_terminal_height(&self) -> io::Result<u16> {
            Ok(self.terminal_height)
        }

        fn create_inline_terminal(&mut self, height: u16) -> io::Result<Self::Terminal> {
            self.push("create_inline_terminal");
            // Check if we should fail BEFORE pushing height (to match test expectations)
            if self.fail_step == Some("create_inline_terminal") {
                return Err(io::Error::other("mock create_inline_terminal failure"));
            }
            // Use Box::leak to get 'static lifetime for the formatted string
            let height_str = Box::leak(format!("height_{}", height).into_boxed_str());
            self.push(height_str);
            Ok(MockTerminal)
        }

        fn show_cursor(&mut self, _terminal: &mut Self::Terminal) -> io::Result<()> {
            self.maybe_fail("show_cursor")
        }
    }

    struct PanicRestoreGuard {
        ops: MockLifecycleOps,
        terminal: Option<MockTerminal>,
        state: LifecycleState,
    }

    impl Drop for PanicRestoreGuard {
        fn drop(&mut self) {
            let _ = restore_terminal(&mut self.ops, self.terminal.as_mut(), &mut self.state);
            self.terminal = None;
        }
    }

    #[allow(dead_code)]
    pub(crate) fn terminal_restore_smoke(
        scenario: RestoreScenario,
    ) -> io::Result<Vec<&'static str>> {
        let log = Arc::new(Mutex::new(Vec::new()));
        let mut ops = MockLifecycleOps::new(log.clone(), None);
        let mut state = LifecycleState::default();
        let mut terminal = initialize_terminal(&mut ops, &mut state)?;

        match scenario {
            RestoreScenario::CleanExit => {}
            RestoreScenario::RuntimeError => ops.push("runtime_error"),
        }

        restore_terminal(&mut ops, Some(&mut terminal), &mut state)?;
        let entries = log.lock().expect("log should lock").clone();
        Ok(entries)
    }

    #[test]
    fn restore_is_idempotent_without_active_terminal() {
        let mut guard = TerminalGuard {
            terminal: None,
            raw_mode_enabled: false,
            inline_height: 0,
        };
        assert!(guard.restore().is_ok());
        assert!(guard.restore().is_ok());
    }

    #[test]
    fn constructor_rollback_restores_raw_mode_if_terminal_creation_fails() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let mut ops = MockLifecycleOps::new(log.clone(), Some("create_inline_terminal"));
        let mut state = LifecycleState::default();

        let error = initialize_terminal(&mut ops, &mut state).expect_err("init should fail");

        assert!(error.to_string().contains("create_inline_terminal"));
        assert_eq!(
            log.lock().expect("log should lock").clone(),
            vec![
                "enable_raw_mode",
                "create_inline_terminal",
                "disable_raw_mode",
            ]
        );
        assert_eq!(state, LifecycleState::default());
    }

    #[test]
    fn panic_path_restore_cleans_up_terminal_state() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let result = panic::catch_unwind(AssertUnwindSafe({
            let log = log.clone();
            move || {
                let mut ops = MockLifecycleOps::new(log, None);
                let mut state = LifecycleState::default();
                let terminal =
                    initialize_terminal(&mut ops, &mut state).expect("init should succeed");
                let _guard = PanicRestoreGuard {
                    ops,
                    terminal: Some(terminal),
                    state,
                };

                panic!("simulate panic");
            }
        }));

        assert!(result.is_err());
        assert_eq!(
            log.lock().expect("log should lock").clone(),
            vec![
                "enable_raw_mode",
                "create_inline_terminal",
                "height_4", // Minimum viewport height
                "show_cursor",
                "disable_raw_mode",
            ]
        );
    }

    #[test]
    fn calculate_inline_height_returns_minimum_viewport() {
        assert_eq!(calculate_inline_height(5), 4);
        assert_eq!(calculate_inline_height(20), 4);
        assert_eq!(calculate_inline_height(100), 4);
    }
}
