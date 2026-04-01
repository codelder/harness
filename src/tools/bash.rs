use rig::completion::ToolDefinition;
use rig::tool::Tool;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::process::Stdio;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::sync::mpsc;
use crate::frontend::{FrontendEvent};
use crate::frontend::tool_ui::emit_tool_output;
/// Arguments for the Bash tool
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct BashArgs {
    /// The command to execute
    pub command: String,
    /// Optional timeout in seconds (default: 30)
    #[serde(default = "default_timeout")]
    pub timeout: u64,
}

fn default_timeout() -> u64 {
    30
}

/// Error type for Bash tool
#[derive(Debug, thiserror::Error)]
pub enum BashError {
    #[error("Command execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Command timed out after {0} seconds")]
    Timeout(u64),

    #[error("Command was terminated by signal")]
    Terminated,

    #[error("Command blocked by safety filter: {0}")]
    Blocked(String),
}

/// Maximum output size (characters). Truncate beyond this to avoid sending
/// huge payloads to the LLM.
const MAX_OUTPUT_SIZE: usize = 50_000;

/// Truncate output to MAX_OUTPUT_SIZE, appending a summary if truncated.
fn truncate_output(output: &str) -> String {
    if output.len() <= MAX_OUTPUT_SIZE {
        return output.to_string();
    }
    let truncated = &output[..output.floor_char_boundary(MAX_OUTPUT_SIZE)];
    let total_lines = output.lines().count();
    let kept_lines = truncated.lines().count();
    format!(
        "{}\n\n... [{} of {} lines shown, {} bytes truncated]",
        truncated,
        kept_lines,
        total_lines,
        output.len() - truncated.len()
    )
}

/// Bash tool for executing shell commands
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BashTool;

impl Tool for BashTool {
    const NAME: &'static str = "bash";

    type Error = BashError;
    type Args = BashArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "bash".to_string(),
            description: r#"
Execute bash commands on the system.

Usage notes:
- Use this tool to run shell commands for file operations, system info, etc.
- Commands run in the current working directory
- Output includes both stdout and stderr
- Use timeout for long-running commands (default: 30 seconds)
"#
            .to_string(),
            parameters: serde_json::to_value(schemars::schema_for!(BashArgs))
                .expect("Failed to generate schema for BashArgs"),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        tracing::info!(
            "Executing bash command: {} (timeout: {}s)",
            args.command,
            args.timeout
        );

        // Safety blacklist check
        let dangerous_patterns = [
            "rm -rf /",
            "rm -rf /*",
            "sudo",
            "mkfs",
            "fdisk",
            "shutdown",
            "reboot",
            "halt",
            "dd if=",
            "> /dev/sd",
        ];

        for pattern in &dangerous_patterns {
            if args.command.contains(pattern) {
                return Err(BashError::Blocked(format!(
                    "contains '{}'. This operation requires explicit user confirmation.",
                    pattern
                )));
            }
        }

        // Execute with timeout.
        // stdin is set to null to prevent the child from inheriting the parent's
        // raw-mode stdin, which can cause hangs (e.g. crossterm and the child
        // both reading from the same fd).
        let mut cmd = Command::new("bash");
        cmd.arg("-c").arg(&args.command);
        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let mut child = cmd
            .spawn()
            .map_err(|e| BashError::ExecutionFailed(e.to_string()))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| BashError::ExecutionFailed("Failed to capture stdout".to_string()))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| BashError::ExecutionFailed("Failed to capture stderr".to_string()))?;

        let (chunk_tx, mut chunk_rx) = mpsc::unbounded_channel::<(bool, Vec<u8>)>();

        tokio::spawn(read_stream(stdout, false, chunk_tx.clone()));
        tokio::spawn(read_stream(stderr, true, chunk_tx));

        let mut stdout_buf: Vec<u8> = Vec::new();
        let mut stderr_buf: Vec<u8> = Vec::new();

        let timeout = tokio::time::sleep(Duration::from_secs(args.timeout));
        tokio::pin!(timeout);

        let status = loop {
            tokio::select! {
                maybe_chunk = chunk_rx.recv() => {
                    let Some((is_stderr, bytes)) = maybe_chunk else { continue; };
                    if is_stderr {
                        stderr_buf.extend_from_slice(&bytes);
                    } else {
                        stdout_buf.extend_from_slice(&bytes);
                    }

                    let delta = String::from_utf8_lossy(&bytes).to_string();
                    emit_tool_output(delta, is_stderr, |ctx, delta, is_stderr| {
                        FrontendEvent::ToolCallOutputDelta {
                            turn_id: ctx.turn_id,
                            call_id: ctx.call_id,
                            delta,
                            is_err_stream: is_stderr,
                        }
                    }).await;
                }
                status = child.wait() => {
                    match status {
                        Ok(status) => break status,
                        Err(e) => return Err(BashError::ExecutionFailed(e.to_string())),
                    }
                }
                _ = &mut timeout => {
                    let _ = child.kill().await;
                    return Err(BashError::Timeout(args.timeout));
                }
            }
        };

        // Drain any remaining chunks that raced with process exit.
        while let Ok((is_stderr, bytes)) = chunk_rx.try_recv() {
            if is_stderr {
                stderr_buf.extend_from_slice(&bytes);
            } else {
                stdout_buf.extend_from_slice(&bytes);
            }
        }

        let stdout = String::from_utf8_lossy(&stdout_buf);
        let stderr = String::from_utf8_lossy(&stderr_buf);

        let combined = if !stdout.is_empty() && !stderr.is_empty() {
            format!("{}\n{}", stdout.trim(), stderr.trim())
        } else if !stdout.is_empty() {
            stdout.trim().to_string()
        } else if !stderr.is_empty() {
            stderr.trim().to_string()
        } else {
            String::new()
        };

        let result = if status.success() {
            if combined.is_empty() {
                "Command completed successfully (no output)".to_string()
            } else {
                truncate_output(&combined)
            }
        } else {
            let truncated = truncate_output(&combined);
            match status.code() {
                Some(code) => {
                    return Err(BashError::ExecutionFailed(format!(
                        "exit code {}: {}",
                        code, truncated
                    )));
                }
                None => return Err(BashError::Terminated),
            }
        };

        tracing::debug!(
            "Command output ({} bytes): {}",
            result.len(),
            &result[..result.len().min(200)]
        );
        Ok(result)
    }
}

