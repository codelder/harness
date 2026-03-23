use crate::error::ProviderError;
use crate::tools::{BashTool, ReadTool, WriteTool, EditTool, GlobTool, GrepTool, TodoTool};
use crate::planning::TodoManager;
use crate::agent::TodoUsageHook;
use std::sync::Arc;
use tokio::sync::Mutex;
use rig::agent::{Agent, AgentBuilder, PromptRequest};
use rig::client::CompletionClient;
use rig::completion::{Chat, Message, PromptError};
use rig::providers::openai::responses_api::ResponsesCompletionModel;
use rig::providers::{anthropic, openai};

/// System prompt for the agent
const SYSTEM_PROMPT: &str = r#"You are an AI agent with access to tools for interacting with the system.

Available tools:
- bash: Execute shell commands
- read: Read file contents
- write: Create or overwrite files
- edit: Perform precise string replacements in files
- glob: Find files matching patterns
- grep: Search file contents with regex
- todo: Track progress on multi-step tasks

Use these tools to interact with the system and accomplish tasks."#;

/// Maximum number of tool-calling turns before returning to the user
const DEFAULT_MAX_TURNS: usize = 50;

/// Supported LLM provider types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderType {
    Anthropic,
    Openai,
    Ollama,
}

impl std::fmt::Display for ProviderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProviderType::Anthropic => write!(f, "anthropic"),
            ProviderType::Openai => write!(f, "openai"),
            ProviderType::Ollama => write!(f, "ollama"),
        }
    }
}

impl std::str::FromStr for ProviderType {
    type Err = ProviderError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "anthropic" => Ok(ProviderType::Anthropic),
            "openai" => Ok(ProviderType::Openai),
            "ollama" => Ok(ProviderType::Ollama),
            _ => Err(ProviderError::UnknownProvider(s.to_string())),
        }
    }
}

/// Enum-based LLM provider supporting multiple backends
///
/// Uses enum dispatch instead of trait objects for:
/// - Zero-cost abstraction (no vtable overhead)
/// - Type-safe with compile-time exhaustiveness checking
/// - All providers known at compile time
pub enum LlmProvider {
    Anthropic(Agent<anthropic::completion::CompletionModel>),
    Openai(Agent<ResponsesCompletionModel>),
    // Ollama support will be added when rig-core supports it natively
    // Currently using placeholder for future implementation
    Ollama,
}

impl LlmProvider {
    /// Send a chat message with conversation history (rig Message type)
    ///
    /// # Arguments
    /// * `prompt` - The prompt message to send
    /// * `chat_history` - Previous conversation history as rig Messages
    ///
    /// # Returns
    /// The LLM's response as a string
    ///
    /// # Errors
    /// Returns PromptError if the request fails
    pub async fn chat_with_history(
        &self,
        prompt: impl Into<String>,
        chat_history: Vec<Message>,
    ) -> Result<String, PromptError> {
        match self {
            LlmProvider::Anthropic(agent) => {
                agent.chat(prompt.into(), chat_history).await
            }
            LlmProvider::Openai(agent) => {
                agent.chat(prompt.into(), chat_history).await
            }
            LlmProvider::Ollama => {
                Err(PromptError::CompletionError(
                    rig::completion::CompletionError::ResponseError(
                        "Ollama provider not yet implemented".to_string()
                    )
                ))
            }
        }
    }

    /// Send a chat message with history and a hook for observing tool calls
    ///
    /// This method uses rig's PromptRequest API with hook support,
    /// which is required for direct tool usage detection.
    ///
    /// # Arguments
    /// * `prompt` - The prompt message to send
    /// * `chat_history` - Previous conversation history as rig Messages
    /// * `hook` - Hook for observing tool calls (e.g., TodoUsageHook)
    ///
    /// # Returns
    /// The LLM's response as a string
    ///
    /// # Errors
    /// Returns PromptError if the request fails
    pub async fn chat_with_history_and_hook(
        &self,
        prompt: impl Into<String>,
        chat_history: Vec<Message>,
        hook: TodoUsageHook,
    ) -> Result<String, PromptError> {
        match self {
            LlmProvider::Anthropic(agent) => {
                // Use PromptRequest with hook instead of agent.chat()
                PromptRequest::from_agent(agent, prompt.into())
                    .with_history(&mut chat_history.clone())
                    .with_hook(hook)
                    .await
            }
            LlmProvider::Openai(agent) => {
                PromptRequest::from_agent(agent, prompt.into())
                    .with_history(&mut chat_history.clone())
                    .with_hook(hook)
                    .await
            }
            LlmProvider::Ollama => {
                Err(PromptError::CompletionError(
                    rig::completion::CompletionError::ResponseError(
                        "Ollama provider not yet implemented".to_string()
                    )
                ))
            }
        }
    }
}

