//! Integration tests for CLI behavior
//!
//! These tests require building the binary first and test actual process behavior.
//!
//! NOTE: These tests are ignored by default because they require interactive TTY.
//! Run manually with: cargo test -- --ignored

use std::process::{Command, Stdio};

/// Test that the CLI shows welcome message and prompt
#[test]
#[ignore = "requires interactive TTY"]
fn test_cli_welcome_message() {
    // Build the binary first
    let build_status = Command::new("cargo")
        .args(["build", "--release"])
        .status()
        .expect("Failed to build harness");
    assert!(build_status.success(), "Build failed");

    // Start the harness and immediately pipe in EOF
    let output = Command::new("./target/release/harness")
        .env_remove("HARNESS_ANTHROPIC_KEY")
        .env_remove("HARNESS_OPENAI_KEY")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("Failed to run harness");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Should show welcome message (or error about missing API key)
    let combined = format!("{}{}", stdout, stderr);
    assert!(
        combined.contains("Agent Harness")
            || combined.contains("API key")
            || combined.contains("error"),
        "Expected welcome message or API key error, got: stdout='{}', stderr='{}'",
        stdout,
        stderr
    );
}

/// Test that missing API key shows clear error message
#[test]
#[ignore = "requires interactive TTY"]
fn test_missing_api_key_error() {
    // Build the binary first
    let build_status = Command::new("cargo")
        .args(["build", "--release"])
        .status()
        .expect("Failed to build harness");
    assert!(build_status.success(), "Build failed");

    // Run without API key
    let output = Command::new("./target/release/harness")
        .env_remove("HARNESS_ANTHROPIC_KEY")
        .env_remove("HARNESS_OPENAI_KEY")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("Failed to run harness");

    let stderr = String::from_utf8_lossy(&output.stderr);

    // Should show clear error about missing API key
    assert!(
        stderr.contains("API key") || stderr.contains("Missing"),
        "Expected API key error message, got: {}",
        stderr
    );
}

/// Test that stdin EOF causes graceful exit with session summary
#[test]
#[ignore = "requires interactive TTY"]
fn test_stdin_eof_graceful_exit() {
    // Build the binary first
    let build_status = Command::new("cargo")
        .args(["build", "--release"])
        .status()
        .expect("Failed to build harness");
    assert!(build_status.success(), "Build failed");

    // Run with empty stdin (immediate EOF)
    let output = Command::new("./target/release/harness")
        .env("HARNESS_ANTHROPIC_KEY", "sk-test-dummy-key")
        .stdin(Stdio::null()) // No input = immediate EOF
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("Failed to run harness");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);

    // Should show session summary on EOF exit
    assert!(
        combined.contains("Session Summary") || combined.contains("stdin closed"),
        "Expected graceful exit with session summary on EOF. stdout='{}', stderr='{}'",
        stdout,
        stderr
    );
}

/// Test that Ctrl+C at input prompt exits gracefully (requires expect)
///
/// This test verifies the fix for the bug where stdin.read_line() blocked
/// the tokio runtime, preventing Ctrl+C from being handled at the "You:" prompt.
///
/// Note: This test requires 'expect' to be installed (available by default on macOS).
#[test]
#[ignore = "requires interactive TTY and expect command"]
#[cfg(unix)]
fn test_ctrl_c_graceful_exit() {
    // Build the binary first
    let build_status = Command::new("cargo")
        .args(["build", "--release"])
        .status()
        .expect("Failed to build harness");
    assert!(build_status.success(), "Build failed");

    // Use expect to test interactive Ctrl+C handling
    let expect_script = r#"
set timeout 10
spawn -noecho env HARNESS_ANTHROPIC_KEY=sk-test-dummy ./target/release/harness
expect "You:"
send "\003"
expect {
    "Session Summary" { }
    timeout { puts "FAILED: Timeout waiting for session summary"; exit 1 }
}
expect eof
"#;

    let output = Command::new("expect")
        .arg("-c")
        .arg(expect_script)
        .output()
        .expect("Failed to run expect (is it installed?)");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "Ctrl+C test failed. stdout='{}', stderr='{}'",
        stdout,
        stderr
    );
}