async fn read_stream(
    mut stream: impl tokio::io::AsyncRead + Unpin + Send + 'static,
    is_stderr: bool,
    tx: mpsc::UnboundedSender<(bool, Vec<u8>)>,
) {
    let mut buf = [0u8; 2048];
    loop {
        match stream.read(&mut buf).await {
            Ok(0) => break,
            Ok(n) => {
                let _ = tx.send((is_stderr, buf[..n].to_vec()));
            }
            Err(_) => break,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_bash_tool_echo() {
        let tool = BashTool;
        let args = BashArgs {
            command: "echo 'Hello, World!'".to_string(),
            timeout: 10,
        };
        let result = tool.call(args).await.unwrap();
        assert_eq!(result, "Hello, World!");
    }

    #[tokio::test]
    async fn test_bash_tool_pwd() {
        let tool = BashTool;
        let args = BashArgs {
            command: "pwd".to_string(),
            timeout: 10,
        };
        let result = tool.call(args).await.unwrap();
        assert!(result.contains('/'));
    }

    #[tokio::test]
    async fn test_bash_tool_failure() {
        let tool = BashTool;
        let args = BashArgs {
            command: "exit 1".to_string(),
            timeout: 10,
        };
        let result = tool.call(args).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            BashError::ExecutionFailed(msg) => assert!(msg.contains("exit code 1")),
            other => panic!("Expected ExecutionFailed, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_bash_tool_definition() {
        let tool = BashTool;
        let definition = tool.definition("test".to_string()).await;
        assert_eq!(definition.name, "bash");
        assert!(definition.description.contains("Execute bash"));
    }

    #[tokio::test]
    async fn test_bash_tool_blocks_rm_rf_root() {
        let tool = BashTool;
        let args = BashArgs {
            command: "rm -rf /".to_string(),
            timeout: 10,
        };
        let result = tool.call(args).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("blocked by safety filter"));
        assert!(err.to_string().contains("rm -rf /"));
    }

    #[tokio::test]
    async fn test_bash_tool_blocks_sudo() {
        let tool = BashTool;
        let args = BashArgs {
            command: "sudo apt-get install something".to_string(),
            timeout: 10,
        };
        let result = tool.call(args).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("blocked by safety filter"));
        assert!(err.to_string().contains("sudo"));
    }

    #[tokio::test]
    async fn test_bash_tool_blocks_mkfs() {
        let tool = BashTool;
        let args = BashArgs {
            command: "mkfs.ext4 /dev/sda1".to_string(),
            timeout: 10,
        };
        let result = tool.call(args).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("blocked by safety filter"));
        assert!(err.to_string().contains("mkfs"));
    }

    #[tokio::test]
    async fn test_bash_tool_allows_safe_commands() {
        let tool = BashTool;
        let args = BashArgs {
            command: "ls -la".to_string(),
            timeout: 10,
        };
        let result = tool.call(args).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.contains("total"));
    }

    #[tokio::test]
    async fn test_bash_tool_timeout() {
        let tool = BashTool;
        // Use a command that sleeps for 60 seconds with a 1-second timeout
        let args = BashArgs {
            command: "sleep 60".to_string(),
            timeout: 1,
        };
        let result = tool.call(args).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            BashError::Timeout(secs) => assert_eq!(secs, 1),
            _ => panic!("Expected Timeout error"),
        }
    }
}
