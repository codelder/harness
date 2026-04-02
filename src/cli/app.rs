use crate::cli::spinner::Spinner;
use crate::cli::theme::CliTheme;
use crate::frontend::{
    FrontendCommand, FrontendEvent, FrontendSubagentToolUse, FrontendTodoItem,
    FrontendTodoStatus,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::prelude::{Frame, Line, Span};
use ratatui::widgets::{Paragraph, Wrap};
use serde::Deserialize;
use the_other_tui_markdown::{into_text_with_renderer as render_markdown, RendererBuilder};
use unicode_width::UnicodeWidthStr;

const TOOL_PANEL_MAX_ROWS: u16 = 8;
const ACTIVITY_ROWS: u16 = 1;
const COMPOSER_ROWS: u16 = 3;
const STATUS_ROWS: u16 = 1;
const COMPOSER_PROMPT: &str = "> ";
const DEFAULT_VISIBLE_SUBAGENT_TOOLS: usize = 4;
const DEFAULT_VISIBLE_TOOL_RESULT_LINES: usize = 24;
const TASK_PROMPT_SUMMARY_CHARS: usize = 48;
const SUBAGENT_TOOL_ARGS_CHARS: usize = 40;
const TOOL_RESULT_LINE_CHARS: usize = 160;
const BASH_OUTPUT_INLINE_MAX_LINES: usize = 8;
const BASH_OUTPUT_INLINE_MAX_CHARS: usize = 600;
const BASH_OUTPUT_TAIL_LINES: usize = 3;
const BASH_LIVE_OUTPUT_LINES: usize = 4;
const MAX_LIVE_OUTPUT_CHARS: usize = 50_000;

#[derive(Debug, Clone, PartialEq, Eq)]
struct ToolBlock {
    turn_id: u64,
    call_id: String,
    name: String,
    args_preview: String,
    result_preview: Option<String>,
    is_error: bool,
    subagent_tools: Vec<FrontendSubagentToolUse>,
    subagent_todo: Vec<FrontendTodoItem>,
    live_output: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NoteKind {
    Reminder,
    Error,
    Status,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TimelineBlock {
    UserMessage {
        turn_id: u64,
        text: String,
    },
    AssistantMessage {
        turn_id: u64,
        text: String,
    },
    Thinking {
        turn_id: u64,
        text: String,
    },
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct MarkdownTable {
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PlannedExecutingLine {
    Header { name: String, args: String },
    SubagentTool { prefix: String, text: String },
    Folded { count: usize },
    OutputLine { prefix: String, text: String },
    FoldedOutput { count: usize },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SubagentTodoBlock {
    header: String,
    items: Vec<FrontendTodoItem>,
}

/// Reducer-owned UI state for the terminal adapter.
pub struct CliApp {
    timeline: Vec<TimelineBlock>,
    todo_footer: Vec<FrontendTodoItem>,
    todo_was_active: bool,
    composer: String,
    status: String,
    provider: Option<String>,
    model: Option<String>,
    version: Option<String>,
    working_directory: Option<String>,
    streaming_assistant: Option<StreamingAssistant>,
    awaiting_assistant: bool,
    should_exit: bool,
    exit_requested: bool,
    theme: CliTheme,
    spinner: Spinner,
    displayed_input_tokens: u64,
    displayed_output_tokens: u64,
    target_input_tokens: u64,
    target_output_tokens: u64,
    expand_tool_results: bool,
    /// Index of the next timeline block to insert above the viewport.
    last_inserted_index: usize,
    /// Result lines for tool blocks that were already drained before their result arrived.
    pending_tool_result_lines: Vec<Line<'static>>,
    expand_subagent_tools: bool,
}

impl CliApp {
    pub fn new() -> Self {
        Self {
            timeline: Vec::new(),
            todo_footer: Vec::new(),
            todo_was_active: false,
            composer: String::new(),
            status: "Starting session...".to_string(),
            provider: None,
            model: None,
            version: None,
            working_directory: None,
            streaming_assistant: None,
            awaiting_assistant: false,
            should_exit: false,
            exit_requested: false,
            theme: CliTheme::default(),
            spinner: Spinner::new(),
            displayed_input_tokens: 0,
            displayed_output_tokens: 0,
            target_input_tokens: 0,
            target_output_tokens: 0,
            expand_tool_results: false,
            last_inserted_index: 0,
            pending_tool_result_lines: Vec::new(),
            expand_subagent_tools: false,
        }
    }

    pub fn apply_event(&mut self, event: FrontendEvent) {
        match event {
            FrontendEvent::SessionStarted {
                provider,
                model,
                working_directory,
                version,
            } => {
                self.provider = Some(provider);
                self.model = Some(model.clone());
                self.working_directory = Some(working_directory.clone());
                self.version = Some(version.clone());
                self.status = self.connection_label();
            }
            FrontendEvent::UserMessageCommitted { turn_id, text } => {
                self.streaming_assistant = None;
                self.awaiting_assistant = true;
                self.spinner.reset();
                self.timeline
                    .push(TimelineBlock::UserMessage { turn_id, text });
                self.status = "Waiting for assistant...".to_string();
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
            }
            FrontendEvent::AssistantMessageCompleted { turn_id, text } => {
                self.streaming_assistant = None;
                self.awaiting_assistant = false;
                self.spinner.reset();
                self.timeline
                    .push(TimelineBlock::AssistantMessage { turn_id, text });
                self.status = self.connection_label();
            }
            FrontendEvent::Thinking { turn_id, text } => {
                self.timeline
                    .push(TimelineBlock::Thinking { turn_id, text });
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
                    is_error: false,
                    subagent_tools: Vec::new(),
                    subagent_todo: Vec::new(),
                    live_output: String::new(),
                }));
            }
            FrontendEvent::ToolCallFinished {
                turn_id,
                call_id,
                name,
                result_preview,
                is_error,
            } => {
                // Find matching tool block index
                let found_index = self
                    .timeline
                    .iter()
                    .rev()
                    .position(|block| {
                        matches!(block, TimelineBlock::Tool(t)
                        if t.turn_id == turn_id && t.call_id == call_id)
                    })
                    .map(|rev_idx| self.timeline.len() - 1 - rev_idx);

                if let Some(idx) = found_index {
                    let was_drained = idx < self.last_inserted_index;
                    // Update result in-place
                    if let TimelineBlock::Tool(tool) = &mut self.timeline[idx] {
                        tool.result_preview = Some(result_preview);
                        tool.is_error = is_error;
                    }
                    // If already printed, queue result lines for next drain
                    if was_drained {
                        let tool_clone = match &self.timeline[idx] {
                            TimelineBlock::Tool(t) => t.clone(),
                            _ => unreachable!(),
                        };
                        let result_lines = self.tool_result_only_lines(&tool_clone);
                        self.pending_tool_result_lines.extend(result_lines);
                    }
                } else {
                    self.timeline.push(TimelineBlock::Tool(ToolBlock {
                        turn_id,
                        call_id,
                        name,
                        args_preview: String::new(),
                        result_preview: Some(result_preview),
                        is_error,
                        subagent_tools: Vec::new(),
                        subagent_todo: Vec::new(),
                        live_output: String::new(),
                    }));
                }
            }
            FrontendEvent::ToolCallOutputDelta { turn_id, call_id, delta, .. } => {
                if let Some(TimelineBlock::Tool(tool)) =
                    self.timeline.iter_mut().rev().find(|block| {
                        matches!(
                            block,
                            TimelineBlock::Tool(t) if t.turn_id == turn_id && t.call_id == call_id
                        )
                    })
                {
                    tool.live_output.push_str(&delta);
                    if tool.live_output.chars().count() > MAX_LIVE_OUTPUT_CHARS {
                        let keep_from = tool
                            .live_output
                            .chars()
                            .count()
                            .saturating_sub(MAX_LIVE_OUTPUT_CHARS);
                        let start = tool
                            .live_output
                            .char_indices()
                            .nth(keep_from)
                            .map(|(idx, _)| idx)
                            .unwrap_or(0);
                        tool.live_output = tool.live_output[start..].to_string();
                    }
                }
            }
            FrontendEvent::SubagentTodoSnapshot {
                turn_id,
                call_id,
                items,
            } => {
                if let Some(TimelineBlock::Tool(tool)) =
                    self.timeline.iter_mut().rev().find(|block| {
                        matches!(
                            block,
                            TimelineBlock::Tool(t) if t.turn_id == turn_id && t.call_id == call_id
                        )
                    })
                {
                    tool.subagent_todo = items;
                }
            }
            FrontendEvent::SubagentProgress {
                turn_id,
                call_id,
                tools,
            } => {
                if let Some(TimelineBlock::Tool(tool)) = self.timeline.iter_mut().rev().find(|block| {
                    matches!(
                        block,
                        TimelineBlock::Tool(t) if t.turn_id == turn_id && t.call_id == call_id
                    )
                }) {
                    tool.subagent_tools = tools;
                }
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
            }
            FrontendEvent::Reminder { turn_id, message } => {
                self.timeline.push(TimelineBlock::Note {
                    turn_id: Some(turn_id),
                    kind: NoteKind::Reminder,
                    message,
                });
            }
            FrontendEvent::TokenUsage {
                input_tokens,
                output_tokens,
                ..
            } => {
                self.target_input_tokens = input_tokens;
                self.target_output_tokens = output_tokens;
            }
            FrontendEvent::TodoSnapshot { items, .. } => {
                let all_completed = !items.is_empty()
                    && items
                        .iter()
                        .all(|item| item.status == FrontendTodoStatus::Completed);
                let was_showing = self.todo_was_active && !self.todo_footer.is_empty();

                if all_completed && was_showing {
                    let summary: String = items
                        .iter()
                        .map(|item| format!("[x] #{} {}", item.id, item.text))
                        .collect::<Vec<_>>()
                        .join("\n");

                    self.timeline.push(TimelineBlock::Note {
                        turn_id: None,
                        kind: NoteKind::Status,
                        message: format!("Completed todos:\n{}", summary),
                    });

                    self.todo_footer = Vec::new();
                    self.todo_was_active = false;
                } else if all_completed {
                    self.todo_footer = items;
                    self.todo_was_active = false;
                } else {
                    self.todo_was_active = !items.is_empty();
                    self.todo_footer = items;
                }
            }
            FrontendEvent::Status { message } => {
                if message == "Interrupted" {
                    self.streaming_assistant = None;
                    self.awaiting_assistant = false;
                    self.spinner.reset();
                    self.status = message;
                    return;
                }
                self.timeline.push(TimelineBlock::Note {
                    turn_id: None,
                    kind: NoteKind::Status,
                    message: message.clone(),
                });
                self.status = message;
            }
            FrontendEvent::Error { turn_id, message } => {
                if message == "Interrupted" {
                    self.streaming_assistant = None;
                    self.awaiting_assistant = false;
                    self.spinner.reset();
                    for block in &mut self.timeline {
                        if let TimelineBlock::Tool(tool) = block {
                            if tool.turn_id == turn_id && tool.result_preview.is_none() {
                                tool.result_preview = Some("Interrupted".to_string());
                                tool.is_error = true;
                            }
                        }
                    }
                }
                self.timeline.push(TimelineBlock::Note {
                    turn_id: Some(turn_id),
                    kind: NoteKind::Error,
                    message,
                });
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

    /// Return new timeline blocks since last call as styled Lines, ready for
    /// `terminal.insert_before()`.
    pub fn drain_new_lines(&mut self) -> Vec<Line<'static>> {
        let mut all_lines = std::mem::take(&mut self.pending_tool_result_lines);
        let timeline_len = self.timeline.len();
        while self.last_inserted_index < timeline_len {
            let block = &self.timeline[self.last_inserted_index];
            // Delay rendering tool blocks until they have a result.
            // This ensures the green completion dot appears on the first line.
            if let TimelineBlock::Tool(tool) = block {
                if tool.result_preview.is_none() {
                    break; // Stop here — wait for result before rendering
                }
            }
            all_lines.extend(self.block_to_lines(block));
            self.last_inserted_index += 1;
        }
        all_lines
    }

    pub fn handle_key_event(&mut self, key: KeyEvent) -> Option<FrontendCommand> {
        match (key.code, key.modifiers) {
            (KeyCode::Esc, _) => Some(FrontendCommand::Exit),
            (KeyCode::Char('c'), modifiers) if modifiers.contains(KeyModifiers::CONTROL) => {
                Some(FrontendCommand::Interrupt)
            }
            (KeyCode::Char('b'), modifiers) if modifiers.contains(KeyModifiers::CONTROL) => {
                self.expand_subagent_tools = !self.expand_subagent_tools;
                None
            }
            (KeyCode::Char('o'), modifiers) if modifiers.contains(KeyModifiers::CONTROL) => {
                self.expand_tool_results = !self.expand_tool_results;
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

    pub fn tick(&mut self) -> bool {
        let tokens_changed = self.animate_token_display();
        let spinner_changed = self.spinner.tick();
        tokens_changed || spinner_changed
    }

    /// Number of tool blocks currently executing (no result yet).
    pub fn executing_tool_count(&self) -> u16 {
        self.timeline
            .iter()
            .filter(|block| {
                matches!(
                    block,
                    TimelineBlock::Tool(ToolBlock {
                        name,
                        result_preview: None,
                        ..
                    }) if name != "todo"
                )
            })
            .count() as u16
    }

    fn visible_tool_rows(&self) -> u16 {
        self.planned_executing_tool_lines()
            .len()
            .min(TOOL_PANEL_MAX_ROWS as usize) as u16
    }

    fn visible_activity_rows(&self) -> u16 {
        if self.awaiting_assistant || self.streaming_assistant.is_some() {
            ACTIVITY_ROWS
        } else {
            0
        }
    }

    fn visible_todo_rows(&self) -> u16 {
        let subagent_rows: usize = self
            .subagent_todo_blocks()
            .iter()
            .map(|block| 1 + block.items.len())
            .sum();
        let main_rows = if self.should_show_todo_footer() {
            1 + self.todo_footer.len()
        } else {
            0
        };

        let total_rows = subagent_rows + main_rows;
        if total_rows == 0 {
            return 0;
        }

        u16::try_from(total_rows).unwrap_or(u16::MAX)
    }

    fn tool_todo_gap_rows(&self) -> u16 {
        if self.visible_tool_rows() > 0 && self.visible_todo_rows() > 0 {
            1
        } else {
            0
        }
    }

    fn planned_executing_tool_lines(&self) -> Vec<PlannedExecutingLine> {
        let mut lines = Vec::new();

        for tool in self.timeline.iter().filter_map(|block| match block {
            TimelineBlock::Tool(tool) if tool.result_preview.is_none() && tool.name != "todo" => {
                Some(tool)
            }
            _ => None,
        }) {
            lines.push(PlannedExecutingLine::Header {
                name: capitalize_first(&tool.name),
                args: if tool.args_preview.is_empty() {
                    String::new()
                } else {
                    format!("({})", summarize_tool_args(&tool.name, &tool.args_preview, 40))
                },
            });

            if tool.name == "task" {
                lines.extend(self.subagent_tool_usage_lines(tool));
            }
            if tool.name == "bash" {
                lines.extend(self.live_output_lines(tool));
            }
        }

        lines
    }

    fn live_output_lines(&self, tool: &ToolBlock) -> Vec<PlannedExecutingLine> {
        if tool.live_output.is_empty() {
            return Vec::new();
        }

        let all_lines: Vec<&str> = tool.live_output.lines().collect();
        let visible = all_lines.len().min(BASH_LIVE_OUTPUT_LINES);
        let hidden = all_lines.len().saturating_sub(visible);
        let start = all_lines.len().saturating_sub(visible);

        let mut lines = all_lines[start..]
            .iter()
            .enumerate()
            .map(|(index, line)| PlannedExecutingLine::OutputLine {
                prefix: if index == 0 {
                    "  └ ".to_string()
                } else {
                    "    ".to_string()
                },
                text: truncate_str(line, TOOL_RESULT_LINE_CHARS),
            })
            .collect::<Vec<_>>();

        if hidden > 0 {
            lines.push(PlannedExecutingLine::FoldedOutput { count: hidden });
        }

        lines
    }

    fn subagent_tool_usage_lines(&self, tool: &ToolBlock) -> Vec<PlannedExecutingLine> {
        if tool.subagent_tools.is_empty() {
            return Vec::new();
        }

        let total = tool.subagent_tools.len();
        let visible_count = if self.expand_subagent_tools {
            total
        } else {
            total.min(DEFAULT_VISIBLE_SUBAGENT_TOOLS)
        };
        let hidden_count = total.saturating_sub(visible_count);
        let start = total.saturating_sub(visible_count);

        let mut lines = tool.subagent_tools[start..]
            .iter()
            .enumerate()
            .map(|(index, entry)| PlannedExecutingLine::SubagentTool {
                prefix: if index == 0 {
                    "  └ ".to_string()
                } else {
                    "    ".to_string()
                },
                text: format_subagent_tool_use(entry),
            })
            .collect::<Vec<_>>();

        if hidden_count > 0 {
            lines.push(PlannedExecutingLine::Folded { count: hidden_count });
        }

        lines
    }

    /// Desired viewport height: composer + status, plus only the rows currently needed
    /// for executing tools and the pinned todo panel.
    pub fn desired_viewport_height(&self) -> u16 {
        self.visible_activity_rows()
            + COMPOSER_ROWS
            + STATUS_ROWS
            + self.visible_tool_rows()
            + self.tool_todo_gap_rows()
            + self.visible_todo_rows()
    }

    /// Render executing tools as stable viewport lines.
    pub fn render_executing_tools_lines(&mut self, max_lines: usize) -> Vec<Line<'static>> {
        let planned = self.planned_executing_tool_lines();
        if planned.is_empty() {
            return Vec::new();
        }

        let spinner_frame = self.spinner.frame();
        planned
            .into_iter()
            .take(max_lines)
            .map(|planned| match planned {
                PlannedExecutingLine::Header { name, args } => Line::from(vec![
                    Span::styled(
                        format!("{} ", spinner_frame),
                        self.theme.tool_executing_indicator,
                    ),
                    Span::styled(name, self.theme.tool),
                    Span::styled(args, self.theme.tool_result),
                ]),
                PlannedExecutingLine::SubagentTool { prefix, text } => Line::from(vec![
                    Span::styled(prefix, self.theme.tool_result),
                    Span::styled(text, self.theme.tool_result),
                ]),
                PlannedExecutingLine::Folded { count } => Line::styled(
                    format!("  +{} more tool uses (ctrl+b to expand)", count),
                    self.theme.status,
                ),
                PlannedExecutingLine::OutputLine { prefix, text } => Line::from(vec![
                    Span::styled(prefix, self.theme.tool_result),
                    Span::styled(text, self.theme.tool_result),
                ]),
                PlannedExecutingLine::FoldedOutput { count } => Line::styled(
                    format!("  +{} more output lines", count),
                    self.theme.status,
                ),
            })
            .collect()
    }

    /// Render executing tools as ANSI escape sequence string.
    /// Kept for experiments/debugging; the interactive UI renders these in-viewport.
    /// Returns the number of lines rendered.
    #[allow(dead_code)]
    pub fn render_executing_tools_ansi(&mut self, width: u16) -> (String, u16) {
        // Collect executing tools info first to avoid borrow issues
        let executing: Vec<(String, String)> = self
            .timeline
            .iter()
            .filter_map(|block| match block {
                TimelineBlock::Tool(t) if t.result_preview.is_none() && t.name != "todo" => Some((
                    capitalize_first(&t.name),
                    if t.args_preview.is_empty() {
                        String::new()
                    } else {
                        format!("({})", summarize_tool_args(&t.name, &t.args_preview, 40))
                    },
                )),
                _ => None,
            })
            .take(3)
            .collect();

        if executing.is_empty() {
            return (String::new(), 0);
        }

        let spinner_frame = self.spinner.frame();
        let mut output = String::new();

        for (tool_name, args) in &executing {
            // Use ANSI color codes matching the theme
            // Dim gray for spinner, bright blue for tool name, default for args
            let line = format!(
                "\x1b[2m{}\x1b[0m \x1b[1;38;5;75m{}\x1b[0m\x1b[2m{}\x1b[0m",
                spinner_frame, tool_name, args
            );

            // Pad to width and ensure proper line
            let padded = format!("{:width$}", line, width = width as usize);
            output.push_str(&padded);
            output.push_str("\r\n");
        }

        let lines_count = executing.len() as u16;
        (output, lines_count)
    }
}

impl CliApp {
    pub fn render(&mut self, frame: &mut Frame) {
        let activity_rows = self.visible_activity_rows();
        let tool_rows = self.visible_tool_rows();
        let gap_rows = self.tool_todo_gap_rows();
        let todo_rows = self.visible_todo_rows();

        let mut constraints = Vec::new();
        if activity_rows > 0 {
            constraints.push(Constraint::Length(activity_rows));
        }
        if tool_rows > 0 {
            constraints.push(Constraint::Length(tool_rows));
        }
        if gap_rows > 0 {
            constraints.push(Constraint::Length(gap_rows));
        }
        if todo_rows > 0 {
            constraints.push(Constraint::Length(todo_rows));
        }
        constraints.push(Constraint::Length(COMPOSER_ROWS));
        constraints.push(Constraint::Length(STATUS_ROWS));

        let areas = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(frame.area());

        let mut area_index = 0;
        if activity_rows > 0 {
            let activity_area = areas[area_index];
            let activity = Paragraph::new(self.activity_line()).wrap(Wrap { trim: false });
            frame.render_widget(activity, activity_area);
            area_index += 1;
        }

        if tool_rows > 0 {
            let tool_area = areas[area_index];
            let tool_lines = self.render_executing_tools_lines(tool_area.height as usize);
            let tools = Paragraph::new(tool_lines).wrap(Wrap { trim: false });
            frame.render_widget(tools, tool_area);
            area_index += 1;
        }

        if gap_rows > 0 {
            area_index += 1;
        }

        if todo_rows > 0 {
            let todo_area = areas[area_index];
            let todo_lines = self.render_todo_footer_lines(todo_area.height as usize);
            let todos = Paragraph::new(todo_lines).wrap(Wrap { trim: false });
            frame.render_widget(todos, todo_area);
            area_index += 1;
        }

        let composer_area = areas[area_index];
        let composer = Paragraph::new(self.render_composer_lines(composer_area.width))
            .wrap(Wrap { trim: false });
        frame.render_widget(composer, composer_area);
        if !self.awaiting_assistant
            && self.streaming_assistant.is_none()
            && composer_area.height > 1
            && composer_area.width > 0
        {
            frame.set_cursor_position(self.composer_cursor_position(composer_area));
        }

        // Status
        let status_area = areas[area_index + 1];
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

    fn status_line(&mut self) -> Line<'static> {
        let token_info = format!(
            "↑ {} ↓ {} | ",
            format_token_slot(self.displayed_input_tokens),
            format_token_slot(self.displayed_output_tokens)
        );

        let status = self.status.clone();
        let subagent_hint = if self
            .timeline
            .iter()
            .any(|block| matches!(block, TimelineBlock::Tool(tool) if tool.name == "task" && !tool.subagent_tools.is_empty()))
        {
            " | Ctrl+B expand tools"
        } else {
            ""
        };

        Line::from(format!(
            "{}{} | Enter submit | Ctrl+C interrupt | Esc exit{}",
            token_info, status, subagent_hint
        ))
    }

    fn activity_line(&mut self) -> Line<'static> {
        if self.executing_tool_count() > 0 {
            // Keep layout stable while tools run; avoid toggling activity row on/off.
            return Line::styled(" ", self.theme.activity_meta);
        }

        let elapsed = format_duration_slot(self.spinner.elapsed_seconds());
        let frame = self.spinner.frame().to_string();
        let message = self.spinner.message().to_string();
        let token_info = format!(
            " · ↑ {} · ↓ {}",
            format_token_slot(self.displayed_input_tokens),
            format_token_slot(self.displayed_output_tokens)
        );
        Line::from(vec![
            Span::styled(format!("{} ", frame), self.theme.activity),
            Span::styled(message, self.theme.activity),
            Span::styled(
                format!(" ({}{} · thinking)", elapsed, token_info),
                self.theme.activity_meta,
            ),
        ])
    }

    fn animate_token_display(&mut self) -> bool {
        let prev_input = self.displayed_input_tokens;
        let prev_output = self.displayed_output_tokens;
        self.displayed_input_tokens =
            advance_towards(self.displayed_input_tokens, self.target_input_tokens, 256);
        self.displayed_output_tokens =
            advance_towards(self.displayed_output_tokens, self.target_output_tokens, 256);
        self.displayed_input_tokens != prev_input || self.displayed_output_tokens != prev_output
    }

    pub fn allow_viewport_shrink(&self) -> bool {
        self.executing_tool_count() == 0
            && !self.awaiting_assistant
            && self.streaming_assistant.is_none()
    }

    fn render_composer_lines(&self, width: u16) -> Vec<Line<'static>> {
        if width == 0 {
            return Vec::new();
        }

        let border = "─".repeat(width as usize);
        let body = if let Some(streaming) = &self.streaming_assistant {
            Line::from(vec![
                Span::styled(COMPOSER_PROMPT, self.theme.composer_prompt),
                Span::styled(streaming.text.clone(), self.theme.assistant),
            ])
        } else if self.composer.is_empty() {
            Line::from(vec![
                Span::styled(COMPOSER_PROMPT, self.theme.composer_prompt),
                Span::styled(" ", self.theme.composer_cursor),
            ])
        } else {
            Line::from(vec![
                Span::styled(COMPOSER_PROMPT, self.theme.composer_prompt),
                Span::styled(self.composer.clone(), self.theme.composer),
                Span::styled(" ", self.theme.composer_cursor),
            ])
        };

        vec![
            Line::styled(border.clone(), self.theme.composer_border),
            body,
            Line::styled(border, self.theme.composer_border),
        ]
    }

    fn composer_cursor_position(&self, composer_area: ratatui::layout::Rect) -> (u16, u16) {
        let prompt_width = UnicodeWidthStr::width(COMPOSER_PROMPT) as u16;
        let text_width = UnicodeWidthStr::width(self.composer.as_str()) as u16;
        let cursor_x = composer_area.x
            + (prompt_width + text_width).min(composer_area.width.saturating_sub(1));
        let cursor_y = composer_area.y + 1;
        (cursor_x, cursor_y)
    }

    fn should_show_todo_footer(&self) -> bool {
        self.todo_was_active && !self.todo_footer.is_empty()
    }

    fn render_todo_footer_lines(&self, max_lines: usize) -> Vec<Line<'static>> {
        if max_lines == 0 {
            return Vec::new();
        }

        let mut lines = Vec::new();
        let mut remaining = max_lines;

        for block in self.subagent_todo_blocks() {
            if remaining == 0 {
                break;
            }
            let mut block_lines = render_todo_block_lines(
                &block.header,
                &block.items,
                &self.theme,
                lines.is_empty() && self.visible_activity_rows() > 0,
                remaining,
            );
            remaining = remaining.saturating_sub(block_lines.len());
            lines.append(&mut block_lines);
        }

        if remaining > 0 && self.should_show_todo_footer() {
            let mut block_lines = render_todo_block_lines(
                &todo_header_text(&self.todo_footer),
                &self.todo_footer,
                &self.theme,
                lines.is_empty() && self.visible_activity_rows() > 0,
                remaining,
            );
            lines.append(&mut block_lines);
        }

        lines
    }

    fn subagent_todo_blocks(&self) -> Vec<SubagentTodoBlock> {
        self.timeline
            .iter()
            .filter_map(|block| match block {
                TimelineBlock::Tool(tool)
                    if tool.name == "task"
                        && tool.result_preview.is_none()
                        && !tool.subagent_todo.is_empty() =>
                {
                    Some(SubagentTodoBlock {
                        header: summarize_tool_args(
                            "task",
                            &tool.args_preview,
                            TASK_PROMPT_SUMMARY_CHARS,
                        ),
                        items: tool.subagent_todo.clone(),
                    })
                }
                _ => None,
            })
            .collect()
    }

    fn block_to_lines(&self, block: &TimelineBlock) -> Vec<Line<'static>> {
        match block {
            TimelineBlock::UserMessage { text, .. } => {
                let mut lines = Vec::new();
                for line in text.lines() {
                    lines.push(Line::from(vec![
                        Span::styled("> ", self.theme.user_prompt),
                        Span::styled(format!(" {} ", line), self.theme.user),
                    ]));
                }
                lines.push(Line::raw(""));
                lines
            }
            TimelineBlock::AssistantMessage { text, .. } => {
                let mut lines = self.render_assistant_markdown(text);
                lines.push(Line::raw(""));
                lines
            }
            TimelineBlock::Thinking { text, .. } => {
                let mut lines = vec![Line::raw("")];
                lines.push(Line::styled(
                    "\u{2234} Thinking".to_string(),
                    self.theme.thinking,
                ));
                lines.push(Line::raw(""));
                for line in text.lines() {
                    lines.push(Line::styled(format!("  {}", line), self.theme.thinking));
                }
                lines.push(Line::raw(""));
                lines
            }
            TimelineBlock::Note { kind, message, .. } => {
                let style = match kind {
                    NoteKind::Error => self.theme.error,
                    NoteKind::Reminder => self.theme.status,
                    NoteKind::Status => self.theme.status,
                };
                let mut lines = Vec::new();
                let mut first = true;
                for line in message.lines() {
                    if first {
                        lines.push(Line::styled(format!("\u{25cf} {}", line), style));
                        first = false;
                    } else {
                        lines.push(Line::styled(format!("  {}", line), style));
                    }
                }
                lines.push(Line::raw(""));
                lines
            }
            TimelineBlock::Tool(tool) => self.tool_to_lines(tool),
        }
    }

    fn render_assistant_markdown(&self, text: &str) -> Vec<Line<'static>> {
        let renderer = RendererBuilder::new()
            .with_heading(|_level, spans| vec![Line::from(spans)])
            .build();
        let source_lines: Vec<&str> = text.lines().collect();
        let mut rendered = Vec::new();
        let mut markdown_chunk = Vec::new();
        let mut index = 0;

        while index < source_lines.len() {
            if let Some((table, consumed)) = parse_markdown_table(&source_lines[index..]) {
                if !markdown_chunk.is_empty() {
                    rendered.extend(render_markdown(&markdown_chunk.join("\n"), &renderer).lines);
                    markdown_chunk.clear();
                }
                rendered.extend(self.render_table_block(&table));
                index += consumed;
                continue;
            }

            markdown_chunk.push(source_lines[index].to_string());
            index += 1;
        }

        if !markdown_chunk.is_empty() {
            rendered.extend(render_markdown(&markdown_chunk.join("\n"), &renderer).lines);
        }

        rendered
    }

    fn render_table_block(&self, table: &MarkdownTable) -> Vec<Line<'static>> {
        let mut widths = vec![0usize; table.headers.len()];
        for (index, cell) in table.headers.iter().enumerate() {
            widths[index] = widths[index].max(UnicodeWidthStr::width(cell.as_str()));
        }
        for row in &table.rows {
            for (index, cell) in row.iter().enumerate() {
                widths[index] = widths[index].max(UnicodeWidthStr::width(cell.as_str()));
            }
        }

        let mut lines = Vec::new();
        lines.push(self.render_table_border(&widths, '┌', '┬', '┐'));
        lines.push(self.render_table_row(&table.headers, &widths, self.theme.table_header));
        lines.push(self.render_table_border(&widths, '├', '┼', '┤'));

        for (index, row) in table.rows.iter().enumerate() {
            lines.push(self.render_table_row(row, &widths, self.theme.table_cell));
            if index + 1 < table.rows.len() {
                lines.push(self.render_table_border(&widths, '├', '┼', '┤'));
            }
        }

        lines.push(self.render_table_border(&widths, '└', '┴', '┘'));
        lines
    }

    fn render_table_border(
        &self,
        widths: &[usize],
        left: char,
        middle: char,
        right: char,
    ) -> Line<'static> {
        let mut text = String::new();
        text.push(left);
        for (index, width) in widths.iter().enumerate() {
            text.push_str(&"─".repeat(width + 2));
            text.push(if index + 1 < widths.len() { middle } else { right });
        }
        Line::styled(text, self.theme.table_border)
    }

    fn render_table_row(
        &self,
        row: &[String],
        widths: &[usize],
        text_style: ratatui::style::Style,
    ) -> Line<'static> {
        let mut spans = Vec::new();
        spans.push(Span::styled("│", self.theme.table_border));

        for (index, cell) in row.iter().enumerate() {
            let cell_width = UnicodeWidthStr::width(cell.as_str());
            let padding = widths[index].saturating_sub(cell_width);
            spans.push(Span::styled(
                format!(" {}{} ", cell, " ".repeat(padding)),
                text_style,
            ));
            spans.push(Span::styled("│", self.theme.table_border));
        }

        Line::from(spans)
    }

    fn tool_to_lines(&self, tool: &ToolBlock) -> Vec<Line<'static>> {
        if tool.name == "todo" {
            return Vec::new();
        }

        let mut lines = vec![Line::raw("")];
        let tool_name = capitalize_first(&tool.name);
        if tool.name == "task" {
            let (indicator_char, indicator_style) = if tool.is_error {
                ("\u{25cf}", self.theme.error)
            } else if tool.result_preview.is_none() {
                ("\u{25cf}", self.theme.tool_executing_indicator)
            } else {
                ("\u{25cf}", self.theme.tool_indicator)
            };
            let args_display = if tool.args_preview.is_empty() {
                String::new()
            } else {
                format!("({})", summarize_tool_args(&tool.name, &tool.args_preview, TASK_PROMPT_SUMMARY_CHARS))
            };
            lines.push(Line::from(vec![
                Span::styled(format!("{} ", indicator_char), indicator_style),
                Span::styled(tool_name, self.theme.tool),
                Span::styled(args_display, self.theme.tool_result),
            ]));

            for entry in self.subagent_tool_usage_lines(tool) {
                match entry {
                    PlannedExecutingLine::Header { .. } => {}
                    PlannedExecutingLine::SubagentTool { prefix, text } => {
                        lines.push(Line::from(vec![
                            Span::styled(prefix, self.theme.tool_result),
                            Span::styled(text, self.theme.tool_result),
                        ]));
                    }
                    PlannedExecutingLine::Folded { count } => lines.push(Line::styled(
                        format!("  +{} more tool uses (ctrl+b to expand)", count),
                        self.theme.status,
                    )),
                    PlannedExecutingLine::OutputLine { .. } | PlannedExecutingLine::FoldedOutput { .. } => {}
                }
            }

            if tool.is_error {
                if let Some(result) = &tool.result_preview {
                    for line in result.lines() {
                        lines.push(Line::from(vec![
                            Span::styled("  └ ", self.theme.error),
                            Span::styled(line.to_string(), self.theme.error),
                        ]));
                    }
                }
            }

            if tool.result_preview.is_some() {
                lines.push(Line::raw(""));
            }
            return lines;
        }

        if tool.name == "read" {
            let file_path = serde_json::from_str::<serde_json::Value>(&tool.args_preview)
                .ok()
                .and_then(|v| {
                    v.get("file_path")
                        .and_then(|f| f.as_str())
                        .map(String::from)
                })
                .unwrap_or_else(|| tool.args_preview.clone());

            let is_err = tool.is_error;
            let (indicator_char, indicator_style) = if is_err {
                ("\u{25cf}", self.theme.error)
            } else if tool.result_preview.is_none() {
                ("\u{25cf}", self.theme.tool_executing_indicator)
            } else {
                ("\u{25cf}", self.theme.tool_indicator)
            };
            lines.push(Line::from(vec![
                Span::styled(format!("{} ", indicator_char), indicator_style),
                Span::styled(tool_name, self.theme.tool),
                Span::styled(format!("({})", file_path), self.theme.tool_result),
            ]));

            if let Some(result) = &tool.result_preview {
                let line_count = result.lines().count();
                let is_err = tool.is_error;
                if is_err {
                    // Error result — show in red
                    for line in result.lines() {
                        lines.push(Line::from(vec![
                            Span::styled("  \u{2514} ", self.theme.error),
                            Span::styled(line.to_string(), self.theme.error),
                        ]));
                    }
                } else if line_count <= 2 {
                    // Short result — show actual content
                    for line in result.lines() {
                        lines.push(Line::from(vec![
                            Span::styled("  \u{2514} ", self.theme.tool_result),
                            Span::styled(line.to_string(), self.theme.tool_result),
                        ]));
                    }
                } else {
                    lines.push(Line::from(vec![
                        Span::styled("  \u{2514} ", self.theme.tool_result),
                        Span::styled("Read ", self.theme.tool_result),
                        Span::styled(format!("{}", line_count), self.theme.tool_result_highlight),
                        Span::styled(" lines", self.theme.tool_result),
                    ]));
                }
            }
        } else {
            let args_display = if tool.args_preview.is_empty() {
                String::new()
            } else {
                format!(
                    "({})",
                    summarize_tool_args(&tool.name, &tool.args_preview, 80)
                )
            };
            let is_err = tool.is_error;
            let (indicator_char, indicator_style) = if is_err {
                ("\u{25cf}", self.theme.error)
            } else if tool.result_preview.is_none() {
                ("\u{25cf}", self.theme.tool_executing_indicator)
            } else {
                ("\u{25cf}", self.theme.tool_indicator)
            };
            lines.push(Line::from(vec![
                Span::styled(format!("{} ", indicator_char), indicator_style),
                Span::styled(tool_name, self.theme.tool),
                Span::styled(args_display, self.theme.tool_result),
            ]));
            if let Some(result) = &tool.result_preview {
                let result_style = if tool.is_error {
                    self.theme.error
                } else {
                    self.theme.tool_result
                };
                if tool.name == "bash" && !tool.is_error {
                    lines.extend(render_bash_result_preview(result, result_style));
                } else {
                    let all_lines: Vec<&str> = result.lines().collect();
                    let (visible_lines, hidden) = visible_tail_lines(
                        &all_lines,
                        self.expand_tool_results,
                        DEFAULT_VISIBLE_TOOL_RESULT_LINES,
                    );
                    for (i, line) in visible_lines.iter().enumerate() {
                        let line = truncate_str(line, TOOL_RESULT_LINE_CHARS);
                        if i == 0 {
                            lines.push(Line::from(vec![
                                Span::styled("  \u{2514} ", result_style),
                                Span::styled(line, result_style),
                            ]));
                        } else {
                            lines.push(Line::styled(format!("    {}", line), result_style));
                        }
                    }
                    if hidden > 0 {
                        lines.push(Line::styled(
                            format!("  +{} more output lines (Ctrl+O to expand)", hidden),
                            self.theme.status,
                        ));
                    }
                }
            }
        }
        // Only add trailing blank when result is present (complete block).
        // If result arrives later, tool_result_only_lines adds the trailing blank.
        if tool.result_preview.is_some() {
            lines.push(Line::raw(""));
        }
        lines
    }
    fn tool_result_only_lines(&self, tool: &ToolBlock) -> Vec<Line<'static>> {
        if tool.name == "todo" || tool.name == "task" {
            return Vec::new();
        }

        let mut lines = Vec::new();
        if let Some(result) = &tool.result_preview {
            // Completion indicator: green dot for success, red for error
            let completion_style = if tool.is_error {
                self.theme.error
            } else {
                self.theme.tool_indicator
            };
            if tool.name == "read" {
                let line_count = result.lines().count();
                if line_count <= 2 {
                    for line in result.lines() {
                        lines.push(Line::from(vec![
                            Span::styled("  \u{25cf} ", completion_style),
                            Span::styled(line.to_string(), self.theme.tool_result),
                        ]));
                    }
                } else {
                    lines.push(Line::from(vec![
                        Span::styled("  \u{25cf} ", completion_style),
                        Span::styled("Read ", self.theme.tool_result),
                        Span::styled(format!("{}", line_count), self.theme.tool_result_highlight),
                        Span::styled(" lines", self.theme.tool_result),
                    ]));
                }
            } else if tool.name == "bash" && !tool.is_error {
                lines.extend(render_bash_result_preview(result, self.theme.tool_result));
            } else {
                let result_style = if tool.is_error {
                    self.theme.error
                } else {
                    self.theme.tool_result
                };
                let all_lines: Vec<&str> = result.lines().collect();
                let (visible_lines, hidden) = visible_tail_lines(
                    &all_lines,
                    self.expand_tool_results,
                    DEFAULT_VISIBLE_TOOL_RESULT_LINES,
                );
                for (i, line) in visible_lines.iter().enumerate() {
                    let line = truncate_str(line, TOOL_RESULT_LINE_CHARS);
                    if i == 0 {
                        lines.push(Line::from(vec![
                            Span::styled("  \u{25cf} ", completion_style),
                            Span::styled(line, result_style),
                        ]));
                    } else {
                        lines.push(Line::styled(format!("    {}", line), result_style));
                    }
                }
                if hidden > 0 {
                    lines.push(Line::styled(
                        format!("    +{} more output lines (Ctrl+O to expand)", hidden),
                        self.theme.status,
                    ));
                }
            }
            lines.push(Line::raw(""));
        }
        lines
    }
}

