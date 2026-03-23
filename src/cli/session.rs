use crate::agent::{agent_loop, Message};
use crate::error::AgentError;
use reedline::{DefaultPrompt, DefaultPromptSegment, Reedline, Signal};
use std::io::Write;
use tracing::{debug, error, info, warn};

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
        thinking: bool,
        thinking_budget: u64,
    ) -> Result<(), AgentError> {
        let provider = crate::llm::create_provider(provider_type, model, base_url, thinking, thinking_budget)?;
        info!(provider = ?provider_type, model = model, thinking = thinking, "Session initialized");

        println!("Agent Harness v0.1.0");
        println!("Provider: {} | Model: {}", provider_type, model);
        if thinking {
            println!("Extended thinking: enabled (budget: {} tokens)", thinking_budget);
        }
        println!("Type your message and press Enter. Ctrl+C or Ctrl+D to exit.\n");

        let prompt = DefaultPrompt::new(
            DefaultPromptSegment::Basic("You".to_string()),
            DefaultPromptSegment::Empty,
        );

        loop {
            debug!("Waiting for user input");
            let prompt = prompt.clone();
            let result = tokio::task::spawn_blocking(move || {
                Reedline::create().read_line(&prompt)
            })
            .await;

            match result {
                Ok(Ok(Signal::Success(line))) => {
                    let line = line.trim();
                    if line.is_empty() {
                        continue;
                    }

                    // Check for exit commands
                    if line == "exit" || line == "quit" {
                        info!("User requested exit via command");
                        self.print_summary();
                        return Ok(());
                    }

                    info!(input = line, "User input received");

                    print!("Agent: ");
                    std::io::stdout().flush().unwrap();

                    match agent_loop(&self.messages, line, &provider).await {
                        Ok(turn) => {
                            debug!(response_len = turn.response.len(), "Agent response received");
                            println!("{}", turn.response);
                            // Commit the turn to history only on success
                            self.messages.push(Message::user(&turn.user_input));
                            self.messages.push(Message::assistant(&turn.response));
                            self.turn_count += 1;
                        }
                        Err(AgentError::ToolsNotImplemented) => {
                            warn!("Tool use requested but not implemented");
                            eprintln!("\nError: Tool use requested but not implemented (Phase 2 feature)");
                        }
                        Err(e) => {
                            error!(error = %e, "Agent loop error");
                            eprintln!("\nError: {}", e);
                        }
                    }
                }
                Ok(Ok(Signal::CtrlC)) => {
                    info!("User pressed Ctrl+C, exiting");
                    self.print_summary();
                    return Ok(());
                }
                Ok(Ok(Signal::CtrlD)) => {
                    info!("User pressed Ctrl+D, exiting");
                    self.print_summary();
                    return Ok(());
                }
                Ok(Err(err)) => {
                    error!(error = %err, "Reedline error");
                    eprintln!("\nTerminal error: {}", err);
                    eprintln!("This may be a terminal compatibility issue. Trying to continue...\n");

                    if err.to_string().contains("cursor position") {
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    }
                    continue;
                }
                Err(err) => {
                    error!(error = %err, "spawn_blocking join error");
                    eprintln!("Error: {}", err);
                    self.print_summary();
                    return Err(AgentError::Provider(crate::error::ProviderError::RequestFailed(
                        err.to_string(),
                    )));
                }
            }
        }
    }

    /// Print session summary on exit
    fn print_summary(&self) {
        println!("\nSession Summary:");
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
        assert_eq!(session.messages.len(), 1);
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
