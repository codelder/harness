use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyEvent, KeyEventKind,
};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
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
    raw_mode_enabled: bool,
    alternate_screen_active: bool,
}

impl TerminalGuard {
    pub fn new() -> io::Result<Self> {
        let mut ops = SystemLifecycleOps;
        let mut state = LifecycleState::default();
        let terminal = initialize_terminal(&mut ops, &mut state)?;

        Ok(Self {
            terminal: Some(terminal),
            raw_mode_enabled: state.raw_mode_enabled,
            alternate_screen_active: state.alternate_screen_active,
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
        let mut ops = SystemLifecycleOps;
        let mut terminal = self.terminal.take();
        let mut state = LifecycleState {
            raw_mode_enabled: self.raw_mode_enabled,
            alternate_screen_active: self.alternate_screen_active,
        };
        let result = restore_terminal(&mut ops, terminal.as_mut(), &mut state);

        self.raw_mode_enabled = state.raw_mode_enabled;
        self.alternate_screen_active = state.alternate_screen_active;

        result
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

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct LifecycleState {
    raw_mode_enabled: bool,
    alternate_screen_active: bool,
}

trait TerminalLifecycleOps {
    type Terminal;

    fn enable_raw_mode(&mut self) -> io::Result<()>;
    fn disable_raw_mode(&mut self) -> io::Result<()>;
    fn enter_alternate_screen(&mut self) -> io::Result<()>;
    fn leave_alternate_screen(&mut self, terminal: Option<&mut Self::Terminal>) -> io::Result<()>;
    fn create_terminal(&mut self) -> io::Result<Self::Terminal>;
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

    fn enter_alternate_screen(&mut self) -> io::Result<()> {
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
    }

    fn leave_alternate_screen(
        &mut self,
        terminal: Option<&mut Self::Terminal>,
    ) -> io::Result<()> {
        match terminal {
            Some(terminal) => execute!(
                terminal.backend_mut(),
                LeaveAlternateScreen,
                DisableMouseCapture
            ),
            None => {
                let mut stdout = io::stdout();
                execute!(stdout, LeaveAlternateScreen, DisableMouseCapture)
            }
        }
    }

    fn create_terminal(&mut self) -> io::Result<Self::Terminal> {
        let backend = CrosstermBackend::new(io::stdout());
        Terminal::new(backend)
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

    if let Err(error) = ops.enter_alternate_screen() {
        let _ = rollback_terminal_setup(ops, state);
        return Err(error);
    }
    state.alternate_screen_active = true;

    match ops.create_terminal() {
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
    mut terminal: Option<&mut Ops::Terminal>,
    state: &mut LifecycleState,
) -> io::Result<()> {
    let mut first_error = None;

    if let Some(active_terminal) = terminal.as_deref_mut() {
        capture_cleanup_error(ops.show_cursor(active_terminal), &mut first_error);
    }

    if state.alternate_screen_active {
        capture_cleanup_error(ops.leave_alternate_screen(terminal), &mut first_error);
        state.alternate_screen_active = false;
    }

    if state.raw_mode_enabled {
        capture_cleanup_error(ops.disable_raw_mode(), &mut first_error);
        state.raw_mode_enabled = false;
    }

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
    }

    impl MockLifecycleOps {
        fn new(log: Arc<Mutex<Vec<&'static str>>>, fail_step: Option<&'static str>) -> Self {
            Self { log, fail_step }
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

        fn enter_alternate_screen(&mut self) -> io::Result<()> {
            self.maybe_fail("enter_alternate_screen")
        }

        fn leave_alternate_screen(
            &mut self,
            _terminal: Option<&mut Self::Terminal>,
        ) -> io::Result<()> {
            self.maybe_fail("leave_alternate_screen")
        }

        fn create_terminal(&mut self) -> io::Result<Self::Terminal> {
            self.maybe_fail("create_terminal")?;
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
            alternate_screen_active: false,
        };
        assert!(guard.restore().is_ok());
        assert!(guard.restore().is_ok());
    }

    #[test]
    fn constructor_rollback_restores_raw_mode_if_screen_entry_fails() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let mut ops = MockLifecycleOps::new(log.clone(), Some("enter_alternate_screen"));
        let mut state = LifecycleState::default();

        let error = initialize_terminal(&mut ops, &mut state).expect_err("init should fail");

        assert!(error.to_string().contains("enter_alternate_screen"));
        assert_eq!(
            log.lock().expect("log should lock").clone(),
            vec![
                "enable_raw_mode",
                "enter_alternate_screen",
                "disable_raw_mode",
            ]
        );
        assert_eq!(state, LifecycleState::default());
    }

    #[test]
    fn constructor_rollback_restores_screen_if_terminal_creation_fails() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let mut ops = MockLifecycleOps::new(log.clone(), Some("create_terminal"));
        let mut state = LifecycleState::default();

        let error = initialize_terminal(&mut ops, &mut state).expect_err("init should fail");

        assert!(error.to_string().contains("create_terminal"));
        assert_eq!(
            log.lock().expect("log should lock").clone(),
            vec![
                "enable_raw_mode",
                "enter_alternate_screen",
                "create_terminal",
                "leave_alternate_screen",
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
                "enter_alternate_screen",
                "create_terminal",
                "show_cursor",
                "leave_alternate_screen",
                "disable_raw_mode",
            ]
        );
    }
}
