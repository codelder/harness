use crate::cli::spinner::Spinner;
use crate::cli::theme::CliTheme;
use crate::frontend::{FrontendCommand, FrontendEvent, FrontendTodoItem, FrontendTodoStatus};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::prelude::{Frame, Line, Span};
use ratatui::widgets::{Paragraph, Wrap};
use serde::Deserialize;
use the_other_tui_markdown::{into_text_with_renderer as render_markdown, RendererBuilder};
use unicode_width::UnicodeWidthStr;

const TOOL_PANEL_MAX_ROWS: u16 = 2;
const COMPOSER_ROWS: u16 = 3;
const STATUS_ROWS: u16 = 1;
const COMPOSER_PROMPT: &str = "> ";

#[derive(Debug, Clone, PartialEq, Eq)]
struct ToolBlock {
    turn_id: u64,
    call_id: String,
    name: String,
    args_preview: String,
    result_preview: Option<String>,
    is_error: bool,
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
    input_tokens: u64,
    output_tokens: u64,
    /// Index of the next timeline block to insert above the viewport.
    last_inserted_index: usize,
    /// Result lines for tool blocks that were already drained before their result arrived.
    pending_tool_result_lines: Vec<Line<'static>>,
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
            input_tokens: 0,
            output_tokens: 0,
            last_inserted_index: 0,
            pending_tool_result_lines: Vec::new(),
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
                    }));
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
                self.input_tokens = input_tokens;
                self.output_tokens = output_tokens;
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
                self.timeline.push(TimelineBlock::Note {
                    turn_id: None,
                    kind: NoteKind::Status,
                    message: message.clone(),
                });
                self.status = message;
            }
            FrontendEvent::Error { turn_id, message } => {
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
        self.executing_tool_count().min(TOOL_PANEL_MAX_ROWS)
    }

    fn visible_todo_rows(&self) -> u16 {
        if !self.should_show_todo_footer() {
            return 0;
        }

        1 + u16::try_from(self.todo_footer.len()).unwrap_or(u16::MAX.saturating_sub(1))
    }

    fn tool_todo_gap_rows(&self) -> u16 {
        if self.visible_tool_rows() > 0 && self.visible_todo_rows() > 0 {
            1
        } else {
            0
        }
    }

    /// Desired viewport height: composer + status, plus only the rows currently needed
    /// for executing tools and the pinned todo panel.
    pub fn desired_viewport_height(&self) -> u16 {
        COMPOSER_ROWS
            + STATUS_ROWS
            + self.visible_tool_rows()
            + self.tool_todo_gap_rows()
            + self.visible_todo_rows()
    }

    /// Render executing tools as stable viewport lines.
    pub fn render_executing_tools_lines(&mut self, max_lines: usize) -> Vec<Line<'static>> {
        // Collect executing tools info
        let executing: Vec<(String, String)> = self
            .timeline
            .iter()
            .filter_map(|block| match block {
                TimelineBlock::Tool(t) if t.result_preview.is_none() && t.name != "todo" => {
                    let args = if t.args_preview.is_empty() {
                        String::new()
                    } else {
                        format!("({})", summarize_tool_args(&t.name, &t.args_preview, 40))
                    };
                    Some((capitalize_first(&t.name), args))
                }
                _ => None,
            })
            .take(max_lines)
            .collect();

        if executing.is_empty() {
            return Vec::new();
        }

        let spinner_frame = self.spinner.frame();
        let mut lines = Vec::new();

        for (tool_name, args) in &executing {
            lines.push(Line::from(vec![
                Span::styled(
                    format!("{} ", spinner_frame),
                    self.theme.tool_executing_indicator,
                ),
                Span::styled(tool_name.clone(), self.theme.tool),
                Span::styled(args.clone(), self.theme.tool_result),
            ]));
        }

        lines
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
                TimelineBlock::Tool(t) if t.result_preview.is_none() && t.name != "todo" => {
                    let args = if t.args_preview.is_empty() {
                        String::new()
                    } else {
                        format!("({})", summarize_tool_args(&t.name, &t.args_preview, 40))
                    };
                    Some((capitalize_first(&t.name), args))
                }
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
        let tool_rows = self.visible_tool_rows();
        let gap_rows = self.tool_todo_gap_rows();
        let todo_rows = self.visible_todo_rows();

        let mut constraints = Vec::new();
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
        if self.streaming_assistant.is_none() && composer_area.height > 1 && composer_area.width > 0 {
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
        let token_info = if self.input_tokens > 0 || self.output_tokens > 0 {
            format!("{} in / {} out | ", self.input_tokens, self.output_tokens)
        } else {
            String::new()
        };

        let status = if self.awaiting_assistant || self.streaming_assistant.is_some() {
            self.spinner.status_text()
        } else {
            self.status.clone()
        };

        Line::from(format!(
            "{}{} | Enter submit | Ctrl+C interrupt | Esc exit",
            token_info, status
        ))
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
        if !self.should_show_todo_footer() || max_lines == 0 {
            return Vec::new();
        }

        let completed = self
            .todo_footer
            .iter()
            .filter(|item| item.status == FrontendTodoStatus::Completed)
            .count();
        let total = self.todo_footer.len();

        let mut lines = vec![Line::styled(
            format!("Tasks ({completed}/{total})"),
            self.theme.footer,
        )];

        let item_rows = max_lines.saturating_sub(1);
        for item in self.todo_footer.iter().take(item_rows) {
            lines.push(Line::styled(
                format!("  {}", format_todo_item(item)),
                self.theme.footer,
            ));
        }

        lines
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
                for (i, line) in result.lines().enumerate() {
                    if i == 0 {
                        lines.push(Line::from(vec![
                            Span::styled("  \u{2514} ", result_style),
                            Span::styled(line.to_string(), result_style),
                        ]));
                    } else {
                        lines.push(Line::styled(format!("    {}", line), result_style));
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
        if tool.name == "todo" {
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
            } else {
                let result_style = if tool.is_error {
                    self.theme.error
                } else {
                    self.theme.tool_result
                };
                for (i, line) in result.lines().enumerate() {
                    if i == 0 {
                        lines.push(Line::from(vec![
                            Span::styled("  \u{25cf} ", completion_style),
                            Span::styled(line.to_string(), result_style),
                        ]));
                    } else {
                        lines.push(Line::styled(format!("    {}", line), result_style));
                    }
                }
            }
            lines.push(Line::raw(""));
        }
        lines
    }
}

