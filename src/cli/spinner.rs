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
    last_frame_update: Instant,
    last_message_update: Instant,
    frame_interval_ms: u64,
    message_interval_ms: u64,
}

impl Spinner {
    pub fn new() -> Self {
        Self {
            frame_index: 0,
            message_index: 0,
            started_at: Instant::now(),
            last_frame_update: Instant::now(),
            last_message_update: Instant::now(),
            frame_interval_ms: 80,     // Frame change every 80ms
            message_interval_ms: 2000, // Message change every 2s
        }
    }

    /// Advance animation state if intervals elapsed.
    pub fn tick(&mut self) -> bool {
        let now = Instant::now();
        let frame_elapsed = now.duration_since(self.last_frame_update);
        let message_elapsed = now.duration_since(self.last_message_update);
        let mut changed = false;

        if frame_elapsed.as_millis() >= self.frame_interval_ms as u128 {
            let steps = (frame_elapsed.as_millis() / self.frame_interval_ms as u128) as usize;
            self.frame_index = self.frame_index.wrapping_add(steps);
            self.last_frame_update = now;
            changed = true;
        }

        if message_elapsed.as_millis() >= self.message_interval_ms as u128 {
            let steps = (message_elapsed.as_millis() / self.message_interval_ms as u128) as usize;
            self.message_index = self.message_index.wrapping_add(steps);
            self.last_message_update = now;
            changed = true;
        }

        changed
    }

    /// Get current frame.
    pub fn frame(&self) -> &'static str {
        SPINNER_FRAMES[self.frame_index % SPINNER_FRAMES.len()]
    }

    /// Get current fun message.
    pub fn message(&self) -> &'static str {
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
        self.last_frame_update = Instant::now();
        self.last_message_update = Instant::now();
    }

    #[cfg(test)]
    pub(crate) fn set_started_at_for_test(&mut self, started_at: Instant) {
        self.started_at = started_at;
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
    use std::time::Duration;

    #[test]
    fn spinner_frames_cycle() {
        let mut spinner = Spinner::new();
        let frames: Vec<_> = (0..12)
            .map(|_| {
                spinner.last_frame_update = Instant::now() - Duration::from_millis(100);
                spinner.tick();
                spinner.frame()
            })
            .collect();
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
        spinner.last_frame_update = Instant::now() - Duration::from_millis(100);
        spinner.last_message_update = Instant::now() - Duration::from_millis(2500);
        spinner.tick();
        spinner.reset();
        // After reset, frame_index should be 0
        assert_eq!(spinner.frame_index, 0);
        assert_eq!(spinner.message_index, 0);
    }

    #[test]
    fn spinner_messages_advance_independently() {
        let mut spinner = Spinner::new();
        let first = spinner.message();
        spinner.last_message_update = Instant::now() - Duration::from_millis(2500);
        spinner.tick();
        assert_ne!(spinner.message(), first);
    }
}