fn visible_tail_lines<'a>(
    all_lines: &'a [&'a str],
    expanded: bool,
    max_visible: usize,
) -> (Vec<&'a str>, usize) {
    if expanded || all_lines.len() <= max_visible {
        return (all_lines.to_vec(), 0);
    }

    let start = all_lines.len().saturating_sub(max_visible);
    (
        all_lines[start..].to_vec(),
        all_lines.len().saturating_sub(max_visible),
    )
}

/// Detect whether a tool result looks like an error message.
fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

fn truncate_str(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }

    let end = text
        .char_indices()
        .nth(max_chars.saturating_sub(3))
        .map(|(idx, _)| idx)
        .unwrap_or(0);
    format!("{}...", &text[..end])
}

fn advance_towards(current: u64, target: u64, max_step: u64) -> u64 {
    if current >= target {
        return target;
    }

    let remaining = target - current;
    if remaining <= 12 {
        return target;
    }

    let step = (remaining / 3).max(24).min(max_step);
    (current + step).min(target)
}

fn format_duration(seconds: u64) -> String {
    if seconds < 60 {
        return format!("{seconds}s");
    }

    let minutes = seconds / 60;
    let seconds = seconds % 60;
    if minutes < 60 {
        return format!("{minutes}m{seconds:02}s");
    }

    let hours = minutes / 60;
    let minutes = minutes % 60;
    format!("{hours}h{minutes:02}m")
}

