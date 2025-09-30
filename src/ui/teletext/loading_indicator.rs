//! Loading indicator for terminal UI

/// Simple ASCII loading indicator with rotating animation
#[derive(Debug, Clone)]
pub struct LoadingIndicator {
    message: String,
    frame: usize,
    frames: Vec<&'static str>,
}

impl LoadingIndicator {
    /// Creates a new loading indicator with the specified message
    pub fn new(message: String) -> Self {
        Self {
            message,
            frame: 0,
            frames: vec!["|", "/", "-", "\\"],
        }
    }

    /// Gets the current animation frame character
    pub fn current_frame(&self) -> &str {
        self.frames[self.frame]
    }

    /// Gets the loading message
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Advances to the next animation frame
    pub fn next_frame(&mut self) {
        self.frame = (self.frame + 1) % self.frames.len();
    }
}
