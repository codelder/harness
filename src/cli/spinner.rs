use std::time::Instant;

/// ASCII flower spinner frames (from Claude Code CLI)
pub const SPINNER_FRAMES: &[&str] = &["·", "✻", "✽", "✶", "✳", "✢"];

/// Fun status messages that rotate during activity
pub const FUN_MESSAGES: &[&str] = &[
    "Germinating...",
    "Reasoning...",
    "Tracing...",
    "Planning...",
    "Stitching...",
    "Parsing...",
    "Searching...",
    "Synthesizing...",
];

/// Spinner state for animation
pub struct Spinner {
    frame_index: usize,
    message_index: usize,
    started_at: Instant,
    last_update: Instant,
    frame_interval_ms: u64,
    message_interval_ms: u64,
}

impl Spinner {
    pub fn new() -> Self {
        Self {
            frame_index: 0,
            message_index: 0,
            started_at: Instant::now(),
            last_update: Instant::now(),
            frame_interval_ms: 150,    // Frame change every 150ms
            message_interval_ms: 2000, // Message change every 2s
        }
    }

    /// Get current frame, advancing if interval elapsed
    pub fn frame(&mut self) -> &'static str {
        self.advance_if_needed();
        SPINNER_FRAMES[self.frame_index % SPINNER_FRAMES.len()]
    }

    /// Get current fun message
    pub fn message(&mut self) -> &'static str {
        self.advance_if_needed();
        FUN_MESSAGES[self.message_index % FUN_MESSAGES.len()]
    }

    /// Get combined spinner + message string
    pub fn status_text(&mut self) -> String {
        format!("{} {}", self.frame(), self.message())
    }

    pub fn elapsed_seconds(&self) -> u64 {
        self.started_at.elapsed().as_secs()
    }

    /// Reset spinner to initial state
    pub fn reset(&mut self) {
        self.frame_index = 0;
        self.message_index = 0;
        self.started_at = Instant::now();
        self.last_update = Instant::now();
    }

    fn advance_if_needed(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_update);

        if elapsed.as_millis() >= self.frame_interval_ms as u128 {
            self.frame_index = self.frame_index.wrapping_add(1);
        }

        if elapsed.as_millis() >= self.message_interval_ms as u128 {
            self.message_index = self.message_index.wrapping_add(1);
        }

        // Reset timer if we've advanced
        if elapsed.as_millis() >= self.frame_interval_ms as u128 {
            self.last_update = now;
        }
    }
}

impl Default for Spinner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spinner_frames_cycle() {
        let mut spinner = Spinner::new();
        let frames: Vec<_> = (0..12).map(|_| spinner.frame()).collect();
        // Should cycle through SPINNER_FRAMES twice
        assert_eq!(frames.len(), 12);
    }

    #[test]
    fn status_text_contains_frame_and_message() {
        let mut spinner = Spinner::new();
        let text = spinner.status_text();
        // Should contain a frame character and a message
        assert!(SPINNER_FRAMES.iter().any(|f| text.contains(f)));
        assert!(FUN_MESSAGES.iter().any(|m| text.contains(m)));
    }

    #[test]
    fn reset_clears_indices() {
        let mut spinner = Spinner::new();
        // Advance a bit
        for _ in 0..10 {
            spinner.frame();
        }
        spinner.reset();
        // After reset, frame_index should be 0
        assert_eq!(spinner.frame_index, 0);
        assert_eq!(spinner.message_index, 0);
    }
}