fn format_duration_slot(seconds: u64) -> String {
    format!("{:>6}", format_duration(seconds))
}

fn format_token_count(tokens: u64) -> String {
    match tokens {
        0..=999 => tokens.to_string(),
        1_000..=999_999 => format_compact(tokens, 1_000, "k"),
        _ => format_compact(tokens, 1_000_000, "m"),
    }
}

fn format_token_slot(tokens: u64) -> String {
    if tokens == 0 {
        return format!("{:>5}", "-");
    }
    format!("{:>5}", format_token_count(tokens))
}

fn format_compact(value: u64, divisor: u64, suffix: &str) -> String {
    let whole = value / divisor;
    let tenth = (value % divisor) / (divisor / 10);
    if tenth == 0 {
        format!("{whole}{suffix}")
    } else {
        format!("{whole}.{tenth}{suffix}")
    }
}

#[derive(Debug, Deserialize)]
struct TodoArgsPreview {
    items: Vec<TodoItemPreview>,
}

#[derive(Debug, Deserialize)]
struct TodoItemPreview {
    id: u32,
    text: String,
    status: TodoStatusPreview,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum TodoStatusPreview {
    Pending,
    InProgress,
    Completed,
}

fn summarize_tool_args(tool_name: &str, args: &str, _max: usize) -> String {
    match tool_name {
        "todo" => truncate_str(&summarize_todo_args(args), _max),
        "task" => truncate_str(&summarize_task_args(args), _max),
        "bash" => truncate_str(&summarize_bash_args(args), _max),
        _ => truncate_str(args, _max),
    }
}

#[derive(Debug, Deserialize)]
struct TaskArgsPreview {
    prompt: String,
}

#[derive(Debug, Deserialize)]
struct BashArgsPreview {
    command: String,
}

fn summarize_task_args(args: &str) -> String {
    serde_json::from_str::<TaskArgsPreview>(args)
        .map(|task| task.prompt)
        .unwrap_or_else(|_| args.to_string())
}

fn summarize_bash_args(args: &str) -> String {
    serde_json::from_str::<BashArgsPreview>(args)
        .map(|bash| bash.command)
        .unwrap_or_else(|_| args.to_string())
}

fn render_bash_result_preview(
    result: &str,
    result_style: ratatui::style::Style,
) -> Vec<Line<'static>> {
    let result_lines: Vec<&str> = result.lines().collect();
    let total_chars = result.chars().count();

