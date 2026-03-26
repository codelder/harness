use ratatui::style::{Color, Modifier, Style};

/// Semantic colors for CLI rendering
pub struct CliTheme {
    /// User message style (bright/cyan)
    pub user: Style,
    /// Assistant message style (green)
    pub assistant: Style,
    /// Thinking block style (dim/italic)
    pub thinking: Style,
    /// Tool call style (yellow)
    pub tool: Style,
    /// Tool result style (dim yellow)
    pub tool_result: Style,
    /// Error style (red)
    pub error: Style,
    /// Status/reminder style (blue)
    pub status: Style,
    /// Banner style (bold)
    pub banner: Style,
    /// Composer style (bold)
    pub composer: Style,
    /// Footer/todo style
    pub footer: Style,
}

impl Default for CliTheme {
    fn default() -> Self {
        Self {
            user: Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            assistant: Style::default().fg(Color::Green),
            thinking: Style::default().fg(Color::Gray).add_modifier(Modifier::ITALIC),
            tool: Style::default().fg(Color::Yellow),
            tool_result: Style::default().fg(Color::DarkGray),
            error: Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            status: Style::default().fg(Color::Blue),
            banner: Style::default().add_modifier(Modifier::BOLD),
            composer: Style::default().add_modifier(Modifier::BOLD),
            footer: Style::default().fg(Color::Magenta),
        }
    }
}

impl CliTheme {
    pub fn new() -> Self {
        Self::default()
    }
}
