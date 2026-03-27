use harness::{
    frontend_event_channel,
    FrontendCommand,
    FrontendEvent,
    FrontendSessionSummary,
    LlmProvider,
    SessionRuntime,
    SessionRuntimeOutcome,
};
use std::process::Command;

#[allow(dead_code)]
#[path = "../src/cli/terminal.rs"]
mod terminal_impl;

#[tokio::test]
async fn session_start_and_exit_emit_metadata_events() {
    let (event_tx, mut event_rx) = frontend_event_channel(8);
    let mut runtime = SessionRuntime::new();

    runtime
        .start_with_provider(LlmProvider::Ollama, "ollama", "test-model", &event_tx)
        .await
        .unwrap();

    let outcome = runtime
        .handle_command(FrontendCommand::Exit, &event_tx)
        .await
        .unwrap();

    assert_eq!(
        outcome,
        SessionRuntimeOutcome::Exit(FrontendSessionSummary {
            turns: 0,
            message_count: 1,
        })
    );
    assert!(matches!(
        event_rx.recv().await,
        Some(FrontendEvent::SessionStarted {
            provider,
            model,
            working_directory: _,
            version: _,
        }) if provider == "ollama" && model == "test-model"
    ));
    let mut saw_session_end = false;
    while let Some(event) = event_rx.recv().await {
        if let FrontendEvent::SessionEnded {
            summary:
                FrontendSessionSummary {
                    turns: 0,
                    message_count: 1,
                },
        } = event
        {
            saw_session_end = true;
            break;
        }
    }
    assert!(saw_session_end, "session exit should emit SessionEnded");
}

#[tokio::test]
async fn interrupt_command_emits_status_event() {
    let (event_tx, mut event_rx) = frontend_event_channel(8);
    let mut runtime = SessionRuntime::new();

    runtime
        .start_with_provider(LlmProvider::Ollama, "ollama", "test-model", &event_tx)
        .await
        .unwrap();
    let _ = event_rx.recv().await;

    let outcome = runtime
        .handle_command(FrontendCommand::Interrupt, &event_tx)
        .await
        .unwrap();

    assert_eq!(outcome, SessionRuntimeOutcome::Interrupted);
    let mut saw_status = false;
    while let Some(event) = event_rx.recv().await {
        match event {
            FrontendEvent::Status { message }
                if message == "Interrupt requested but not yet implemented" =>
            {
                saw_status = true;
                break;
            }
            _ => {}
        }
    }
    assert!(saw_status, "interrupt should emit status feedback");
}

#[test]
fn harness_help_smoke() {
    let output = Command::new(env!("CARGO_BIN_EXE_harness"))
        .arg("--help")
        .output()
        .expect("failed to run harness --help");

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage") || stdout.contains("harness"));
}

#[test]
fn clean_exit_restore_smoke_without_live_terminal() {
    let log = terminal_impl::tests::terminal_restore_smoke(
        terminal_impl::tests::RestoreScenario::CleanExit,
    )
    .expect("clean exit smoke should succeed");

    assert_eq!(
        log,
        vec![
            "enable_raw_mode",
            "create_inline_terminal",
            "height_22",
            "show_cursor",
            "disable_raw_mode",
        ]
    );
}

#[test]
fn runtime_error_restore_smoke_without_live_model_call() {
    let log = terminal_impl::tests::terminal_restore_smoke(
        terminal_impl::tests::RestoreScenario::RuntimeError,
    )
    .expect("runtime error smoke should succeed");

    assert_eq!(
        log,
        vec![
            "enable_raw_mode",
            "create_inline_terminal",
            "height_22",
            "runtime_error",
            "show_cursor",
            "disable_raw_mode",
        ]
    );
}