    if result_lines.len() <= BASH_OUTPUT_INLINE_MAX_LINES && total_chars <= BASH_OUTPUT_INLINE_MAX_CHARS
    {
        return result_lines
            .iter()
            .enumerate()
            .map(|(index, line)| {
                let line = truncate_str(line, TOOL_RESULT_LINE_CHARS);
                if index == 0 {
                    Line::from(vec![
                        Span::styled("  \u{2514} ", result_style),
                        Span::styled(line, result_style),
                    ])
                } else {
                    Line::styled(format!("    {}", line), result_style)
                }
            })
            .collect();
    }

    let mut lines = vec![Line::from(vec![
        Span::styled("  \u{2514} ", result_style),
        Span::styled(
            format!(
                "Output truncated: {} more lines",
                result_lines.len().saturating_sub(BASH_OUTPUT_TAIL_LINES)
            ),
            result_style,
        ),
    ])];

    let tail_start = result_lines.len().saturating_sub(BASH_OUTPUT_TAIL_LINES);
    if tail_start > 0 {
        lines.push(Line::styled("    ...".to_string(), result_style));
    }

    for line in &result_lines[tail_start..] {
        lines.push(Line::styled(
            format!("    {}", truncate_str(line, TOOL_RESULT_LINE_CHARS)),
            result_style,
        ));
    }

