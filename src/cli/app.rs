use crate::frontend::{FrontendCommand, FrontendEvent};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::prelude::{Frame, Line};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

#[derive(Debug, Clone, PartialEq, Eq)]
struct TranscriptEntry {
    speaker: &'static str,
    text: String,
}

/// Reducer-owned UI state for the terminal adapter.
pub struct CliApp {
    transcript: Vec<TranscriptEntry>,
    activity: Vec<String>,
    composer: String,
    status: String,
    provider: Option<String>,
    model: Option<String>,
    assistant_stream: String,
    should_exit: bool,
    exit_requested: bool,
}

impl CliApp {
    pub fn new() -> Self {
        Self {
            transcript: Vec::new(),
            activity: Vec::new(),
            composer: String::new(),
            status: "Starting terminal workbench...".to_string(),
            provider: None,
            model: None,
            assistant_stream: String::new(),
            should_exit: false,
            exit_requested: false,
        }
    }

    pub fn apply_event(&mut self, event: FrontendEvent) {
        match event {
            FrontendEvent::SessionStarted { provider, model } => {
                self.provider = Some(provider);
                self.model = Some(model);
                self.status = self.connection_label();
            }
            FrontendEvent::UserMessageCommitted { text } => {
                self.transcript.push(TranscriptEntry {
                    speaker: "You",
                    text,
                });
                self.status = "Waiting for assistant...".to_string();
            }
            FrontendEvent::AssistantMessageDelta { delta } => {
                self.assistant_stream.push_str(&delta);
            }
            FrontendEvent::AssistantMessageCompleted { text } => {
                self.assistant_stream.clear();
                self.transcript.push(TranscriptEntry {
                    speaker: "Agent",
                    text,
                });
                self.status = self.connection_label();
            }
            FrontendEvent::Thinking { text } => {
                self.activity.push(format!("Thinking: {}", text));
            }
            FrontendEvent::ToolCallStarted { name, args_preview } => {
                self.activity
                    .push(format!("Tool start: {}({})", name, args_preview));
            }
            FrontendEvent::ToolCallFinished {
                name,
                result_preview,
            } => {
                self.activity
                    .push(format!("Tool end: {} -> {}", name, result_preview));
            }
            FrontendEvent::RetryScheduled {
                attempt,
                max_retries,
                delay_ms,
                reason,
            } => {
                self.activity.push(format!(
                    "Retry {}/{} in {}ms: {}",
                    attempt, max_retries, delay_ms, reason
                ));
                self.status = "Retry scheduled...".to_string();
            }
            FrontendEvent::Reminder { message } => {
                self.activity.push(format!("Reminder: {}", message));
            }
            FrontendEvent::Status { message } => {
                self.status = message;
            }
            FrontendEvent::Error { message } => {
                self.activity.push(format!("Error: {}", message));
                self.status = "Error".to_string();
            }
            FrontendEvent::SessionEnded { summary } => {
                self.status = format!(
                    "Session ended after {} turns / {} messages",
                    summary.turns, summary.message_count
                );
                self.should_exit = true;
            }
        }
    }

    pub fn handle_key_event(&mut self, key: KeyEvent) -> Option<FrontendCommand> {
        match (key.code, key.modifiers) {
            (KeyCode::Esc, _) => Some(FrontendCommand::Exit),
            (KeyCode::Char('c'), KeyModifiers::CONTROL) => Some(FrontendCommand::Exit),
            (KeyCode::Enter, _) => {
                let message = self.composer.trim().to_string();
                if message.is_empty() {
                    None
                } else {
                    self.composer.clear();
                    Some(FrontendCommand::SubmitMessage(message))
                }
            }
            (KeyCode::Backspace, _) => {
                self.composer.pop();
                None
            }
            (KeyCode::Char(ch), modifiers)
                if !modifiers.contains(KeyModifiers::CONTROL)
                    && !modifiers.contains(KeyModifiers::ALT) =>
            {
                self.composer.push(ch);
                None
            }
            _ => None,
        }
    }

