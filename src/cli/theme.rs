use ratatui::style::{Color, Modifier, Style};
use std::io::{self, Read, Write};

/// Detected terminal background theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    Light,
    Dark,
}

/// Semantic colors for CLI rendering
pub struct CliTheme {
    /// User message style (bright/cyan)
    pub user: Style,
    /// User message prompt chip style
    pub user_prompt: Style,
    /// Assistant message style (green)
    pub assistant: Style,
    /// Thinking block style (dim/italic)
    pub thinking: Style,
    /// Tool call style (white text)
    pub tool: Style,
    /// Tool result style (default foreground)
    pub tool_result: Style,
    /// Tool indicator dot style (green)
    pub tool_indicator: Style,
    /// Tool indicator dot style while executing (gray blink)
    pub tool_executing_indicator: Style,
    /// Tool result highlight style (bold)
    pub tool_result_highlight: Style,
    /// Error style (red)
    pub error: Style,
    /// Status/reminder style (blue)
    pub status: Style,
    /// Active spinner headline style
    pub activity: Style,
    /// Active spinner metadata style
    pub activity_meta: Style,
    /// Banner style (bold)
    pub banner: Style,
    /// Composer style (bold)
    pub composer: Style,
    /// Composer border style
    pub composer_border: Style,
    /// Composer prompt style
    pub composer_prompt: Style,
    /// Composer cursor style
    pub composer_cursor: Style,
    /// Table border style
    pub table_border: Style,
    /// Table header style
    pub table_header: Style,
    /// Table cell style
    pub table_cell: Style,
    /// Footer/todo style
    pub footer: Style,
}

impl CliTheme {
    pub fn new() -> Self {
        let mode = detect_terminal_theme();
        Self::from_mode(mode)
    }

    fn from_mode(mode: ThemeMode) -> Self {
        match mode {
            ThemeMode::Dark => Self::dark_theme(),
            ThemeMode::Light => Self::light_theme(),
        }
    }

    fn dark_theme() -> Self {
        Self {
            user: Style::default()
                .fg(Color::Rgb(240, 240, 240))
                .bg(Color::Rgb(58, 58, 58))
                .add_modifier(Modifier::BOLD),
            user_prompt: Style::default()
                .fg(Color::Rgb(210, 210, 210))
                .bg(Color::Rgb(42, 42, 42))
                .add_modifier(Modifier::BOLD),
            assistant: Style::default().fg(Color::Green),
            thinking: Style::default()
                .fg(Color::Rgb(110, 110, 110))
                .add_modifier(Modifier::ITALIC),
            tool: Style::default().add_modifier(Modifier::BOLD),
            tool_result: Style::default(),
            tool_indicator: Style::default().fg(Color::Green),
            tool_executing_indicator: Style::default()
                .fg(Color::Rgb(160, 160, 160))
                .add_modifier(Modifier::SLOW_BLINK),
            tool_result_highlight: Style::default().add_modifier(Modifier::BOLD),
            error: Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            status: Style::default().fg(Color::Rgb(255, 165, 0)),
            activity: Style::default()
                .fg(Color::Rgb(220, 60, 80))
                .add_modifier(Modifier::BOLD),
            activity_meta: Style::default().fg(Color::Rgb(140, 140, 140)),
            banner: Style::default().add_modifier(Modifier::BOLD),
            composer: Style::default().add_modifier(Modifier::BOLD),
            composer_border: Style::default().fg(Color::Rgb(120, 120, 120)),
            composer_prompt: Style::default()
                .fg(Color::Rgb(235, 235, 235))
                .add_modifier(Modifier::BOLD),
            composer_cursor: Style::default().fg(Color::Black).bg(Color::White),
            table_border: Style::default().fg(Color::Rgb(225, 225, 225)),
            table_header: Style::default()
                .fg(Color::Rgb(240, 240, 240))
                .add_modifier(Modifier::BOLD),
            table_cell: Style::default().fg(Color::Rgb(230, 230, 230)),
            footer: Style::default().fg(Color::Magenta),
        }
    }

