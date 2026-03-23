use crate::agent::{agent_loop, Message, TodoUsageHook};
use crate::error::AgentError;
use crate::planning::TodoManager;
use reedline::{DefaultPrompt, DefaultPromptSegment, Reedline, Signal};
use std::io::Write;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

/// Interactive REPL session
pub struct Session {
    messages: Vec<Message>,
    turn_count: u32,
    todo_manager: Arc<Mutex<TodoManager>>,
    rounds_since_todo: u32,
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
            todo_manager: Arc::new(Mutex::new(TodoManager::new())),
            rounds_since_todo: 0,
        }
    }

    /// Get a clone of the shared TodoManager for tool creation
    pub fn todo_manager(&self) -> Arc<Mutex<TodoManager>> {
        self.todo_manager.clone()
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
        let provider = crate::llm::create_provider(
            provider_type,
            model,
            base_url,
            thinking,
            thinking_budget,
            self.todo_manager.clone(),
        )?;
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

                    // Create hook for this turn (flag starts false)
                    let (hook, used_todo_flag) = TodoUsageHook::new();

                    match agent_loop(&self.messages, line, &provider, hook).await {
                        Ok(turn) => {
                            debug!(response_len = turn.response.len(), "Agent response received");

                            // Check if todo was used (directly from dispatch via hook)
                            // This is equivalent to Python's: rounds_since_todo = 0 if used_todo else rounds_since_todo + 1
                            if used_todo_flag.load(Ordering::SeqCst) {
                                self.rounds_since_todo = 0;
                                used_todo_flag.store(false, Ordering::SeqCst); // Reset for next turn
                            }

                            // Build the response with optional nag reminder
                            // Per Python reference: inject reminder into response for model visibility
                            let response = if self.rounds_since_todo >= 3 {
                                let manager = self.todo_manager.lock().await;
                                if !manager.is_empty() {
                                    format!(
                                        "<reminder>You have pending todos. Use the 'todo' tool to update your task list.</reminder>\n\n{}",
                                        turn.response
                                    )
                                } else {
                                    turn.response.clone()
                                }
                            } else {
                                turn.response.clone()
                            };

                            println!("{}", response);

                            // Increment round counter after each agent response
                            self.rounds_since_todo += 1;

                            // Commit the turn to history only on success
                            // Store the original response (without reminder) in history
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

    #[test]
    fn test_session_rounds_since_todo_initializes_to_zero() {
        let session = Session::new();
        assert_eq!(session.rounds_since_todo, 0);
    }

    #[tokio::test]
    async fn test_session_todo_manager_initializes_empty() {
        let session = Session::new();
        let manager = session.todo_manager.lock().await;
        assert!(manager.is_empty());
    }

    #[test]
    fn test_session_todo_manager_accessor_returns_shared_reference() {
        let session = Session::new();
        let manager1 = session.todo_manager();
        let manager2 = session.todo_manager();

        // Both should point to the same underlying manager
        assert!(Arc::ptr_eq(&manager1, &manager2));
    }
}
