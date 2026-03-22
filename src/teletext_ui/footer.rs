//! Footer rendering and management for teletext UI
//!
//! This module handles the footer area of the teletext UI, including:
//! - Control key display
//! - Loading indicators
//! - Auto-refresh indicators
//! - Error warnings
//! - Season countdown display

use crate::error::AppError;
use crate::teletext_ui::core::get_ansi_code;
use crate::ui::teletext::colors::*;
use crate::ui::teletext::loading_indicator::LoadingIndicator;
use crossterm::{
    cursor::MoveTo,
    execute,
    style::{Color, Print, ResetColor, SetForegroundColor},
};
use std::io::{Stdout, Write};

/// Context for rendering the footer
pub struct FooterContext<'a> {
    pub footer_y: usize,
    pub width: usize,
    pub total_pages: usize,
    pub auto_refresh_indicator: &'a Option<LoadingIndicator>,
    pub auto_refresh_disabled: bool,
    pub error_warning_active: bool,
    pub season_countdown: &'a Option<String>,
    pub view_mode: Option<&'a crate::ui::interactive::state_manager::ViewMode>,
    pub show_today_shortcut: bool,
    pub has_bracket_data: bool,
}

/// Renders footer with view-mode-aware controls
pub fn render_footer_with_view(
    _stdout: &mut Stdout,
    buffer: &mut String,
    ctx: &FooterContext<'_>,
) -> Result<(), AppError> {
    // Determine navigation controls based on view mode and page count
    let controls = match ctx.view_mode {
        Some(crate::ui::interactive::state_manager::ViewMode::Standings { live_mode }) => {
            if *live_mode {
                if ctx.auto_refresh_disabled {
                    if ctx.total_pages > 1 {
                        "q=Lopeta ←→=Sivut s=Ottelut l=Live ✓ (Ei päivity)"
                    } else {
                        "q=Lopeta s=Ottelut l=Live ✓ (Ei päivity)"
                    }
                } else if ctx.total_pages > 1 {
                    "q=Lopeta ←→=Sivut s=Ottelut l=Live ✓"
                } else {
                    "q=Lopeta s=Ottelut l=Live ✓"
                }
            } else if ctx.auto_refresh_disabled {
                if ctx.total_pages > 1 {
                    "q=Lopeta ←→=Sivut s=Ottelut l=Live (Ei päivity)"
                } else {
                    "q=Lopeta s=Ottelut l=Live (Ei päivity)"
                }
            } else if ctx.total_pages > 1 {
                "q=Lopeta ←→=Sivut s=Ottelut l=Live"
            } else {
                "q=Lopeta s=Ottelut l=Live"
            }
        }
        Some(crate::ui::interactive::state_manager::ViewMode::Bracket) => {
            match (ctx.auto_refresh_disabled, ctx.total_pages > 1) {
                (true, true) => "q=Lopeta ←→=Sivut p=Pudotuspeli (Ei päivity)",
                (true, false) => "q=Lopeta p=Pudotuspeli (Ei päivity)",
                (false, true) => "q=Lopeta ←→=Sivut p=Pudotuspeli",
                (false, false) => "q=Lopeta p=Pudotuspeli",
            }
        }
        Some(crate::ui::interactive::state_manager::ViewMode::Games) | None => {
            match (
                ctx.auto_refresh_disabled,
                ctx.show_today_shortcut,
                ctx.total_pages > 1,
                ctx.has_bracket_data,
            ) {
                (true, true, true, true) => {
                    "q=Lopeta ←→=Sivut s=Taulukko p=Pudotuspeli t=Tänään (Ei päivity)"
                }
                (true, true, true, false) => "q=Lopeta ←→=Sivut s=Taulukko t=Tänään (Ei päivity)",
                (true, true, false, true) => {
                    "q=Lopeta s=Taulukko p=Pudotuspeli t=Tänään (Ei päivity)"
                }
                (true, true, false, false) => "q=Lopeta s=Taulukko t=Tänään (Ei päivity)",
                (true, false, true, true) => {
                    "q=Lopeta ←→=Sivut s=Taulukko p=Pudotuspeli (Ei päivity)"
                }
                (true, false, true, false) => "q=Lopeta ←→=Sivut s=Taulukko (Ei päivity)",
                (true, false, false, true) => "q=Lopeta s=Taulukko p=Pudotuspeli (Ei päivity)",
                (true, false, false, false) => "q=Lopeta s=Taulukko (Ei päivity)",
                (false, true, true, true) => "q=Lopeta ←→=Sivut s=Taulukko p=Pudotuspeli t=Tänään",
                (false, true, true, false) => "q=Lopeta ←→=Sivut s=Taulukko t=Tänään",
                (false, true, false, true) => "q=Lopeta s=Taulukko p=Pudotuspeli t=Tänään",
                (false, true, false, false) => "q=Lopeta s=Taulukko t=Tänään",
                (false, false, true, true) => "q=Lopeta ←→=Sivut s=Taulukko p=Pudotuspeli",
                (false, false, true, false) => "q=Lopeta ←→=Sivut s=Taulukko",
                (false, false, false, true) => "q=Lopeta s=Taulukko p=Pudotuspeli",
                (false, false, false, false) => "q=Lopeta s=Taulukko",
            }
        }
    };

    // Add season countdown above the footer if available
    if let Some(countdown) = ctx.season_countdown {
        let countdown_y = ctx.footer_y.saturating_sub(1);

        // Use optimized ANSI code generation for countdown (requirement 4.3)
        // Convert 0-based countdown_y to 1-based for ANSI cursor positioning
        let countdown_code = format!(
            "\x1b[{};1H\x1b[38;5;{}m{:^width$}\x1b[0m",
            countdown_y + 1,
            get_ansi_code(Color::AnsiValue(226), 226), // Bright yellow
            countdown,
            width = ctx.width
        );
        buffer.push_str(&countdown_code);
    }

    // Footer text is just the controls - indicators moved to right padding
    let footer_text = controls.to_string();

    // Build right padding with activity indicator
    let footer_width = ctx.width.saturating_sub(6);
    let header_bg_code = get_ansi_code(header_bg(), 21);

    // Determine right padding content and color
    let (right_padding, right_color_code) = if let Some(indicator) = ctx.auto_refresh_indicator {
        let frame = indicator.current_frame();
        (
            format!(" {frame} "),
            get_ansi_code(Color::AnsiValue(231), 231),
        ) // white
    } else if ctx.error_warning_active {
        (" ! ".to_string(), get_ansi_code(Color::AnsiValue(226), 226)) // yellow
    } else {
        ("   ".to_string(), get_ansi_code(Color::AnsiValue(21), 21)) // invisible
    };

    // Convert 0-based footer_y to 1-based for ANSI cursor positioning
    let footer_code = format!(
        "\x1b[{};1H\x1b[48;5;{}m\x1b[38;5;21m{}\x1b[38;5;231m{:^width$}\x1b[38;5;{}m{}\x1b[0m",
        ctx.footer_y + 1,
        header_bg_code,
        "   ",
        footer_text,
        right_color_code,
        right_padding,
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

    #[test]
    fn test_footer_games_view_with_bracket_data() {
        let mut buffer = String::new();
        let mut stdout = std::io::stdout();
        let ctx = FooterContext {
            footer_y: 23,
            width: 80,
            total_pages: 2,
            auto_refresh_indicator: &None,
            auto_refresh_disabled: false,
            error_warning_active: false,
            season_countdown: &None,
            view_mode: Some(&crate::ui::interactive::state_manager::ViewMode::Games),
            show_today_shortcut: false,
            has_bracket_data: true,
        };
        render_footer_with_view(&mut stdout, &mut buffer, &ctx).unwrap();
        assert!(buffer.contains("p=Pudotuspeli"));
        assert!(buffer.contains("s=Taulukko"));
    }

    #[test]
    fn test_footer_games_view_without_bracket_data() {
        let mut buffer = String::new();
        let mut stdout = std::io::stdout();
        let ctx = FooterContext {
            footer_y: 23,
            width: 80,
            total_pages: 2,
            auto_refresh_indicator: &None,
            auto_refresh_disabled: false,
            error_warning_active: false,
            season_countdown: &None,
            view_mode: Some(&crate::ui::interactive::state_manager::ViewMode::Games),
            show_today_shortcut: false,
            has_bracket_data: false,
        };
        render_footer_with_view(&mut stdout, &mut buffer, &ctx).unwrap();
        assert!(!buffer.contains("p=Pudotuspeli"));
        assert!(buffer.contains("s=Taulukko"));
    }

    #[test]
    fn test_footer_bracket_view_no_standings_key() {
        let mut buffer = String::new();
        let mut stdout = std::io::stdout();
        let ctx = FooterContext {
            footer_y: 23,
            width: 80,
            total_pages: 2,
            auto_refresh_indicator: &None,
            auto_refresh_disabled: false,
            error_warning_active: false,
            season_countdown: &None,
            view_mode: Some(&crate::ui::interactive::state_manager::ViewMode::Bracket),
            show_today_shortcut: false,
            has_bracket_data: true,
        };
        render_footer_with_view(&mut stdout, &mut buffer, &ctx).unwrap();
        assert!(!buffer.contains("s=Taulukko"));
        assert!(buffer.contains("p=Pudotuspeli"));
    }

    #[test]
    fn test_footer_none_view_mode() {
        let mut buffer = String::new();
        let mut stdout = std::io::stdout();
        let ctx = FooterContext {
            footer_y: 23,
            width: 80,
            total_pages: 1,
            auto_refresh_indicator: &None,
            auto_refresh_disabled: false,
            error_warning_active: false,
            season_countdown: &None,
            view_mode: None,
            show_today_shortcut: false,
            has_bracket_data: false,
        };
        render_footer_with_view(&mut stdout, &mut buffer, &ctx).unwrap();
        assert!(buffer.contains("s=Taulukko"));
    }
}