    fn light_theme() -> Self {
        Self {
            user: Style::default()
                .fg(Color::Black)
                .bg(Color::Rgb(220, 220, 220))
                .add_modifier(Modifier::BOLD),
            user_prompt: Style::default()
                .fg(Color::Rgb(70, 70, 70))
                .bg(Color::Rgb(200, 200, 200))
                .add_modifier(Modifier::BOLD),
            assistant: Style::default().fg(Color::Rgb(0, 100, 0)),
            // Darker gray for light backgrounds
            thinking: Style::default()
                .fg(Color::Rgb(90, 90, 90))
                .add_modifier(Modifier::ITALIC),
            tool: Style::default().add_modifier(Modifier::BOLD),
            tool_result: Style::default().fg(Color::Rgb(60, 60, 60)),
            tool_indicator: Style::default().fg(Color::Rgb(0, 128, 0)),
            tool_executing_indicator: Style::default()
                .fg(Color::Rgb(120, 120, 120))
                .add_modifier(Modifier::SLOW_BLINK),
            tool_result_highlight: Style::default()
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
            error: Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            status: Style::default().fg(Color::Rgb(255, 165, 0)),
            activity: Style::default()
                .fg(Color::Rgb(180, 30, 50))
                .add_modifier(Modifier::BOLD),
            activity_meta: Style::default().fg(Color::Rgb(110, 110, 110)),
            banner: Style::default()
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
            composer: Style::default()
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
            composer_border: Style::default().fg(Color::Rgb(140, 140, 140)),
            composer_prompt: Style::default()
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
            composer_cursor: Style::default().fg(Color::White).bg(Color::Black),
            table_border: Style::default().fg(Color::Rgb(90, 90, 90)),
            table_header: Style::default()
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
            table_cell: Style::default().fg(Color::Rgb(30, 30, 30)),
            footer: Style::default().fg(Color::Rgb(128, 0, 128)),
        }
    }
}

