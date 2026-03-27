use rig::completion::Usage;
use serde::{Deserialize, Serialize};

/// Result of a single agent turn (user input + agent response)
///
/// This struct represents one complete turn of conversation with the agent.
/// It captures both the user's input and the agent's response, making it
/// easy for the session layer to commit the turn to history on success.
///
/// Future extensions (Phase 02+) may include:
/// - tool_calls: Vec<ToolCall>
/// - tool_results: Vec<ToolResult>
#[derive(Debug, Clone)]
pub struct AgentTurn {
    /// The user's input for this turn
    pub user_input: String,
    /// The agent's text response
    pub response: String,
    /// Token usage from the LLM provider for this turn
    pub usage: Option<Usage>,
}

/// Conversation role
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    System,
    User,
    Assistant,
}

/// A single message in the conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

impl Message {
    /// Create a system message
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: content.into(),
        }
    }

    /// Create a user message
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: content.into(),
        }
    }

    /// Create an assistant message
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: content.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_creation() {
        let system = Message::system("You are an AI assistant.");
        assert_eq!(system.role, Role::System);
        assert_eq!(system.content, "You are an AI assistant.");

        let user = Message::user("Hello!");
        assert_eq!(user.role, Role::User);

        let assistant = Message::assistant("Hi there!");
        assert_eq!(assistant.role, Role::Assistant);
    }
}