/// Create an LLM provider based on provider type and model
///
/// # Arguments
/// * `provider_type` - The provider type (Anthropic, Openai, Ollama)
/// * `model` - The model identifier (provider-specific)
/// * `base_url` - Optional custom base URL (for proxies or custom endpoints)
/// * `thinking` - Enable extended thinking/reasoning (Anthropic only)
/// * `thinking_budget` - Budget tokens for extended thinking
/// * `todo_manager` - Shared TodoManager for the todo tool
///
/// # Returns
/// An LlmProvider enum variant that can be used to interact with the LLM
///
/// # Errors
/// Returns ProviderError if the provider is unknown or API key is missing
pub fn create_provider(
    provider_type: ProviderType,
    model: &str,
    base_url: Option<&str>,
    thinking: bool,
    thinking_budget: u64,
    todo_manager: Arc<Mutex<TodoManager>>,
) -> Result<LlmProvider, ProviderError> {
    match provider_type {
        ProviderType::Anthropic => {
            // Check for API key before calling builder
            let api_key = std::env::var("HARNESS_ANTHROPIC_KEY")
                .map_err(|_| ProviderError::MissingApiKey("anthropic".to_string()))?;

            let mut builder = anthropic::Client::builder().api_key(api_key);

            // Check for base URL: CLI arg > HARNESS_ANTHROPIC_URL env
            if let Some(url) = base_url {
                builder = builder.base_url(url);
            } else if let Ok(env_url) = std::env::var("HARNESS_ANTHROPIC_URL") {
                builder = builder.base_url(&env_url);
            }

            let client = builder.build()
                .map_err(|e| ProviderError::RequestFailed(format!("Failed to build client: {}", e)))?;

            // Create completion model
            let completion_model = client.completion_model(model);

            // Create TodoTool with shared TodoManager
            let todo_tool = TodoTool::new(todo_manager.clone());

            // Build agent with tools using AgentBuilder
            // Note: max_tokens is required for Anthropic API
            let mut agent_builder = AgentBuilder::new(completion_model)
                .preamble(SYSTEM_PROMPT)
                .tool(BashTool)
                .tool(ReadTool)
                .tool(WriteTool)
                .tool(EditTool)
                .tool(GlobTool)
                .tool(GrepTool)
                .tool(todo_tool)
                .default_max_turns(DEFAULT_MAX_TURNS)
                .max_tokens(4096);

            // Enable extended thinking if requested
            if thinking {
                let thinking_config = serde_json::json!({
                    "thinking": {
                        "type": "enabled",
                        "budget_tokens": thinking_budget
                    }
                });
                agent_builder = agent_builder.additional_params(thinking_config);
            }

            let agent = agent_builder.build();

            Ok(LlmProvider::Anthropic(agent))
        }
        ProviderType::Openai => {
            // Check for API key before calling builder
            let api_key = std::env::var("HARNESS_OPENAI_KEY")
                .map_err(|_| ProviderError::MissingApiKey("openai".to_string()))?;

            let mut builder = openai::Client::builder().api_key(api_key);

            // Check for base URL: CLI arg > HARNESS_OPENAI_URL env
            if let Some(url) = base_url {
                builder = builder.base_url(url);
            } else if let Ok(env_url) = std::env::var("HARNESS_OPENAI_URL") {
                builder = builder.base_url(&env_url);
            }

            let client = builder.build()
                .map_err(|e| ProviderError::RequestFailed(format!("Failed to build client: {}", e)))?;

            // Create completion model using Responses API (default for openai::Client)
            let completion_model = client.completion_model(model);

            // Create TodoTool with shared TodoManager
            let todo_tool = TodoTool::new(todo_manager.clone());

            // Build agent with tools using AgentBuilder
            let agent = AgentBuilder::new(completion_model)
                .preamble(SYSTEM_PROMPT)
                .tool(BashTool)
                .tool(ReadTool)
                .tool(WriteTool)
                .tool(EditTool)
                .tool(GlobTool)
                .tool(GrepTool)
                .tool(todo_tool)
                .default_max_turns(DEFAULT_MAX_TURNS)
                .build();

            Ok(LlmProvider::Openai(agent))
        }
        ProviderType::Ollama => {
            // Ollama support pending - rig-core 0.31 doesn't have native Ollama client
            // Will be implemented when rig-core adds Ollama support
            Ok(LlmProvider::Ollama)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planning::TodoManager;

    #[test]
    fn test_provider_type_display() {
        assert_eq!(ProviderType::Anthropic.to_string(), "anthropic");
        assert_eq!(ProviderType::Openai.to_string(), "openai");
        assert_eq!(ProviderType::Ollama.to_string(), "ollama");
    }

    #[test]
    fn test_provider_type_from_str() {
        assert_eq!("anthropic".parse::<ProviderType>().unwrap(), ProviderType::Anthropic);
        assert_eq!("openai".parse::<ProviderType>().unwrap(), ProviderType::Openai);
        assert_eq!("ollama".parse::<ProviderType>().unwrap(), ProviderType::Ollama);
        assert_eq!("ANTHROPIC".parse::<ProviderType>().unwrap(), ProviderType::Anthropic);
    }

    #[test]
    fn test_unknown_provider() {
        let result: Result<ProviderType, _> = "unknown".parse();
        assert!(result.is_err());
        matches!(result.unwrap_err(), ProviderError::UnknownProvider(_));
    }

    #[tokio::test]
    async fn test_create_anthropic_provider_with_custom_model() {
        // Test that create_provider works with non-standard model names
        // This verifies max_tokens is set correctly (not dependent on model name)
        use std::env;

        // Save original API key
        let original_key = env::var("HARNESS_ANTHROPIC_KEY");

        // Set a dummy API key for testing
        env::set_var("HARNESS_ANTHROPIC_KEY", "test-key-12345");

        // Create a fresh TodoManager for the test
        let todo_manager = Arc::new(Mutex::new(TodoManager::new()));

        // Test with a custom model name (not standard Claude model)
        let result = create_provider(ProviderType::Anthropic, "custom-model-name", None, false, 10000, todo_manager);

        // Restore original API key
        match original_key {
            Ok(val) => env::set_var("HARNESS_ANTHROPIC_KEY", val),
            Err(_) => env::remove_var("HARNESS_ANTHROPIC_KEY"),
        }

        // Provider creation should succeed (max_tokens is set to 4096)
        assert!(result.is_ok(), "Provider creation should succeed with custom model name");
    }
}