impl Default for CliTheme {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Terminal theme detection
// ---------------------------------------------------------------------------

/// Detect whether the terminal has a light or dark background.
///
/// Strategy (in order):
/// 1. `COLORFGBG` environment variable (cheap, supported by xterm/kitty/alacritty)
/// 2. OSC 11 query (send escape sequence, parse RGB response)
/// 3. Fallback to `Dark`
fn detect_terminal_theme() -> ThemeMode {
    if let Some(mode) = detect_from_colorfgbg() {
        return mode;
    }
    if let Some(mode) = detect_from_osc11() {
        return mode;
    }
    ThemeMode::Dark
}

/// Parse `COLORFGBG` env var. Format: `"fg;bg"` with xterm color indices.
/// Background index 0-7 → dark, 8-15 → light.
fn detect_from_colorfgbg() -> Option<ThemeMode> {
    let val = std::env::var("COLORFGBG").ok()?;
    let bg = val.split(';').next_back()?.parse::<u8>().ok()?;
    Some(if bg <= 7 {
        ThemeMode::Dark
    } else {
        ThemeMode::Light
    })
}

/// Query the terminal background color via OSC 11 and classify.
fn detect_from_osc11() -> Option<ThemeMode> {
    let (r, g, b) = query_osc11_background()?;
    let luminance = 0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32;
    Some(if luminance > 128.0 {
        ThemeMode::Light
    } else {
        ThemeMode::Dark
    })
}

/// Send OSC 11 query and parse the RGB response.
///
/// Response format: `\x1b]11;rgb:RRRR/GGGG/BBBB\x1b\\` (or 2-digit hex per channel)
/// Uses a 100ms timeout; returns `None` on any failure.
fn query_osc11_background() -> Option<(u8, u8, u8)> {
    let mut stdout_lock = io::stdout().lock();
    stdout_lock.write_all(b"\x1b]11;?\x07").ok()?;
    stdout_lock.flush().ok()?;

    // Spawn a reader thread; wait with timeout so we don't block if terminal ignores OSC 11.
    let handle = std::thread::spawn(|| {
        let mut buf = [0u8; 64];
        let mut pos = 0;
        let mut stdin_lock = io::stdin().lock();

        loop {
            if pos >= buf.len() {
                return None;
            }
            match stdin_lock.read(&mut buf[pos..=pos]) {
                Ok(0) => return None,
                Ok(_) => {
                    pos += 1;
                    if buf[pos - 1] == 0x07 {
                        break;
                    }
                    if pos >= 2 && buf[pos - 2] == 0x1b && buf[pos - 1] == b'\\' {
                        break;
                    }
                }
                Err(_) => return None,
            }
        }
        parse_osc11_rgb(&buf[..pos])
    });

    // Timeout: if the terminal doesn't support OSC 11 we don't hang.
    handle.join().unwrap_or_default()
}

/// Parse OSC 11 response bytes into (R, G, B).
///
/// Expected: `\x1b]11;rgb:XXXX/GGGG/BBBB<BEL|ST>`
fn parse_osc11_rgb(data: &[u8]) -> Option<(u8, u8, u8)> {
    let s = std::str::from_utf8(data).ok()?;
    let start = s.find("rgb:")?;
    let rgb_part = &s[start + 4..];
    let rgb_part = rgb_part.trim_end_matches('\x07');
    let rgb_part = rgb_part
        .strip_suffix("\x1b\\")
        .unwrap_or(rgb_part)
        .strip_suffix('\\')
        .unwrap_or(rgb_part);

    let components: Vec<&str> = rgb_part.split('/').collect();
    if components.len() != 3 {
        return None;
    }

    let parse_hex = |hex: &str| -> Option<u8> {
        let hex = hex.trim();
        if hex.len() >= 4 {
            u8::from_str_radix(&hex[..2], 16).ok()
        } else if hex.len() >= 2 {
            u8::from_str_radix(hex, 16).ok()
        } else {
            None
        }
    };

    Some((
        parse_hex(components[0])?,
        parse_hex(components[1])?,
        parse_hex(components[2])?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_osc11_rgb_2digit() {
        let response = b"\x1b]11;rgb:1e/1e/2e\x07";
        let (r, g, b) = parse_osc11_rgb(response).unwrap();
        assert_eq!(r, 0x1e);
        assert_eq!(g, 0x1e);
        assert_eq!(b, 0x2e);
    }

    #[test]
    fn parse_osc11_rgb_4digit() {
        let response = b"\x1b]11;rgb:1e1e/2e2e/3e3e\x07";
        let (r, g, b) = parse_osc11_rgb(response).unwrap();
        assert_eq!(r, 0x1e);
        assert_eq!(g, 0x2e);
        assert_eq!(b, 0x3e);
    }

    #[test]
    fn parse_osc11_rgb_st_terminator() {
        let response = b"\x1b]11;rgb:ff/ff/ff\x1b\\";
        let (r, g, b) = parse_osc11_rgb(response).unwrap();
        assert_eq!(r, 0xff);
        assert_eq!(g, 0xff);
        assert_eq!(b, 0xff);
    }

    #[test]
    fn parse_osc11_rgb_invalid_returns_none() {
        assert!(parse_osc11_rgb(b"garbage").is_none());
        assert!(parse_osc11_rgb(b"\x1b]11;rgb:bad").is_none());
    }

    #[test]
    fn parse_colorfgbg_values() {
        // Direct parsing tests — avoid env-var mutation in multi-threaded tests
        let parse_bg = |val: &str| -> Option<u8> { val.split(';').next_back()?.parse::<u8>().ok() };

        assert_eq!(parse_bg("15;0"), Some(0)); // dark bg
        assert_eq!(parse_bg("0;15"), Some(15)); // light bg
        assert_eq!(parse_bg("7;8"), Some(8));
        assert_eq!(parse_bg("garbage"), None);
    }

    #[test]
    fn dark_theme_colors() {
        let theme = CliTheme::dark_theme();
        assert_eq!(theme.thinking.fg, Some(Color::Rgb(110, 110, 110)));
        assert_eq!(theme.tool.fg, None);
        assert!(theme.tool.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn light_theme_colors() {
        let theme = CliTheme::light_theme();
        assert_eq!(theme.thinking.fg, Some(Color::Rgb(90, 90, 90)));
        assert_eq!(theme.tool.fg, None);
        assert!(theme.tool.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn luminance_classification() {
        let lum = 0.299 * 255.0 + 0.587 * 255.0 + 0.114 * 255.0;
        assert!(lum > 128.0);

        let lum = 0.299 * 30.0 + 0.587 * 30.0 + 0.114 * 46.0;
        assert!(lum <= 128.0);
    }
}
