use rig::tool::Tool;
use rig::completion::ToolDefinition;
use serde::Deserialize;
use schemars::JsonSchema;
use crate::llm::{create_provider, ProviderType};
use crate::agent::{agent_loop, TodoUsageHook};
use crate::planning::TodoManager;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Arguments for the task (subagent) tool
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct TaskArgs {
    /// The prompt to send to the subagent
    pub prompt: String,
}

/// Error type for subagent operations
#[derive(Debug, thiserror::Error)]
pub enum SubagentError {
    #[error("Subagent execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Provider creation failed: {0}")]
    ProviderError(String),
}

/// Configuration needed to create child agents.
///
/// Holds provider configuration (NOT a provider instance) to prevent
/// recursion -- child agents must not inherit the parent's `task` tool.
#[derive(Debug, Clone, PartialEq)]
pub struct SubagentConfig {
    pub provider_type: ProviderType,
    pub model: String,
    pub base_url: Option<String>,
    pub thinking: bool,
    pub thinking_budget: u64,
    pub max_turns: usize,
}

/// System prompt for child agents.
///
/// Deliberately does NOT mention the `task` tool to prevent
/// recursive subagent spawning. Will be used by create_parent_provider()
/// in the next plan to override the default SYSTEM_PROMPT for child agents.
#[allow(dead_code)]
const CHILD_SYSTEM_PROMPT: &str = r#"You are a subagent. Complete the assigned task using available tools.
You have access to bash, read, write, edit, glob, grep, and todo tools.
Return a clear summary of what you did and what you found."#;

/// Maximum length of result text returned to parent agent.
const MAX_RESULT_LENGTH: usize = 10_000;
const TRUNCATION_SUFFIX: &str = "\n... (truncated)";

/// Tool that spawns an isolated child agent to handle a subtask.
///
/// When the parent LLM calls the `task` tool, this creates a fresh child
/// `LlmProvider` with only the 7 base tools (no `task`), runs an isolated
/// agent loop with empty history, and returns the final text response.
pub struct SubagentTool {
    config: SubagentConfig,
}

impl SubagentTool {
    /// Create a new SubagentTool with the given configuration.
    pub fn new(config: SubagentConfig) -> Self {
        Self { config }
    }
}

impl Tool for SubagentTool {
    const NAME: &'static str = "task";

    type Error = SubagentError;
    type Args = TaskArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "task".to_string(),
            description: "Spawn a subagent with fresh context to handle a subtask. \
                The subagent has access to bash, read, write, edit, glob, grep, and todo tools \
                but cannot spawn further subagents. Only the final summary is returned."
                .to_string(),
            parameters: serde_json::to_value(schemars::schema_for!(TaskArgs))
                .expect("Failed to generate schema for TaskArgs"),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let preview_len = args.prompt.len().min(80);
        tracing::info!(
            "Subagent spawned for prompt: {}...",
            &args.prompt[..preview_len]
        );

        // Create a fresh TodoManager for the child agent.
        // CRITICAL: Child must NOT share parent's TodoManager (Pitfall #2).
        let child_todo = Arc::new(Mutex::new(TodoManager::new()));

        // Create a fresh child provider with only 7 base tools (no task tool).
        let provider = create_provider(
            self.config.provider_type,
            &self.config.model,
            self.config.base_url.as_deref(),
            self.config.thinking,
            self.config.thinking_budget,
            child_todo,
        )
        .map_err(|e| SubagentError::ProviderError(e.to_string()))?;