    lines
}

fn format_subagent_tool_use(tool_use: &FrontendSubagentToolUse) -> String {
    let name = capitalize_first(&tool_use.name);
    let args = summarize_tool_args(&tool_use.name, &tool_use.args_preview, SUBAGENT_TOOL_ARGS_CHARS);
    if args.is_empty() {
        name
    } else {
        format!("{}({})", name, args)
    }
}

fn summarize_todo_args(args: &str) -> String {
    let Ok(todo) = serde_json::from_str::<TodoArgsPreview>(args) else {
        return args.to_string();
    };

    if todo.items.is_empty() {
        return "0 items".to_string();
    }

    let mut parts = todo
        .items
        .iter()
        .take(2)
        .map(|item| {
            format!(
                "#{} {} {}",
                item.id,
                todo_status_label(&item.status),
                item.text.trim()
            )
        })
        .collect::<Vec<_>>();

    if todo.items.len() > 2 {
        parts.push(format!("+{} more", todo.items.len() - 2));
    }

    format!(
        "{} item{}: {}",
        todo.items.len(),
        if todo.items.len() == 1 { "" } else { "s" },
        parts.join("; ")
    )
}

fn todo_status_label(status: &TodoStatusPreview) -> &'static str {
    match status {
        TodoStatusPreview::Pending => "pending",
        TodoStatusPreview::InProgress => "in progress",
        TodoStatusPreview::Completed => "done",
    }
}

fn render_todo_item_line(item: &FrontendTodoItem, theme: &CliTheme) -> Line<'static> {
    let (marker, marker_style, text_style) = match item.status {
        FrontendTodoStatus::Pending => ("□", theme.todo_pending, theme.todo_pending),
        FrontendTodoStatus::InProgress => ("■", theme.todo_in_progress, theme.todo_in_progress),
        FrontendTodoStatus::Completed => ("✓", theme.todo_completed, theme.todo_completed),
    };

    Line::from(vec![
        Span::styled("  ", theme.todo_connector),
        Span::styled(marker.to_string(), marker_style),
        Span::styled(" ", theme.todo_connector),
        Span::styled(item.text.clone(), text_style),
    ])
}

fn render_todo_block_lines(
    header: &str,
    items: &[FrontendTodoItem],
    theme: &CliTheme,
    connect_to_activity: bool,
    max_lines: usize,
) -> Vec<Line<'static>> {
    if max_lines == 0 {
        return Vec::new();
    }

    let header_prefix = if connect_to_activity { "└ " } else { "• " };
    let mut lines = vec![Line::from(vec![
        Span::styled(header_prefix, theme.todo_connector),
        Span::styled(header.to_string(), theme.footer),
    ])];

    let item_rows = max_lines.saturating_sub(1);
    for item in items.iter().take(item_rows) {
        lines.push(render_todo_item_line(item, theme));
    }

    lines
}

fn todo_header_text(items: &[FrontendTodoItem]) -> String {
    items.iter()
        .find(|item| item.status == FrontendTodoStatus::InProgress)
        .or_else(|| {
            items.iter()
                .find(|item| item.status == FrontendTodoStatus::Pending)
        })
        .map(|item| item.text.clone())
        .or_else(|| items.first().map(|item| item.text.clone()))
        .unwrap_or_else(|| "Tasks".to_string())
}

fn parse_markdown_table(lines: &[&str]) -> Option<(MarkdownTable, usize)> {
    if lines.len() < 2 {
        return None;
    }

    let headers = parse_table_row(lines[0])?;
    if !is_table_delimiter(lines[1], headers.len()) {
        return None;
    }

    let mut rows = Vec::new();
    let mut consumed = 2;
    while let Some(line) = lines.get(consumed) {
        if line.trim().is_empty() {
            break;
        }

        let Some(row) = parse_table_row(line) else {
            break;
        };
        if row.len() != headers.len() {
            break;
        }
        rows.push(row);
        consumed += 1;
    }

    Some((MarkdownTable { headers, rows }, consumed))
}

fn parse_table_row(line: &str) -> Option<Vec<String>> {
    let trimmed = line.trim();
    if !trimmed.contains('|') {
        return None;
    }

    let trimmed = trimmed
        .strip_prefix('|')
        .unwrap_or(trimmed)
        .strip_suffix('|')
        .unwrap_or(trimmed);
    let cells: Vec<String> = trimmed.split('|').map(|cell| cell.trim().to_string()).collect();
    if cells.len() < 2 || cells.iter().any(|cell| cell.is_empty()) {
        return None;
    }
    Some(cells)
}

fn is_table_delimiter(line: &str, expected_columns: usize) -> bool {
    let trimmed = line.trim();
    let trimmed = trimmed
        .strip_prefix('|')
        .unwrap_or(trimmed)
        .strip_suffix('|')
        .unwrap_or(trimmed);
    let cells: Vec<&str> = trimmed.split('|').map(str::trim).collect();
    if cells.len() != expected_columns {
        return false;
    }

    cells.iter().all(|cell| {
        let cell = cell.trim_matches(':');
        cell.len() >= 3 && cell.chars().all(|ch| ch == '-')
    })
}

