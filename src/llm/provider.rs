use crate::error::ProviderError;
use rig::agent::Agent;
use rig::client::{CompletionClient, ProviderClient};
use rig::completion::{Chat, Message, PromptError};
use rig::providers::{anthropic, openai};

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
    Openai(Agent<openai::completion::CompletionModel>),
    // Ollama support will be added when rig-core supports it natively
    // Currently using placeholder for future implementation
    Ollama,
}

impl LlmProvider {
    /// Send a chat message to the LLM provider
    ///
    /// # Arguments
    /// * `prompt` - The prompt message to send
    /// * `chat_history` - Previous conversation history
    ///
    /// # Returns
    /// The LLM's response as a string
    ///
    /// # Errors
    /// Returns PromptError if the request fails
    pub async fn chat(
        &self,
        prompt: impl Into<String>,
        chat_history: Vec<String>,
    ) -> Result<String, PromptError> {
        // For now, we'll use a simple approach where chat_history is treated as user messages
        // In a future iteration, we can add proper message role handling
        let history: Vec<Message> = chat_history
            .into_iter()
            .map(|msg| Message::user(msg))
            .collect();

        match self {
            LlmProvider::Anthropic(agent) => {
                agent.chat(prompt.into(), history).await
            }
            LlmProvider::Openai(agent) => {
                agent.chat(prompt.into(), history).await
            }
            LlmProvider::Ollama => {
                // Placeholder - Ollama support not yet implemented in rig-core 0.31
                // Using CompletionError variant for now
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
///
/// # Returns
/// An LlmProvider enum variant that can be used to interact with the LLM
///
/// # Errors
/// Returns ProviderError if the provider is unknown or API key is missing
pub fn create_provider(provider_type: ProviderType, model: &str) -> Result<LlmProvider, ProviderError> {
    match provider_type {
        ProviderType::Anthropic => {
            // Check for API key before calling from_env() which panics if missing
            std::env::var("ANTHROPIC_API_KEY")
                .map_err(|_| ProviderError::MissingApiKey("anthropic".to_string()))?;

            let client = anthropic::Client::from_env();
            let agent = client.agent(model).build();
            Ok(LlmProvider::Anthropic(agent))
        }
        ProviderType::Openai => {
            // Check for API key before calling from_env() which panics if missing
            std::env::var("OPENAI_API_KEY")
                .map_err(|_| ProviderError::MissingApiKey("openai".to_string()))?;

            let client = openai::Client::from_env();
            // Use legacy completion API instead of responses API
            let agent = client.completions_api().agent(model).build();
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
}
