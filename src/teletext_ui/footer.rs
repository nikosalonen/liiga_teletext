//! Footer rendering and management for teletext UI
//!
//! This module handles the footer area of the teletext UI, including:
//! - Control key display
//! - Loading indicators
//! - Auto-refresh indicators
//! - Error warnings
//! - Season countdown display

use crate::error::AppError;
use crate::teletext_ui::utils::get_ansi_code;
use crate::ui::teletext::colors::*;
use crate::ui::teletext::loading_indicator::LoadingIndicator;
use crossterm::{
    cursor::MoveTo,
    execute,
    style::{Color, Print, ResetColor, SetForegroundColor},
};
use std::io::{Stdout, Write};

/// Renders footer with navigation controls, loading indicator, and error warning
///
/// # Arguments
/// * `stdout` - The stdout to write to
/// * `buffer` - The buffer to write to
/// * `footer_y` - The vertical position of the footer
/// * `width` - The width of the terminal
/// * `total_pages` - The total number of pages
/// * `loading_indicator` - Optional loading indicator
/// * `auto_refresh_indicator` - Optional auto-refresh indicator
/// * `auto_refresh_disabled` - Whether auto-refresh is disabled
/// * `error_warning_active` - Whether to show an error warning
/// * `season_countdown` - Optional season countdown text
///
/// # Returns
/// * `Result<(), AppError>` - Result indicating success or failure
#[allow(clippy::too_many_arguments)]
pub fn render_footer(
    _stdout: &mut Stdout,
    buffer: &mut String,
    footer_y: usize,
    width: usize,
    total_pages: usize,
    loading_indicator: &Option<LoadingIndicator>,
    auto_refresh_indicator: &Option<LoadingIndicator>,
    auto_refresh_disabled: bool,
    error_warning_active: bool,
    season_countdown: &Option<String>,
) -> Result<(), AppError> {
    // Determine navigation controls based on page count
    let controls = if total_pages > 1 {
        "q=Lopeta ←→=Sivut"
    } else {
        "q=Lopeta"
    };

    // Add auto-refresh disabled note if needed
    let controls = if auto_refresh_disabled {
        if total_pages > 1 {
            "q=Lopeta ←→=Sivut (Ei päivity)"
        } else {
            "q=Lopeta (Ei päivity)"
        }
    } else {
        controls
    };

    // Add season countdown above the footer if available
    if let Some(countdown) = season_countdown {
        let countdown_y = footer_y.saturating_sub(1);

        // Use optimized ANSI code generation for countdown (requirement 4.3)
        // Convert 0-based countdown_y to 1-based for ANSI cursor positioning
        let countdown_code = format!(
            "\x1b[{};1H\x1b[38;5;{}m{:^width$}\x1b[0m",
            countdown_y + 1,
            get_ansi_code(Color::AnsiValue(226), 226), // Bright yellow
            countdown,
            width = width
        );
        buffer.push_str(&countdown_code);
    }

    // Add loading indicator or auto-refresh indicator if active
    let mut footer_text = if let Some(loading) = loading_indicator {
        let loading_frame = loading.current_frame();
        format!("{controls} {} {}", loading_frame, loading.message())
    } else if let Some(indicator) = auto_refresh_indicator {
        let indicator_frame = indicator.current_frame();
        format!("{controls} {indicator_frame}")
    } else {
        controls.to_string()
    };

    // Append error warning if active
    if error_warning_active {
        footer_text.push_str("  ⚠️");
    }

    // Batch footer ANSI code generation for better performance (requirement 4.3)
    let footer_width = width.saturating_sub(6);
    let header_bg_code = get_ansi_code(header_bg(), 21);

    // Convert 0-based footer_y to 1-based for ANSI cursor positioning
    let footer_code = format!(
        "\x1b[{};1H\x1b[48;5;{}m\x1b[38;5;21m{}\x1b[38;5;231m{:^width$}\x1b[38;5;21m{}\x1b[0m",
        footer_y + 1,
        header_bg_code,
        "   ",
        footer_text,
        "   ",
        width = footer_width
    );
    buffer.push_str(&footer_code);

    Ok(())
}

/// Renders only the loading indicator area without redrawing the entire screen
///
/// This is used for updating loading animations without redrawing the whole page.
///
/// # Arguments
/// * `stdout` - The stdout to write to
/// * `screen_height` - The height of the terminal
/// * `ignore_height_limit` - Whether to ignore terminal height limits
/// * `loading_indicator` - Optional loading indicator
///
/// # Returns
/// * `Result<(), AppError>` - Result indicating success or failure
pub fn render_loading_indicator_only(
    stdout: &mut Stdout,
    screen_height: u16,
    ignore_height_limit: bool,
    loading_indicator: &Option<LoadingIndicator>,
) -> Result<(), AppError> {
    if ignore_height_limit {
        // In --once mode, we don't update loading indicators
        return Ok(());
    }

    let (width, _) = crossterm::terminal::size()?;
    let footer_y = screen_height.saturating_sub(1);
    let empty_y = footer_y.saturating_sub(1);

    // Clear the loading indicator line first
    execute!(
        stdout,
        MoveTo(0, empty_y),
        Print(" ".repeat(width as usize))
    )?;

    // Show loading indicator if active
    if let Some(loading) = loading_indicator {
        let loading_text = format!("{} {}", loading.current_frame(), loading.message());
        let loading_width = loading_text.chars().count();
        let left_padding = if width as usize > loading_width {
            (width as usize - loading_width) / 2
        } else {
            0
        };
        execute!(
            stdout,
            MoveTo(0, empty_y),
            SetForegroundColor(goal_type_fg()), // Use existing color function for consistency
            Print(format!(
                "{space:>pad$}{text}",
                space = "",
                pad = left_padding,
                text = loading_text
            )),
            ResetColor
        )?;
    }

    stdout.flush()?;
    Ok(())
}

/// Calculates the footer position based on settings and screen size
///
/// # Arguments
/// * `ignore_height_limit` - Whether to ignore terminal height limits
/// * `current_line` - The current line position
/// * `screen_height` - The height of the terminal
///
/// # Returns
/// * `usize` - The y-coordinate of the footer
pub fn calculate_footer_position(
    ignore_height_limit: bool,
    current_line: usize,
    screen_height: u16,
) -> usize {
    if ignore_height_limit {
        // In non-interactive mode, position footer after content
        current_line + 1
    } else {
        // In interactive mode, position footer at bottom of screen
        screen_height.saturating_sub(1) as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_footer_position_interactive() {
        let screen_height = 24;
        let current_line = 10;
        let position = calculate_footer_position(false, current_line, screen_height);

        // In interactive mode, footer should be at bottom of screen
        assert_eq!(position, 23);
    }

    #[test]
    fn test_calculate_footer_position_non_interactive() {
        let screen_height = 24;
        let current_line = 10;
        let position = calculate_footer_position(true, current_line, screen_height);

        // In non-interactive mode, footer should be below content
        assert_eq!(position, 11);
    }
}