        // Run agent_loop with empty history -- key context isolation (CROSS-03).
        // Child starts with no parent context, only its assigned prompt.
        match agent_loop(
            &[],
            &args.prompt,
            &provider,
            TodoUsageHook::default(),
        )
        .await
        {
            Ok(turn) => {
                // Log child token usage if available
                if let Some(usage) = &turn.usage {
                    tracing::info!(
                        "Subagent token usage: input={}, output={}",
                        usage.input_tokens,
                        usage.output_tokens
                    );
                }

                // Truncate result if too long to prevent context explosion
                let result = if turn.response.len() > MAX_RESULT_LENGTH {
                    let mut truncated: String =
                        turn.response.chars().take(MAX_RESULT_LENGTH).collect();
                    truncated.push_str(TRUNCATION_SUFFIX);
                    truncated
                } else {
                    turn.response
                };

                Ok(result)
            }
            Err(e) => {
                // Error isolation pattern (CROSS-02):
                // Return Ok() with error text so parent LLM can decide what to do.
                // Never propagate Err() which would crash the parent's tool loop.
                tracing::warn!("Subagent failed: {}", e);
                Ok(format!("Subagent error: {}", e))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_args_deserialize() {
        let json = r#"{"prompt": "do something"}"#;
        let args: TaskArgs = serde_json::from_str(json).expect("deserialization should succeed");
        assert_eq!(args.prompt, "do something");
    }

    #[test]
    fn test_task_args_schema_generation() {
        let schema = schemars::schema_for!(TaskArgs);
        let schema_value = serde_json::to_value(&schema).expect("schema serialization");
        assert!(schema_value.is_object());
    }

    #[test]
    fn test_subagent_config_fields() {
        let config = SubagentConfig {
            provider_type: ProviderType::Anthropic,
            model: "claude-3-5-sonnet-20241022".to_string(),
            base_url: None,
            thinking: false,
            thinking_budget: 0,
            max_turns: 30,
        };
        assert_eq!(config.provider_type, ProviderType::Anthropic);
        assert_eq!(config.model, "claude-3-5-sonnet-20241022");
        assert!(config.base_url.is_none());
        assert_eq!(config.max_turns, 30);
    }

    #[test]
    fn test_subagent_config_clone() {
        let config = SubagentConfig {
            provider_type: ProviderType::Anthropic,
            model: "claude-3-5-sonnet-20241022".to_string(),
            base_url: Some("http://localhost:8080".to_string()),
            thinking: true,
            thinking_budget: 5000,
            max_turns: 30,
        };
        let cloned = config.clone();
        assert_eq!(config, cloned);
    }

    #[test]
    fn test_subagent_error_display() {
        let err = SubagentError::ExecutionFailed("test error".to_string());
        assert!(err.to_string().contains("test error"));

        let err = SubagentError::ProviderError("bad config".to_string());
        assert!(err.to_string().contains("bad config"));
    }

    #[tokio::test]
    async fn test_subagent_tool_definition() {
        let config = SubagentConfig {
            provider_type: ProviderType::Anthropic,
            model: "claude-3-5-sonnet-20241022".to_string(),
            base_url: None,
            thinking: false,
            thinking_budget: 0,
            max_turns: 30,
        };
        let tool = SubagentTool::new(config);
        let definition = tool.definition("test".to_string()).await;
        assert_eq!(definition.name, "task");
        assert!(definition.description.contains("subagent"));
        assert!(definition.parameters.is_object());
    }

    #[test]
    fn test_truncation_constants() {
        assert_eq!(MAX_RESULT_LENGTH, 10_000);
        assert_eq!(TRUNCATION_SUFFIX, "\n... (truncated)");
    }

    #[test]
    fn test_result_truncation_logic() {
        // Simulate truncation logic
        let long_response: String = "x".repeat(15_000);
        let result = if long_response.len() > MAX_RESULT_LENGTH {
            let mut truncated: String = long_response.chars().take(MAX_RESULT_LENGTH).collect();
            truncated.push_str(TRUNCATION_SUFFIX);
            truncated
        } else {
            long_response
        };
        assert!(result.len() > MAX_RESULT_LENGTH);
        assert!(result.len() < 15_000);
        assert!(result.ends_with(TRUNCATION_SUFFIX));
    }

    #[test]
    fn test_short_result_not_truncated() {
        let short_response = "Hello from subagent".to_string();
        let result = if short_response.len() > MAX_RESULT_LENGTH {
            let mut truncated: String = short_response.chars().take(MAX_RESULT_LENGTH).collect();
            truncated.push_str(TRUNCATION_SUFFIX);
            truncated
        } else {
            short_response.clone()
        };
        assert_eq!(result, short_response);
    }
}
