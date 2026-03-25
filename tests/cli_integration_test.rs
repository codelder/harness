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
    assert_eq!(
        event_rx.recv().await,
        Some(FrontendEvent::SessionStarted {
            provider: "ollama".to_string(),
            model: "test-model".to_string(),
        })
    );
    assert_eq!(
        event_rx.recv().await,
        Some(FrontendEvent::SessionEnded {
            summary: FrontendSessionSummary {
                turns: 0,
                message_count: 1,
            },
        })
    );
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
    assert_eq!(
        event_rx.recv().await,
        Some(FrontendEvent::Status {
            message: "Interrupt requested but not yet implemented".to_string(),
        })
    );
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
