use crate::agent::{agent_loop, Message};
use crate::error::AgentError;
use std::io::{self, BufRead, Write};
use tokio::signal::ctrl_c;

/// Interactive REPL session
pub struct Session {
    messages: Vec<Message>,
    turn_count: u32,
}

impl Session {
    /// Create a new session with minimal system prompt
    pub fn new() -> Self {
        Self {
            messages: vec![Message::system(
                "You are an AI agent with the ability to have a conversation. \
                 Respond naturally to user messages.",
            )],
            turn_count: 0,
        }
    }

    /// Run the interactive REPL session
    pub async fn run(
        &mut self,
        provider_type: crate::llm::ProviderType,
        model: &str,
        base_url: Option<&str>,
    ) -> Result<(), AgentError> {
        let provider = crate::llm::create_provider(provider_type, model, base_url)?;

        println!("Agent Harness v0.1.0");
        println!("Provider: {} | Model: {}", provider_type, model);
        println!("Type your message and press Enter. Ctrl+C to exit.\n");

        let stdin = io::stdin();

        loop {
            print!("You: ");
            io::stdout().flush().unwrap();

            let mut input = String::new();

            // Use tokio::select for Ctrl+C handling
            // Note: ctrl_c() is called fresh each iteration to avoid Unpin issues
            tokio::select! {
                _ = ctrl_c() => {
                    self.print_summary();
                    return Ok(());
                }
                result = async {
                    let _ = stdin.lock().read_line(&mut input);
                    input
                } => {
                    input = result;
                }
            }

            let input = input.trim();
            if input.is_empty() {
                continue;
            }

            // Add user message
            self.messages.push(Message::user(input));

            // Run agent loop
            print!("Agent: ");
            match agent_loop(&mut self.messages, &provider).await {
                Ok(response) => {
                    println!("{}", response);
                    self.turn_count += 1;
                }
                Err(AgentError::ToolsNotImplemented) => {
                    eprintln!("\nError: Tool use requested but not implemented (Phase 2 feature)");
                    // Remove the failed message exchange
                    self.messages.pop();
                }
                Err(e) => {
                    eprintln!("\nError: {}", e);
                    // Remove the failed message exchange
                    self.messages.pop();
                }
            }
        }
    }

    /// Print session summary on exit
    fn print_summary(&self) {
        println!("\n\nSession Summary:");
        println!("  Turns: {}", self.turn_count);
        println!("  Messages: {}", self.messages.len());
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_initialization() {
        let session = Session::new();
        assert_eq!(session.messages.len(), 1); // System message
        assert_eq!(session.turn_count, 0);
    }

    #[test]
    fn test_session_default() {
        let session = Session::default();
        assert_eq!(session.messages.len(), 1);
    }

    #[test]
    fn test_system_message_content() {
        let session = Session::new();
        assert!(session.messages[0].content.contains("AI agent"));
    }
}