/// Detect whether a tool result looks like an error message.

fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

fn truncate_str(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let end = s.floor_char_boundary(max.saturating_sub(3));
    format!("{}...", &s[..end])
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

fn summarize_tool_args(tool_name: &str, args: &str, max: usize) -> String {
    let summary = match tool_name {
        "todo" => summarize_todo_args(args),
        _ => args.to_string(),
    };
    truncate_str(&summary, max)
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

fn format_todo_item(item: &FrontendTodoItem) -> String {
    let status = match item.status {
        FrontendTodoStatus::Pending => "[ ]",
        FrontendTodoStatus::InProgress => "[>]",
        FrontendTodoStatus::Completed => "[x]",
    };
    format!("{status} #{}: {}", item.id, item.text)
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
    fn waiting_for_assistant_shows_spinner_status() {
        let mut app = CliApp::new();
        app.apply_event(FrontendEvent::UserMessageCommitted {
            turn_id: 1,
            text: "hello".to_string(),
        });

        let status = app.status_line().to_string();
        assert!(crate::cli::spinner::FUN_MESSAGES
            .iter()
            .any(|message| status.contains(message)));
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
        assert!(lines.iter().any(|line| line.contains("Tasks (0/1)")));
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
        assert!(lines[2].contains("Tasks (0/1)"));
        assert!(lines[3].contains("Pinned footer"));
        assert!(lines[5..8].iter().any(|line| line.contains("> ")));
    }

    #[test]
    fn desired_viewport_height_grows_only_for_visible_panels() {
        let mut app = CliApp::new();
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

        assert!(lines.iter().any(|line| line.contains("One")));
        assert!(lines.iter().any(|line| line.contains("Two")));
        assert!(lines.iter().any(|line| line.contains("Three")));
        assert!(!lines.iter().any(|line| line.contains("+1 more")));
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
        assert_eq!(tool.is_error, false, "is_error should be false initially");

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
        assert_eq!(
            tool.is_error, true,
            "is_error should be updated to true when tool fails"
        );
        assert_eq!(
            tool.result_preview.as_deref(),
            Some("Failed to read file: No such file or directory (os error 2)")
        );
    }
}