    pub fn render(&self, frame: &mut Frame) {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(10),
                Constraint::Length(3),
            ])
            .split(frame.area());

        let body = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
            .split(layout[1]);

        let status = Paragraph::new(self.status_line())
            .block(Block::default().title("Status").borders(Borders::ALL))
            .wrap(Wrap { trim: false });
        frame.render_widget(status, layout[0]);

        let transcript = Paragraph::new(self.transcript_text())
            .block(Block::default().title("Transcript").borders(Borders::ALL))
            .wrap(Wrap { trim: false });
        frame.render_widget(transcript, body[0]);

        let activity = Paragraph::new(self.activity_text())
            .block(Block::default().title("Activity").borders(Borders::ALL))
            .wrap(Wrap { trim: false });
        frame.render_widget(activity, body[1]);

        let composer = Paragraph::new(self.composer.as_str())
            .style(Style::default().add_modifier(Modifier::BOLD))
            .block(
                Block::default()
                    .title("Composer (Enter submit, Esc exit)")
                    .borders(Borders::ALL),
            )
            .wrap(Wrap { trim: false });
        frame.render_widget(composer, layout[2]);
    }

    pub fn should_exit(&self) -> bool {
        self.should_exit
    }

    pub fn exit_requested(&self) -> bool {
        self.exit_requested
    }

    pub fn mark_exit_requested(&mut self) {
        self.exit_requested = true;
        self.status = "Exiting session...".to_string();
    }

    fn connection_label(&self) -> String {
        match (&self.provider, &self.model) {
            (Some(provider), Some(model)) => format!("Connected: {} / {}", provider, model),
            _ => "Session active".to_string(),
        }
    }

    fn status_line(&self) -> Line<'static> {
        let left = self.status.clone();
        let right = "Ctrl+C or Esc exits".to_string();
        Line::from(format!("{} | {}", left, right))
    }

    fn transcript_text(&self) -> String {
        let mut lines: Vec<String> = self
            .transcript
            .iter()
            .map(|entry| format!("{}: {}", entry.speaker, entry.text))
            .collect();

        if !self.assistant_stream.is_empty() {
            lines.push(format!("Agent (streaming): {}", self.assistant_stream));
        }

        if lines.is_empty() {
            "No messages yet.".to_string()
        } else {
            lines.join("\n\n")
        }
    }

    fn activity_text(&self) -> String {
        if self.activity.is_empty() {
            "No activity yet.".to_string()
        } else {
            self.activity.join("\n\n")
        }
    }
}

impl Default for CliApp {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reducer_updates_transcript_and_activity_panes() {
        let mut app = CliApp::new();
        app.apply_event(FrontendEvent::SessionStarted {
            provider: "anthropic".to_string(),
            model: "claude".to_string(),
        });
        app.apply_event(FrontendEvent::UserMessageCommitted {
            text: "hello".to_string(),
        });
        app.apply_event(FrontendEvent::ToolCallStarted {
            name: "read".to_string(),
            args_preview: "README.md".to_string(),
        });
        app.apply_event(FrontendEvent::AssistantMessageCompleted {
            text: "world".to_string(),
        });

        assert_eq!(app.transcript.len(), 2);
        assert!(app.transcript_text().contains("You: hello"));
        assert!(app.transcript_text().contains("Agent: world"));
        assert!(app.activity_text().contains("Tool start: read(README.md)"));
        assert!(app.status.contains("Connected: anthropic / claude"));
    }

    #[test]
    fn key_handling_maps_submit_and_exit_commands() {
        let mut app = CliApp::new();

        assert_eq!(
            app.handle_key_event(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE)),
            None
        );
        assert_eq!(
            app.handle_key_event(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE)),
            None
        );

        assert_eq!(
            app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            Some(FrontendCommand::SubmitMessage("hi".to_string()))
        );
        assert_eq!(
            app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            Some(FrontendCommand::Exit)
        );
    }
}
