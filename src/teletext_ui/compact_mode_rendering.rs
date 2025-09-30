// src/teletext_ui/compact_mode_rendering.rs - Compact mode rendering logic

use super::core::{TeletextPage, TeletextRow};
use crate::teletext_ui::CONTENT_MARGIN;
use crate::ui::teletext::compact_display::{
    CompactDisplayConfig, CompactModeValidation, TerminalWidthValidation,
};

impl TeletextPage {
    /// Renders game content in compact mode with multiple games per line.
    /// This mode is optimized for displaying many games in limited vertical space.
    ///
    /// # Arguments
    /// * `buffer` - The string buffer to append rendered content to
    /// * `visible_rows` - The rows to render
    /// * `width` - Terminal width in characters
    /// * `current_line` - Current line position (mutable reference)
    /// * `text_fg_code` - Text foreground color code
    pub fn render_compact_mode_content(
        &self,
        buffer: &mut String,
        visible_rows: &[&TeletextRow],
        width: usize,
        current_line: &mut usize,
        text_fg_code: u8,
    ) {
        let config = CompactDisplayConfig::default();
        let validation = config.validate_terminal_width(width);

        match validation {
            TerminalWidthValidation::Sufficient {
                current_width: _,
                required_width: _,
                excess: _,
            } => {
                // Terminal is wide enough for compact mode
                let compact_lines =
                    self.group_games_for_compact_display(visible_rows, &config, width);

                // Check for compatibility warnings
                let compatibility = self.validate_compact_mode_compatibility();
                if let CompactModeValidation::CompatibleWithWarnings { warnings } = compatibility {
                    // Display warnings at the top of compact content
                    for (warning_index, warning) in warnings.iter().enumerate() {
                        buffer.push_str(&format!(
                            "\x1b[{};{}H\x1b[38;5;{}m⚠ {} (compact mode)\x1b[0m",
                            *current_line + warning_index,
                            CONTENT_MARGIN + 1,
                            text_fg_code,
                            warning
                        ));
                    }
                    *current_line += warnings.len();
                }

                for (line_index, compact_line) in compact_lines.iter().enumerate() {
                    buffer.push_str(&format!(
                        "\x1b[{};{}H{}",
                        *current_line + line_index,
                        CONTENT_MARGIN + 1,
                        compact_line
                    ));
                }
                *current_line += compact_lines.len();
            }
            TerminalWidthValidation::Insufficient {
                current_width,
                required_width,
                shortfall,
            } => {
                // Terminal is too narrow for compact mode - show detailed error message
                self.render_compact_mode_error(
                    buffer,
                    current_line,
                    text_fg_code,
                    current_width,
                    required_width,
                    shortfall,
                );
            }
        }
    }

    /// Renders an error message when terminal is too narrow for compact mode.
    ///
    /// # Arguments
    /// * `buffer` - The string buffer to append to
    /// * `current_line` - Current line position
    /// * `text_fg_code` - Text foreground color code
    /// * `current_width` - Current terminal width
    /// * `required_width` - Required terminal width for compact mode
    /// * `shortfall` - Number of characters short
    fn render_compact_mode_error(
        &self,
        buffer: &mut String,
        current_line: &mut usize,
        text_fg_code: u8,
        current_width: usize,
        required_width: usize,
        shortfall: usize,
    ) {
        let error_message = format!(
            "Terminal too narrow for compact mode ({current_width} chars, need {required_width} chars, short {shortfall} chars)"
        );

        buffer.push_str(&format!(
            "\x1b[{};{}H\x1b[38;5;{}m{}\x1b[0m",
            current_line,
            CONTENT_MARGIN + 1,
            text_fg_code,
            error_message
        ));
        *current_line += 1;

        // Add suggestion for minimum terminal width
        buffer.push_str(&format!(
            "\x1b[{};{}H\x1b[38;5;{}mResize terminal to at least {} characters wide\x1b[0m",
            current_line,
            CONTENT_MARGIN + 1,
            text_fg_code,
            required_width
        ));
        *current_line += 1;
    }
}
