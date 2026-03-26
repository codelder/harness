use crate::frontend::{FrontendCommand, FrontendEvent, FrontendTodoItem, FrontendTodoStatus};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::prelude::{Frame, Line, Terminal};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use unicode_width::UnicodeWidthStr;

const APP_BANNER: &str = "Harness";
const SCROLL_STEP: u16 = 1;
const PAGE_SCROLL_STEP: u16 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ViewportState {
    offset: u16,
    follow_tail: bool,
}

impl ViewportState {
    fn new() -> Self {
        Self {
            offset: 0,
            follow_tail: true,
        }
    }

    fn scroll_up(&mut self, lines: u16) {
        self.offset = self.offset.saturating_sub(lines.max(1));
        self.follow_tail = false;
    }

    fn scroll_down(&mut self, lines: u16) {
        self.offset = self.offset.saturating_add(lines.max(1));
        self.follow_tail = false;
    }

    fn scroll_home(&mut self) {
        self.offset = 0;
        self.follow_tail = false;
    }

    fn scroll_end(&mut self) {
        self.follow_tail = true;
    }

    fn resolve(&mut self, max_scroll: u16) -> u16 {
        let resolved = if self.follow_tail {
            max_scroll
        } else {
            self.offset.min(max_scroll)
        };
        self.offset = resolved;
        self.follow_tail = resolved >= max_scroll;
        resolved
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ToolBlock {
    turn_id: u64,
    call_id: String,
    name: String,
    args_preview: String,
    result_preview: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NoteKind {
    Reminder,
    Error,
    Status,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TimelineBlock {
    UserMessage { turn_id: u64, text: String },
    AssistantMessage { turn_id: u64, text: String },
    Thinking { turn_id: u64, text: String },
    Note {
        turn_id: Option<u64>,
        kind: NoteKind,
        message: String,
    },
    Tool(ToolBlock),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StreamingAssistant {
    turn_id: u64,
    text: String,
}

/// Reducer-owned UI state for the terminal adapter.
pub struct CliApp {
    timeline: Vec<TimelineBlock>,
    todo_footer: Vec<FrontendTodoItem>,
    composer: String,
    status: String,
    provider: Option<String>,
    model: Option<String>,
    viewport: ViewportState,
    streaming_assistant: Option<StreamingAssistant>,
    should_exit: bool,
    exit_requested: bool,
}

impl CliApp {
    pub fn new() -> Self {
        Self {
            timeline: Vec::new(),
            todo_footer: Vec::new(),
            composer: String::new(),
            status: "Starting session...".to_string(),
            provider: None,
            model: None,
            viewport: ViewportState::new(),
            streaming_assistant: None,
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
            FrontendEvent::UserMessageCommitted { turn_id, text } => {
                self.streaming_assistant = None;
                self.timeline
                    .push(TimelineBlock::UserMessage { turn_id, text });
                self.status = "Waiting for assistant...".to_string();
                self.viewport.scroll_end();
            }
            FrontendEvent::AssistantMessageDelta { turn_id, delta } => {
                if delta.is_empty() {
                    return;
                }

                match &mut self.streaming_assistant {
                    Some(streaming) if streaming.turn_id == turn_id => {
                        streaming.text.push_str(&delta);
                    }
                    _ => {
                        self.streaming_assistant = Some(StreamingAssistant {
                            turn_id,
                            text: delta,
                        });
                    }
                }
                self.viewport.scroll_end();
            }
            FrontendEvent::AssistantMessageCompleted { turn_id, text } => {
                self.streaming_assistant = None;
                self.timeline
                    .push(TimelineBlock::AssistantMessage { turn_id, text });
                self.status = self.connection_label();
                self.viewport.scroll_end();
            }
            FrontendEvent::Thinking { turn_id, text } => {
                self.timeline.push(TimelineBlock::Thinking { turn_id, text });
                self.viewport.scroll_end();
            }
            FrontendEvent::ToolCallStarted {
                turn_id,
                call_id,
                name,
                args_preview,
            } => {
                self.timeline.push(TimelineBlock::Tool(ToolBlock {
                    turn_id,
                    call_id,
                    name,
                    args_preview,
                    result_preview: None,
                }));
                self.viewport.scroll_end();
            }
            FrontendEvent::ToolCallFinished {
                turn_id,
                call_id,
                name,
                result_preview,
            } => {
                if let Some(tool) = self.timeline.iter_mut().rev().find_map(|block| match block {
                    TimelineBlock::Tool(tool)
                        if tool.turn_id == turn_id && tool.call_id == call_id =>
                    {
                        Some(tool)
                    }
                    _ => None,
                }) {
                    tool.result_preview = Some(result_preview);
                } else {
                    self.timeline.push(TimelineBlock::Tool(ToolBlock {
                        turn_id,
                        call_id,
                        name,
                        args_preview: String::new(),
                        result_preview: Some(result_preview),
                    }));
                }
                self.viewport.scroll_end();
            }
            FrontendEvent::RetryScheduled {
                turn_id,
                attempt,
                max_retries,
                delay_ms,
                reason,
            } => {
                self.timeline.push(TimelineBlock::Note {
                    turn_id: Some(turn_id),
                    kind: NoteKind::Status,
                    message: format!(
                        "Retry {}/{} in {}ms: {}",
                        attempt, max_retries, delay_ms, reason
                    ),
                });
                self.status = "Retry scheduled...".to_string();
                self.viewport.scroll_end();
            }
            FrontendEvent::Reminder { turn_id, message } => {
                self.timeline.push(TimelineBlock::Note {
                    turn_id: Some(turn_id),
                    kind: NoteKind::Reminder,
                    message,
                });
                self.viewport.scroll_end();
            }
            FrontendEvent::TodoSnapshot { items, .. } => {
                self.todo_footer = items;
            }
            FrontendEvent::Status { message } => {
                self.timeline.push(TimelineBlock::Note {
                    turn_id: None,
                    kind: NoteKind::Status,
                    message: message.clone(),
                });
                self.status = message;
                self.viewport.scroll_end();
            }
            FrontendEvent::Error { turn_id, message } => {
                self.timeline.push(TimelineBlock::Note {
                    turn_id: Some(turn_id),
                    kind: NoteKind::Error,
                    message,
                });
                self.status = "Error".to_string();
                self.viewport.scroll_end();
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
            (KeyCode::Char('c'), modifiers) if modifiers.contains(KeyModifiers::CONTROL) => {
                Some(FrontendCommand::Interrupt)
            }
            (KeyCode::Up, _) => {
                self.viewport.scroll_up(SCROLL_STEP);
                None
            }
            (KeyCode::Down, _) => {
                self.viewport.scroll_down(SCROLL_STEP);
                None
            }
            (KeyCode::PageUp, _) => {
                self.viewport.scroll_up(PAGE_SCROLL_STEP);
                None
            }
            (KeyCode::PageDown, _) => {
                self.viewport.scroll_down(PAGE_SCROLL_STEP);
                None
            }
            (KeyCode::Home, _) => {
                self.viewport.scroll_home();
                None
            }
            (KeyCode::End, _) => {
                self.viewport.scroll_end();
                None
            }
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

    pub fn render(&mut self, frame: &mut Frame) {
        let [banner_area, display_area, composer_area, status_area] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Min(0),
                Constraint::Length(3),
                Constraint::Length(1),
            ])
            .split(frame.area())
            .as_ref()
            .try_into()
            .expect("fixed layout");

        let footer_text = self.todo_footer_text();
        let footer_height = footer_height(&footer_text, display_area);
        let [timeline_area, footer_area] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(footer_height)])
            .split(display_area)
            .as_ref()
            .try_into()
            .expect("display layout");

        let timeline_text = self.timeline_text();
        let max_scroll = max_scroll_for_text(&timeline_text, timeline_area, 0);
        let scroll = self.viewport.resolve(max_scroll);
        let timeline_render_area = bottom_align_area(
            timeline_area,
            visible_text_height(&timeline_text, timeline_area.width, 0),
        );

        let banner = Paragraph::new(APP_BANNER)
            .style(Style::default().add_modifier(Modifier::BOLD));
        frame.render_widget(banner, banner_area);

        let timeline = Paragraph::new(timeline_text)
            .scroll((scroll, 0))
            .wrap(Wrap { trim: false });
        frame.render_widget(timeline, timeline_render_area);

        if footer_height > 0 {
            let footer = Paragraph::new(footer_text)
                .block(Block::default().title("Todo").borders(Borders::TOP))
                .wrap(Wrap { trim: false });
            frame.render_widget(footer, footer_area);
        }

        let composer = Paragraph::new(self.composer_text())
            .style(Style::default().add_modifier(Modifier::BOLD))
            .block(Block::default().borders(Borders::TOP))
            .wrap(Wrap { trim: false });
        frame.render_widget(composer, composer_area);

        let status = Paragraph::new(self.status_line());
        frame.render_widget(status, status_area);
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
        Line::from(format!(
            "{} | Enter submit | Ctrl+C interrupt | Esc exit | Scroll Up/Down PgUp/PgDn Home/End",
            self.status
        ))
    }

    fn composer_text(&self) -> String {
        if self.composer.is_empty() {
            "> Type a message...".to_string()
        } else {
            format!("> {}", self.composer)
        }
    }

    fn timeline_text(&self) -> String {
        let mut parts = self
            .timeline
            .iter()
            .map(render_timeline_block)
            .collect::<Vec<_>>();

        if let Some(streaming) = &self.streaming_assistant {
            parts.push(format!("Assistant [{}]\n{}", streaming.turn_id, streaming.text));
        }

        if parts.is_empty() {
            "Start a conversation below.".to_string()
        } else {
            parts.join("\n\n")
        }
    }

    fn todo_footer_text(&self) -> String {
        if self.todo_footer.is_empty() {
            return String::new();
        }

        let mut lines = self
            .todo_footer
            .iter()
            .map(|item| format!("{} #{} {}", todo_marker(item.status), item.id, item.text))
            .collect::<Vec<_>>();
        let done = self
            .todo_footer
            .iter()
            .filter(|item| item.status == FrontendTodoStatus::Completed)
            .count();
        lines.push(String::new());
        lines.push(format!("({}/{})", done, self.todo_footer.len()));
        lines.join("\n")
    }
}

impl Default for CliApp {
    fn default() -> Self {
        Self::new()
    }
}

fn render_timeline_block(block: &TimelineBlock) -> String {
    match block {
        TimelineBlock::UserMessage { turn_id, text } => format!("You [{}]\n{}", turn_id, text),
        TimelineBlock::AssistantMessage { turn_id, text } => {
            format!("Assistant [{}]\n{}", turn_id, text)
        }
        TimelineBlock::Thinking { turn_id, text } => {
            format!("Thinking [{}]\n{}", turn_id, text)
        }
        TimelineBlock::Note {
            turn_id,
            kind,
            message,
        } => {
            let label = match kind {
                NoteKind::Reminder => "Reminder",
                NoteKind::Error => "Error",
                NoteKind::Status => "Status",
            };
            match turn_id {
                Some(turn_id) => format!("{} [{}]\n{}", label, turn_id, message),
                None => format!("{}\n{}", label, message),
            }
        }
        TimelineBlock::Tool(tool) => {
            let mut lines = vec![format!(
                "Tool [{}:{}] {}",
                tool.turn_id, tool.call_id, tool.name
            )];
            if !tool.args_preview.is_empty() {
                lines.push(format!("args: {}", tool.args_preview));
            }
            if let Some(result_preview) = &tool.result_preview {
                lines.push(format!("result: {}", result_preview));
            }
            lines.join("\n")
        }
    }
}

fn todo_marker(status: FrontendTodoStatus) -> &'static str {
    match status {
        FrontendTodoStatus::Pending => "[ ]",
        FrontendTodoStatus::InProgress => "[>]",
        FrontendTodoStatus::Completed => "[x]",
    }
}

fn bottom_align_area(area: Rect, content_height: u16) -> Rect {
    if content_height == 0 || content_height >= area.height {
        area
    } else {
        Rect {
            x: area.x,
            y: area.y + area.height - content_height,
            width: area.width,
            height: content_height,
        }
    }
}

fn footer_height(text: &str, area: Rect) -> u16 {
    if text.is_empty() {
        0
    } else {
        visible_text_height(text, area.width, 2)
    }
}

fn max_scroll_for_text(text: &str, area: Rect, vertical_chrome: u16) -> u16 {
    let visible_height = area.height.saturating_sub(vertical_chrome);
    if visible_height == 0 {
        return 0;
    }

    wrapped_line_count(text, area.width).saturating_sub(visible_height)
}

fn visible_text_height(text: &str, width: u16, vertical_chrome: u16) -> u16 {
    wrapped_line_count(text, width.saturating_sub(0)).saturating_add(vertical_chrome)
}

fn wrapped_line_count(text: &str, width: u16) -> u16 {
    if width == 0 {
        return 0;
    }

    let width = usize::from(width);
    let mut total = 0usize;

    for line in text.lines() {
        let line_width = UnicodeWidthStr::width(line);
        total += std::cmp::max(1, line_width.div_ceil(width));
    }

    if total == 0 {
        1
    } else {
        total.min(u16::MAX as usize) as u16
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render_buffer(app: &mut CliApp, width: u16, height: u16) -> Buffer {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| app.render(frame))
            .expect("render should succeed");
        terminal.backend().buffer().clone()
    }

    fn buffer_text(buffer: &Buffer) -> Vec<String> {
        (0..buffer.area.height)
            .map(|row| {
                (0..buffer.area.width)
                    .map(|col| buffer[(col, row)].symbol())
                    .collect::<String>()
            })
            .collect()
    }

    #[test]
    fn tool_group_and_todo_footer() {
        let mut app = CliApp::new();
        app.apply_event(FrontendEvent::ToolCallStarted {
            turn_id: 1,
            call_id: "read-1".to_string(),
            name: "read".to_string(),
            args_preview: "README.md".to_string(),
        });
        app.apply_event(FrontendEvent::ToolCallFinished {
            turn_id: 1,
            call_id: "read-1".to_string(),
            name: "read".to_string(),
            result_preview: "ok".to_string(),
        });
        app.apply_event(FrontendEvent::TodoSnapshot {
            turn_id: 1,
            items: vec![FrontendTodoItem {
                id: 1,
                text: "Review output".to_string(),
                status: FrontendTodoStatus::Pending,
            }],
        });

        assert_eq!(app.timeline.len(), 1);
        assert!(matches!(
            &app.timeline[0],
            TimelineBlock::Tool(ToolBlock {
                turn_id: 1,
                call_id,
                result_preview: Some(result),
                ..
            }) if call_id == "read-1" && result == "ok"
        ));
        assert_eq!(app.todo_footer.len(), 1);
    }

    #[test]
    fn same_turn_out_of_order_completion_updates_correct_tool_block() {
        let mut app = CliApp::new();
        app.apply_event(FrontendEvent::ToolCallStarted {
            turn_id: 2,
            call_id: "a".to_string(),
            name: "read".to_string(),
            args_preview: "A".to_string(),
        });
        app.apply_event(FrontendEvent::ToolCallStarted {
            turn_id: 2,
            call_id: "b".to_string(),
            name: "grep".to_string(),
            args_preview: "B".to_string(),
        });
        app.apply_event(FrontendEvent::ToolCallFinished {
            turn_id: 2,
            call_id: "b".to_string(),
            name: "grep".to_string(),
            result_preview: "second".to_string(),
        });
        app.apply_event(FrontendEvent::ToolCallFinished {
            turn_id: 2,
            call_id: "a".to_string(),
            name: "read".to_string(),
            result_preview: "first".to_string(),
        });

        let text = app.timeline_text();
        assert!(text.contains("Tool [2:a] read\nargs: A\nresult: first"));
        assert!(text.contains("Tool [2:b] grep\nargs: B\nresult: second"));
    }

    #[test]
    fn duplicate_call_ids_across_turns_stay_isolated() {
        let mut app = CliApp::new();
        for turn_id in [3, 4] {
            app.apply_event(FrontendEvent::ToolCallStarted {
                turn_id,
                call_id: "same".to_string(),
                name: "read".to_string(),
                args_preview: format!("turn-{turn_id}"),
            });
        }
        app.apply_event(FrontendEvent::ToolCallFinished {
            turn_id: 4,
            call_id: "same".to_string(),
            name: "read".to_string(),
            result_preview: "turn-4-result".to_string(),
        });

        let first = match &app.timeline[0] {
            TimelineBlock::Tool(tool) => tool,
            other => panic!("expected tool block, got {other:?}"),
        };
        let second = match &app.timeline[1] {
            TimelineBlock::Tool(tool) => tool,
            other => panic!("expected tool block, got {other:?}"),
        };

        assert_eq!(first.turn_id, 3);
        assert_eq!(first.result_preview, None);
        assert_eq!(second.turn_id, 4);
        assert_eq!(second.result_preview.as_deref(), Some("turn-4-result"));
    }

    #[test]
    fn todo_snapshots_replace_footer_without_adding_timeline_rows() {
        let mut app = CliApp::new();
        app.apply_event(FrontendEvent::UserMessageCommitted {
            turn_id: 1,
            text: "hello".to_string(),
        });
        let timeline_len = app.timeline.len();

        app.apply_event(FrontendEvent::TodoSnapshot {
            turn_id: 1,
            items: vec![FrontendTodoItem {
                id: 1,
                text: "First".to_string(),
                status: FrontendTodoStatus::Pending,
            }],
        });
        app.apply_event(FrontendEvent::TodoSnapshot {
            turn_id: 1,
            items: vec![FrontendTodoItem {
                id: 2,
                text: "Second".to_string(),
                status: FrontendTodoStatus::Completed,
            }],
        });

        assert_eq!(app.timeline.len(), timeline_len);
        assert_eq!(app.todo_footer.len(), 1);
        assert_eq!(app.todo_footer[0].id, 2);
    }

    #[test]
    fn key_handling_maps_interrupt_and_exit_commands() {
        let mut app = CliApp::new();

        assert_eq!(
            app.handle_key_event(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            Some(FrontendCommand::Interrupt)
        );
        assert_eq!(
            app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            Some(FrontendCommand::Exit)
        );
    }

    #[test]
    fn render_keeps_empty_state_close_to_composer() {
        let mut app = CliApp::new();
        let buffer = render_buffer(&mut app, 40, 12);
        let lines = buffer_text(&buffer);

        assert!(lines.iter().any(|line| line.contains(APP_BANNER)));
        let empty_state_row = lines
            .iter()
            .position(|line| line.contains("Start a conversation below."))
            .expect("empty state should render");
        let composer_row = lines
            .iter()
            .position(|line| line.contains("> Type a message..."))
            .expect("composer should render");
        assert!(composer_row.saturating_sub(empty_state_row) <= 3);
    }

    #[test]
    fn render_keeps_todo_footer_at_bottom() {
        let mut app = CliApp::new();
        app.apply_event(FrontendEvent::TodoSnapshot {
            turn_id: 1,
            items: vec![FrontendTodoItem {
                id: 1,
                text: "Pinned footer".to_string(),
                status: FrontendTodoStatus::InProgress,
            }],
        });

        let buffer = render_buffer(&mut app, 40, 14);
        let lines = buffer_text(&buffer);
        let footer_row = lines
            .iter()
            .position(|line| line.contains("Pinned footer"))
            .expect("footer text should render");
        assert!(footer_row >= 8);
    }

    #[test]
    fn overflow_and_resize_clamp_viewport() {
        let mut app = CliApp::new();
        for turn_id in 1..=20 {
            app.apply_event(FrontendEvent::AssistantMessageCompleted {
                turn_id,
                text: format!("line {turn_id}"),
            });
        }

        app.viewport.scroll_home();
        let small_buffer = render_buffer(&mut app, 30, 8);
        let small_lines = buffer_text(&small_buffer);
        assert!(small_lines.iter().any(|line| line.contains("line 1")));

        app.viewport.scroll_end();
        let large_buffer = render_buffer(&mut app, 30, 14);
        let large_lines = buffer_text(&large_buffer);
        assert!(large_lines.iter().any(|line| line.contains("line 20")));
        assert!(app.viewport.follow_tail);
    }
}