impl Default for CliApp {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;
    use ratatui::Terminal;

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
            is_error: false,
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
            is_error: false,
        });
        app.apply_event(FrontendEvent::ToolCallFinished {
            turn_id: 2,
            call_id: "a".to_string(),
            name: "read".to_string(),
            result_preview: "first".to_string(),
            is_error: false,
        });

        let read_tool = app.timeline.iter().find_map(|block| match block {
            TimelineBlock::Tool(t) if t.call_id == "a" => Some(t),
            _ => None,
        });
        let grep_tool = app.timeline.iter().find_map(|block| match block {
            TimelineBlock::Tool(t) if t.call_id == "b" => Some(t),
            _ => None,
        });

        assert!(read_tool.is_some(), "read tool should exist");
        assert!(grep_tool.is_some(), "grep tool should exist");
        assert_eq!(read_tool.unwrap().result_preview.as_deref(), Some("first"));
        assert_eq!(grep_tool.unwrap().result_preview.as_deref(), Some("second"));
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
            is_error: false,
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
    fn todo_auto_collapses_completed_items_into_timeline() {
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
        assert!(app.todo_was_active);

        app.apply_event(FrontendEvent::TodoSnapshot {
            turn_id: 1,
            items: vec![FrontendTodoItem {
                id: 2,
                text: "Second".to_string(),
                status: FrontendTodoStatus::Completed,
            }],
        });

        assert_eq!(app.timeline.len(), timeline_len + 1);
        assert!(app.todo_footer.is_empty());
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
    fn assistant_deltas_append_within_the_active_turn() {
        let mut app = CliApp::new();

        app.apply_event(FrontendEvent::AssistantMessageDelta {
            turn_id: 7,
            delta: "hello".to_string(),
        });
        app.apply_event(FrontendEvent::AssistantMessageDelta {
            turn_id: 7,
            delta: " world".to_string(),
        });

        assert_eq!(
            app.streaming_assistant,
            Some(StreamingAssistant {
                turn_id: 7,
                text: "hello world".to_string(),
            })
        );

        app.apply_event(FrontendEvent::AssistantMessageCompleted {
            turn_id: 7,
            text: "hello world".to_string(),
        });

        assert_eq!(app.streaming_assistant, None);
        assert!(matches!(
            app.timeline.last(),
            Some(TimelineBlock::AssistantMessage { turn_id: 7, text })
                if text == "hello world"
        ));
    }

    #[test]
    fn render_shows_composer_and_status() {
        let mut app = CliApp::new();
        let height = app.desired_viewport_height();
        let buffer = render_buffer(&mut app, 40, height);
        let lines = buffer_text(&buffer);

        assert!(lines.iter().any(|line| line.contains("> ")));
        assert!(lines.iter().any(|line| line.contains("Starting session")));
    }

    #[test]
    fn render_shows_streaming_text_in_composer_area() {
        let mut app = CliApp::new();
        app.apply_event(FrontendEvent::AssistantMessageDelta {
            turn_id: 1,
            delta: "hello world".to_string(),
        });

        let height = app.desired_viewport_height();
        let buffer = render_buffer(&mut app, 40, height);
        let lines = buffer_text(&buffer);

        assert!(lines.iter().any(|line| line.contains("hello world")));
    }

    #[test]
    fn waiting_for_assistant_shows_spinner_in_activity_line() {
        let mut app = CliApp::new();
        app.apply_event(FrontendEvent::UserMessageCommitted {
            turn_id: 1,
            text: "hello".to_string(),
        });

        let status = app.activity_line().to_string();
        assert!(crate::cli::spinner::FUN_MESSAGES
            .iter()
            .any(|message| status.contains(message)));
        assert!(crate::cli::spinner::SPINNER_FRAMES
            .iter()
            .any(|frame| status.contains(frame)));
        assert!(status.contains("thinking"));
    }

    #[test]
    fn render_places_activity_line_above_composer() {
        let mut app = CliApp::new();
        app.apply_event(FrontendEvent::UserMessageCommitted {
            turn_id: 1,
            text: "hello".to_string(),
        });

        let height = app.desired_viewport_height();
        let buffer = render_buffer(&mut app, 80, height);
        let lines = buffer_text(&buffer);

        assert!(lines.first().is_some_and(|line| line.contains("thinking")));
        assert!(lines.iter().any(|line| line.contains("> ")));
    }

    #[test]
    fn activity_line_uses_real_token_usage_updates() {
        let mut app = CliApp::new();
        app.apply_event(FrontendEvent::UserMessageCommitted {
            turn_id: 1,
            text: "please inspect the repository and summarize the main modules".to_string(),
        });
        let before = app.activity_line().to_string();
        assert!(before.contains("↑     -"));
        assert!(before.contains("↓     -"));

        app.apply_event(FrontendEvent::TokenUsage {
            turn_id: 1,
            input_tokens: 321,
            output_tokens: 192,
            total_tokens: 513,
        });

        let intermediate = app.activity_line().to_string();
        assert!(!intermediate.contains("↑ 321"));
        assert!(!intermediate.contains("↓ 192"));

        for _ in 0..12 {
            app.tick();
        }

        let line = app.activity_line().to_string();
        assert!(line.contains("↑   321"));
        assert!(line.contains("↓   192"));
    }

    #[test]
    fn render_shows_executing_tools_in_viewport() {
        let mut app = CliApp::new();
        app.apply_event(FrontendEvent::ToolCallStarted {
            turn_id: 1,
            call_id: "bash-1".to_string(),
            name: "bash".to_string(),
            args_preview: "{\"command\":\"cargo test\"}".to_string(),
        });

        let height = app.desired_viewport_height();
        let buffer = render_buffer(&mut app, 60, height);
        let lines = buffer_text(&buffer);

        assert!(lines.iter().any(|line| line.contains("Bash")));
        assert!(lines.iter().any(|line| line.contains("cargo test")));
        assert!(lines.iter().any(|line| line.contains("> ")));
    }

    #[test]
    fn executing_bash_shows_live_output_tail() {
        let mut app = CliApp::new();
        app.apply_event(FrontendEvent::ToolCallStarted {
            turn_id: 1,
            call_id: "bash-1".to_string(),
            name: "bash".to_string(),
            args_preview: "{\"command\":\"echo hi\"}".to_string(),
        });
        app.apply_event(FrontendEvent::ToolCallOutputDelta {
            turn_id: 1,
            call_id: "bash-1".to_string(),
            delta: "one\ntwo\nthree\nfour\nfive\n".to_string(),
            is_err_stream: false,
        });

        let lines = app.render_executing_tools_lines(8);
        let rendered: Vec<String> = lines
            .iter()
            .map(|line| line.spans.iter().map(|span| span.content.as_ref()).collect())
            .collect();

        // Shows only tail lines by default.
        assert!(!rendered.iter().any(|line| line.contains("one")));
        assert!(rendered.iter().any(|line| line.contains("two")));
        assert!(rendered.iter().any(|line| line.contains("three")));
        assert!(rendered.iter().any(|line| line.contains("four")));
        assert!(rendered.iter().any(|line| line.contains("five")));
        assert!(rendered
            .iter()
            .any(|line| line.contains("more output lines")));
    }

    #[test]
    fn executing_tools_hide_activity_line_until_assistant_resumes() {
        let mut app = CliApp::new();
        app.apply_event(FrontendEvent::UserMessageCommitted {
            turn_id: 1,
            text: "install nginx".to_string(),
        });
        assert_eq!(app.visible_activity_rows(), ACTIVITY_ROWS);

        app.apply_event(FrontendEvent::ToolCallStarted {
            turn_id: 1,
            call_id: "bash-1".to_string(),
            name: "bash".to_string(),
            args_preview: r#"{"command":"brew install nginx"}"#.to_string(),
        });

        assert_eq!(app.visible_activity_rows(), ACTIVITY_ROWS);
        assert_eq!(app.activity_line().to_string().trim(), "");

        app.apply_event(FrontendEvent::ToolCallFinished {
            turn_id: 1,
            call_id: "bash-1".to_string(),
            name: "bash".to_string(),
            result_preview: "done".to_string(),
            is_error: false,
        });

        assert_eq!(app.visible_activity_rows(), ACTIVITY_ROWS);
    }

    #[test]
    fn subagent_progress_renders_last_tools_and_folds_older_ones() {
        let mut app = CliApp::new();
        app.apply_event(FrontendEvent::ToolCallStarted {
            turn_id: 1,
            call_id: "task-1".to_string(),
            name: "task".to_string(),
            args_preview: r#"{"prompt":"Explore project overview"}"#.to_string(),
        });
        app.apply_event(FrontendEvent::SubagentProgress {
            turn_id: 1,
            call_id: "task-1".to_string(),
            tools: vec![
                FrontendSubagentToolUse {
                    name: "glob".to_string(),
                    args_preview: "src/**/*.rs".to_string(),
                },
                FrontendSubagentToolUse {
                    name: "read".to_string(),
                    args_preview: "/tmp/a.rs".to_string(),
                },
                FrontendSubagentToolUse {
                    name: "read".to_string(),
                    args_preview: "/tmp/b.rs".to_string(),
                },
                FrontendSubagentToolUse {
                    name: "grep".to_string(),
                    args_preview: "TodoUsageHook".to_string(),
                },
                FrontendSubagentToolUse {
                    name: "read".to_string(),
                    args_preview: "/tmp/c.rs".to_string(),
                },
            ],
        });

        let lines = app.render_executing_tools_lines(8);
        let rendered: Vec<String> = lines
            .iter()
            .map(|line| line.spans.iter().map(|span| span.content.as_ref()).collect())
            .collect();

        assert!(rendered.iter().any(|line| line.contains("Task")));
        assert!(!rendered.iter().any(|line| line.contains("Glob(src/**/*.rs)")));
        assert!(rendered.iter().any(|line| line.contains("/tmp/a.rs")));
        assert!(rendered.iter().any(|line| line.contains("/tmp/b.rs")));
        assert!(rendered.iter().any(|line| line.contains("/tmp/c.rs")));
        assert!(rendered.iter().any(|line| line.contains("+1 more tool uses")));
    }

    #[test]
    fn ctrl_b_toggles_subagent_tool_expansion() {
        let mut app = CliApp::new();
        app.apply_event(FrontendEvent::ToolCallStarted {
            turn_id: 1,
            call_id: "task-1".to_string(),
            name: "task".to_string(),
            args_preview: r#"{"prompt":"Explore project overview"}"#.to_string(),
        });
        app.apply_event(FrontendEvent::SubagentProgress {
            turn_id: 1,
            call_id: "task-1".to_string(),
            tools: vec![
                FrontendSubagentToolUse {
                    name: "read".to_string(),
                    args_preview: "/tmp/1.rs".to_string(),
                },
                FrontendSubagentToolUse {
                    name: "read".to_string(),
                    args_preview: "/tmp/2.rs".to_string(),
                },
                FrontendSubagentToolUse {
                    name: "read".to_string(),
                    args_preview: "/tmp/3.rs".to_string(),
                },
                FrontendSubagentToolUse {
                    name: "read".to_string(),
                    args_preview: "/tmp/4.rs".to_string(),
                },
                FrontendSubagentToolUse {
                    name: "read".to_string(),
                    args_preview: "/tmp/5.rs".to_string(),
                },
            ],
        });

        let collapsed = app.render_executing_tools_lines(8);
        let collapsed_text: Vec<String> = collapsed
            .iter()
            .map(|line| line.spans.iter().map(|span| span.content.as_ref()).collect())
            .collect();
        assert!(!collapsed_text.iter().any(|line| line.contains("/tmp/1.rs")));

        assert_eq!(
            app.handle_key_event(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::CONTROL)),
            None
        );

        let expanded = app.render_executing_tools_lines(8);
        let expanded_text: Vec<String> = expanded
            .iter()
            .map(|line| line.spans.iter().map(|span| span.content.as_ref()).collect())
            .collect();
        assert!(expanded_text.iter().any(|line| line.contains("/tmp/1.rs")));
        assert!(expanded_text.iter().any(|line| line.contains("/tmp/5.rs")));
    }

    #[test]
    fn task_summary_is_truncated_for_long_multibyte_prompt() {
        let summary = summarize_tool_args(
            "task",
            r#"{"prompt":"请全面分析当前项目，并先使用todo工具创建任务列表来跟踪分析进度，包括探索项目结构、识别配置文件、分析源代码和理解技术栈。"}"#,
            TASK_PROMPT_SUMMARY_CHARS,
        );

        assert!(summary.chars().count() <= TASK_PROMPT_SUMMARY_CHARS);
        assert!(summary.ends_with("...") || summary.contains("todo工具"));
    }

    #[test]
    fn bash_summary_shows_command_without_json_wrapper() {
        let summary = summarize_tool_args(
            "bash",
            r#"{"command":"ls -la /Users/wangyue/workspace/codelder/harness/target/debug/deps/ 2>/dev/null"}"#,
            120,
        );

        assert!(summary.starts_with("ls -la "));
        assert!(summary.contains("2>/dev/null"));
        assert!(!summary.contains("\"command\""));
        assert!(!summary.contains('{'));
    }

    #[test]
    fn completed_bash_tool_line_uses_wider_command_summary() {
        let mut app = CliApp::new();
        app.apply_event(FrontendEvent::ToolCallStarted {
            turn_id: 1,
            call_id: "bash-1".to_string(),
            name: "bash".to_string(),
            args_preview:
                r#"{"command":"ls -la /Users/wangyue/workspace/codelder/harness/target/debug/deps/ 2>/dev/null"}"#
                    .to_string(),
        });
        app.apply_event(FrontendEvent::ToolCallFinished {
            turn_id: 1,
            call_id: "bash-1".to_string(),
            name: "bash".to_string(),
            result_preview: "done".to_string(),
            is_error: false,
        });

        let lines = app.drain_new_lines();
        let rendered: Vec<String> = lines
            .iter()
            .map(|line| line.spans.iter().map(|span| span.content.as_ref()).collect())
            .collect();

        assert!(rendered
            .iter()
            .any(|line| line.contains("Bash(ls -la /Users/wangyue/workspace/codelder/harness/target/debug/deps/ 2>/dev/null)")));
        assert!(!rendered.iter().any(|line| line.contains("\"command\"")));
    }

    #[test]
    fn long_bash_output_is_summarized_in_timeline() {
        let mut app = CliApp::new();
        let long_output = (0..20)
            .map(|index| format!("target/debug/deps/file-{index}.o"))
            .collect::<Vec<_>>()
            .join("\n");

        app.apply_event(FrontendEvent::ToolCallStarted {
            turn_id: 1,
            call_id: "bash-1".to_string(),
            name: "bash".to_string(),
            args_preview: r#"{"command":"find target/debug/deps -maxdepth 1"}"#.to_string(),
        });
        app.apply_event(FrontendEvent::ToolCallFinished {
            turn_id: 1,
            call_id: "bash-1".to_string(),
            name: "bash".to_string(),
            result_preview: long_output,
            is_error: false,
        });

        let lines = app.drain_new_lines();
        let rendered: Vec<String> = lines
            .iter()
            .map(|line| line.spans.iter().map(|span| span.content.as_ref()).collect())
            .collect();

        assert!(rendered
            .iter()
            .any(|line| line.contains("Output truncated: 17 more lines")));
        assert!(rendered.iter().any(|line| line.contains("...")));
        assert!(rendered.iter().any(|line| line.contains("file-19.o")));
        assert!(!rendered.iter().any(|line| line.contains("file-0.o")));
    }

    #[test]
    fn token_animation_advances_towards_target() {
        let mut value = 0;
        for _ in 0..20 {
            value = advance_towards(value, 513, 256);
        }
        assert_eq!(value, 513);
    }

    #[test]
    fn activity_line_formats_duration_and_compact_tokens() {
        let mut app = CliApp::new();
        app.apply_event(FrontendEvent::UserMessageCommitted {
            turn_id: 1,
            text: "hello".to_string(),
        });
        app.displayed_input_tokens = 1_300;
        app.displayed_output_tokens = 240;
        app.spinner.reset();
        app.spinner.set_started_at_for_test(
            std::time::Instant::now() - std::time::Duration::from_secs(100),
        );

        let line = app.activity_line().to_string();
        assert!(line.contains("( 1m40s"));
        assert!(line.contains("↑  1.3k"));
        assert!(line.contains("↓   240"));
    }

    #[test]
    fn status_line_shows_split_token_counts() {
        let mut app = CliApp::new();
        app.displayed_input_tokens = 9_900;
        app.displayed_output_tokens = 1_300;

        let line = app.status_line().to_string();
        assert!(line.contains("↑  9.9k"));
        assert!(line.contains("↓  1.3k"));
    }

    #[test]
    fn token_slots_stay_fixed_width() {
        assert_eq!(format_token_slot(0).len(), 5);
        assert_eq!(format_token_slot(12).len(), 5);
        assert_eq!(format_token_slot(1_300).len(), 5);
        assert_eq!(format_duration_slot(7).len(), 6);
        assert_eq!(format_duration_slot(100).len(), 6);
    }

    #[test]
    fn assistant_markdown_tables_render_without_raw_rule_row() {
        let mut app = CliApp::new();
        app.apply_event(FrontendEvent::AssistantMessageCompleted {
            turn_id: 1,
            text: "| Name | Value |\n| --- | --- |\n| Alpha | 1 |\n| Beta | 22 |".to_string(),
        });

        let lines = app.drain_new_lines();
        let rendered: Vec<String> = lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect()
            })
            .collect();

        assert!(rendered.iter().any(|line| line.contains("Name")));
        assert!(rendered.iter().any(|line| line.contains("Alpha")));
        assert!(rendered.iter().any(|line| line.contains("Beta")));
        assert!(rendered.iter().any(|line| line.contains('┌')));
        assert!(rendered.iter().any(|line| line.contains('│')));
        assert!(rendered.iter().any(|line| line.contains('┼')));
        assert!(rendered.iter().any(|line| line.contains('┘')));
        assert!(!rendered.iter().any(|line| line.contains("| Alpha | 1 |")));
    }

    #[test]
    fn assistant_markdown_headings_render_without_hash_prefix() {
        let mut app = CliApp::new();
        app.apply_event(FrontendEvent::AssistantMessageCompleted {
            turn_id: 1,
            text: "# Main Title".to_string(),
        });

        let lines = app.drain_new_lines();
        let rendered: Vec<String> = lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect()
            })
            .collect();

        assert!(rendered.iter().any(|line| line.contains("Main Title")));
        assert!(!rendered.iter().any(|line| line.contains("# Main Title")));
    }

    #[test]
    fn summarize_todo_args_produces_human_readable_summary() {
        let summary = summarize_todo_args(
            r#"{"items":[{"id":1,"text":"Write tests","status":"pending"},{"id":2,"text":"Fix UI","status":"in_progress"},{"id":3,"text":"Ship","status":"completed"}]}"#,
        );

        assert!(summary.contains("3 items:"));
        assert!(summary.contains("Write tests"));
        assert!(!summary.contains(r#"{"items":"#));
    }

    #[test]
    fn todo_tool_does_not_render_as_scrolling_history_block() {
        let mut app = CliApp::new();
        app.apply_event(FrontendEvent::ToolCallStarted {
            turn_id: 1,
            call_id: "todo-1".to_string(),
            name: "todo".to_string(),
            args_preview: r#"{"items":[{"id":1,"text":"Pinned footer","status":"in_progress"}]}"#
                .to_string(),
        });
        app.apply_event(FrontendEvent::ToolCallFinished {
            turn_id: 1,
            call_id: "todo-1".to_string(),
            name: "todo".to_string(),
            result_preview: "[>] #1: Pinned footer".to_string(),
            is_error: false,
        });

        let lines = app.drain_new_lines();
        let rendered: Vec<String> = lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect()
            })
            .collect();

        assert!(!rendered.iter().any(|line| line.contains("Todo(")));
        assert!(!rendered.iter().any(|line| line.contains("Pinned footer")));
    }

    #[test]
    fn render_viewport_shows_pinned_todo_footer() {
        let mut app = CliApp::new();
        app.apply_event(FrontendEvent::TodoSnapshot {
            turn_id: 1,
            items: vec![FrontendTodoItem {
                id: 1,
                text: "Pinned footer".to_string(),
                status: FrontendTodoStatus::InProgress,
            }],
        });

        let height = app.desired_viewport_height();
        let buffer = render_buffer(&mut app, 40, height);
        let lines = buffer_text(&buffer);
        assert!(lines.iter().any(|line| line.contains("Pinned footer")));
        assert!(lines.iter().any(|line| line.contains("• Pinned footer")));
        assert!(lines.iter().any(|line| line.contains("■ Pinned footer")));
        assert!(lines.iter().any(|line| line.contains("> ")));
    }

    #[test]
    fn render_keeps_tools_panel_above_todo_when_both_visible() {
        let mut app = CliApp::new();
        app.apply_event(FrontendEvent::TodoSnapshot {
            turn_id: 1,
            items: vec![FrontendTodoItem {
                id: 1,
                text: "Pinned footer".to_string(),
                status: FrontendTodoStatus::InProgress,
            }],
        });
        app.apply_event(FrontendEvent::ToolCallStarted {
            turn_id: 1,
            call_id: "bash-1".to_string(),
            name: "bash".to_string(),
            args_preview: "{\"command\":\"cargo test\"}".to_string(),
        });

        let height = app.desired_viewport_height();
        let buffer = render_buffer(&mut app, 60, height);
        let lines = buffer_text(&buffer);

        assert!(lines[0].contains("Bash"));
        assert!(lines[0].contains("cargo test"));
        assert!(lines[1].trim().is_empty());
        assert!(lines[2].contains("Pinned footer"));
        assert!(lines[3].contains("■ Pinned footer"));
        assert!(lines[5..8].iter().any(|line| line.contains("> ")));
    }

    #[test]
    fn desired_viewport_height_grows_only_for_visible_panels() {
        let mut app = CliApp::new();
        assert_eq!(app.desired_viewport_height(), 4);

        app.apply_event(FrontendEvent::UserMessageCommitted {
            turn_id: 1,
            text: "hello".to_string(),
        });
        assert_eq!(app.desired_viewport_height(), 5);

        app.apply_event(FrontendEvent::AssistantMessageCompleted {
            turn_id: 1,
            text: "hi".to_string(),
        });
        assert_eq!(app.desired_viewport_height(), 4);

        app.apply_event(FrontendEvent::ToolCallStarted {
            turn_id: 1,
            call_id: "bash-1".to_string(),
            name: "bash".to_string(),
            args_preview: "{\"command\":\"cargo test\"}".to_string(),
        });
        assert_eq!(app.desired_viewport_height(), 5);

        app.apply_event(FrontendEvent::TodoSnapshot {
            turn_id: 1,
            items: vec![FrontendTodoItem {
                id: 1,
                text: "Pinned footer".to_string(),
                status: FrontendTodoStatus::InProgress,
            }],
        });
        assert_eq!(app.desired_viewport_height(), 8);
    }

    #[test]
    fn render_todo_footer_shows_all_items_without_fold() {
        let mut app = CliApp::new();
        app.apply_event(FrontendEvent::TodoSnapshot {
            turn_id: 1,
            items: vec![
                FrontendTodoItem {
                    id: 1,
                    text: "One".to_string(),
                    status: FrontendTodoStatus::Pending,
                },
                FrontendTodoItem {
                    id: 2,
                    text: "Two".to_string(),
                    status: FrontendTodoStatus::InProgress,
                },
                FrontendTodoItem {
                    id: 3,
                    text: "Three".to_string(),
                    status: FrontendTodoStatus::Completed,
                },
            ],
        });

        let height = app.desired_viewport_height();
        let buffer = render_buffer(&mut app, 40, height);
        let lines = buffer_text(&buffer);

        assert!(lines.iter().any(|line| line.contains("□ One")));
        assert!(lines.iter().any(|line| line.contains("■ Two")));
        assert!(lines.iter().any(|line| line.contains("✓ Three")));
        assert!(lines.iter().any(|line| line.contains("One")));
        assert!(lines.iter().any(|line| line.contains("Two")));
        assert!(lines.iter().any(|line| line.contains("Three")));
        assert!(!lines.iter().any(|line| line.contains("+1 more")));
    }

    #[test]
    fn todo_footer_connects_to_activity_line_when_spinner_visible() {
        let mut app = CliApp::new();
        app.apply_event(FrontendEvent::UserMessageCommitted {
            turn_id: 1,
            text: "hello".to_string(),
        });
        app.apply_event(FrontendEvent::TodoSnapshot {
            turn_id: 1,
            items: vec![
                FrontendTodoItem {
                    id: 1,
                    text: "Pending".to_string(),
                    status: FrontendTodoStatus::Pending,
                },
                FrontendTodoItem {
                    id: 2,
                    text: "Running".to_string(),
                    status: FrontendTodoStatus::InProgress,
                },
                FrontendTodoItem {
                    id: 3,
                    text: "Done".to_string(),
                    status: FrontendTodoStatus::Completed,
                },
            ],
        });

        let lines = app.render_todo_footer_lines(4);
        let rendered: Vec<String> = lines
            .iter()
            .map(|line| line.spans.iter().map(|span| span.content.as_ref()).collect())
            .collect();

        assert!(rendered[0].contains("└ Running"));
        assert!(rendered[1].contains("□ Pending"));
        assert!(rendered[2].contains("■ Running"));
        assert!(rendered[3].contains("✓ Done"));
    }

    #[test]
    fn todo_header_prefers_in_progress_then_pending() {
        let in_progress = todo_header_text(&[
            FrontendTodoItem {
                id: 1,
                text: "Done".to_string(),
                status: FrontendTodoStatus::Completed,
            },
            FrontendTodoItem {
                id: 2,
                text: "Running".to_string(),
                status: FrontendTodoStatus::InProgress,
            },
            FrontendTodoItem {
                id: 3,
                text: "Later".to_string(),
                status: FrontendTodoStatus::Pending,
            },
        ]);
        assert_eq!(in_progress, "Running");

        let pending = todo_header_text(&[
            FrontendTodoItem {
                id: 1,
                text: "Done".to_string(),
                status: FrontendTodoStatus::Completed,
            },
            FrontendTodoItem {
                id: 2,
                text: "Next".to_string(),
                status: FrontendTodoStatus::Pending,
            },
        ]);
        assert_eq!(pending, "Next");
    }

    #[test]
    fn subagent_todo_renders_as_independent_block() {
        let mut app = CliApp::new();
        app.apply_event(FrontendEvent::ToolCallStarted {
            turn_id: 1,
            call_id: "task-1".to_string(),
            name: "task".to_string(),
            args_preview: r#"{"prompt":"Analyze repository health"}"#.to_string(),
        });
        app.apply_event(FrontendEvent::SubagentTodoSnapshot {
            turn_id: 1,
            call_id: "task-1".to_string(),
            items: vec![
                FrontendTodoItem {
                    id: 1,
                    text: "Scan modules".to_string(),
                    status: FrontendTodoStatus::Completed,
                },
                FrontendTodoItem {
                    id: 2,
                    text: "Summarize risks".to_string(),
                    status: FrontendTodoStatus::InProgress,
                },
            ],
        });

        let rendered: Vec<String> = app
            .render_todo_footer_lines(4)
            .iter()
            .map(|line| line.spans.iter().map(|span| span.content.as_ref()).collect())
            .collect();

        assert!(rendered[0].contains("Analyze repository health"));
        assert!(rendered[1].contains("✓ Scan modules"));
        assert!(rendered[2].contains("■ Summarize risks"));
    }

    #[test]
    fn subagent_todo_renders_above_main_todo() {
        let mut app = CliApp::new();
        app.apply_event(FrontendEvent::TodoSnapshot {
            turn_id: 1,
            items: vec![FrontendTodoItem {
                id: 1,
                text: "Main todo".to_string(),
                status: FrontendTodoStatus::InProgress,
            }],
        });
        app.apply_event(FrontendEvent::ToolCallStarted {
            turn_id: 1,
            call_id: "task-1".to_string(),
            name: "task".to_string(),
            args_preview: r#"{"prompt":"Analyze repository health"}"#.to_string(),
        });
        app.apply_event(FrontendEvent::SubagentTodoSnapshot {
            turn_id: 1,
            call_id: "task-1".to_string(),
            items: vec![FrontendTodoItem {
                id: 1,
                text: "Subagent todo".to_string(),
                status: FrontendTodoStatus::InProgress,
            }],
        });

        let rendered: Vec<String> = app
            .render_todo_footer_lines(5)
            .iter()
            .map(|line| line.spans.iter().map(|span| span.content.as_ref()).collect())
            .collect();

        let subagent_index = rendered
            .iter()
            .position(|line| line.contains("Analyze repository health"))
            .expect("subagent header");
        let main_index = rendered
            .iter()
            .position(|line| line.contains("Main todo"))
            .expect("main header");

        assert!(subagent_index < main_index);
        assert!(rendered.iter().any(|line| line.contains("■ Subagent todo")));
        assert!(rendered.iter().any(|line| line.contains("■ Main todo")));
    }

    #[test]
    fn drain_new_lines_returns_timeline_blocks_as_styled_lines() {
        let mut app = CliApp::new();
        app.apply_event(FrontendEvent::UserMessageCommitted {
            turn_id: 1,
            text: "hello".to_string(),
        });

        let lines = app.drain_new_lines();
        assert!(!lines.is_empty());
        // Should contain "> hello" and an empty line
        assert!(lines
            .iter()
            .any(|l| { l.spans.iter().any(|s| s.content.contains("hello")) }));

        // Second call returns nothing
        let lines2 = app.drain_new_lines();
        assert!(lines2.is_empty());
    }

    #[test]
    fn todo_auto_collapses_when_all_completed() {
        let mut app = CliApp::new();

        app.apply_event(FrontendEvent::TodoSnapshot {
            turn_id: 1,
            items: vec![FrontendTodoItem {
                id: 1,
                text: "Task one".to_string(),
                status: FrontendTodoStatus::Pending,
            }],
        });
        assert!(app.todo_was_active);

        app.apply_event(FrontendEvent::TodoSnapshot {
            turn_id: 1,
            items: vec![FrontendTodoItem {
                id: 1,
                text: "Task one".to_string(),
                status: FrontendTodoStatus::Completed,
            }],
        });

        assert!(!app.todo_was_active);
        assert!(app.timeline.iter().any(|block| {
            matches!(block, TimelineBlock::Note { message, .. } if message.contains("Completed todos"))
        }));
    }

    #[test]
    fn todo_footer_shows_when_mixed_status() {
        let mut app = CliApp::new();

        app.apply_event(FrontendEvent::TodoSnapshot {
            turn_id: 1,
            items: vec![
                FrontendTodoItem {
                    id: 1,
                    text: "Done".to_string(),
                    status: FrontendTodoStatus::Completed,
                },
                FrontendTodoItem {
                    id: 2,
                    text: "Pending".to_string(),
                    status: FrontendTodoStatus::Pending,
                },
            ],
        });

        assert!(app.todo_was_active);
    }

    #[test]
    fn tool_call_error_status_is_updated_correctly() {
        let mut app = CliApp::new();

        // Start a tool call
        app.apply_event(FrontendEvent::ToolCallStarted {
            turn_id: 1,
            call_id: "read-error-1".to_string(),
            name: "read".to_string(),
            args_preview: "/nonexistent/file.txt".to_string(),
        });

        // Verify initial state (is_error should be false)
        let tool = match &app.timeline[0] {
            TimelineBlock::Tool(t) => t,
            other => panic!("expected tool block, got {other:?}"),
        };
        assert!(!tool.is_error, "is_error should be false initially");

        // Finish with an error
        app.apply_event(FrontendEvent::ToolCallFinished {
            turn_id: 1,
            call_id: "read-error-1".to_string(),
            name: "read".to_string(),
            result_preview: "Failed to read file: No such file or directory (os error 2)"
                .to_string(),
            is_error: true,
        });

        // Verify is_error was updated to true
        let tool = match &app.timeline[0] {
            TimelineBlock::Tool(t) => t,
            other => panic!("expected tool block, got {other:?}"),
        };
        assert!(
            tool.is_error,
            "is_error should be updated to true when tool fails"
        );
        assert_eq!(
            tool.result_preview.as_deref(),
            Some("Failed to read file: No such file or directory (os error 2)")
        );
    }
}
