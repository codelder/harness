use rig::tool::Tool;
use rig::completion::ToolDefinition;
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;
use std::process::Command;

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

    #[error("Invalid UTF-8 in command output")]
    InvalidUtf8,
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
"#.to_string(),
            parameters: serde_json::to_value(schemars::schema_for!(BashArgs))
                .expect("Failed to generate schema for BashArgs"),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        tracing::info!("Executing bash command: {}", args.command);

        let output = Command::new("bash")
            .arg("-c")
            .arg(&args.command)
            .output()
            .map_err(|e| BashError::ExecutionFailed(e.to_string()))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        let result = if output.status.success() {
            if stdout.is_empty() && stderr.is_empty() {
                "Command completed successfully (no output)".to_string()
            } else if stdout.is_empty() {
                format!("stderr: {}", stderr.trim())
            } else {
                stdout.trim().to_string()
            }
        } else {
            match output.status.code() {
                Some(code) => {
                    let mut msg = format!("Command failed with exit code {}", code);
                    if !stdout.is_empty() {
                        msg.push_str(&format!("\nstdout: {}", stdout.trim()));
                    }
                    if !stderr.is_empty() {
                        msg.push_str(&format!("\nstderr: {}", stderr.trim()));
                    }
                    msg
                }
                None => return Err(BashError::Terminated),
            }
        };

        tracing::debug!("Command output: {}", result);
        Ok(result)
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
        let result = tool.call(args).await.unwrap();
        assert!(result.contains("exit code 1"));
    }

    #[tokio::test]
    async fn test_bash_tool_definition() {
        let tool = BashTool;
        let definition = tool.definition("test".to_string()).await;
        assert_eq!(definition.name, "bash");
        assert!(definition.description.contains("Execute bash"));
    }
}
