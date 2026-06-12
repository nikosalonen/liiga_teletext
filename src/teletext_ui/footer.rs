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
    pub auto_refresh_indicator: &'a Option<LoadingIndicator>,
    pub auto_refresh_disabled: bool,
    pub error_warning_active: bool,
    pub season_countdown: &'a Option<String>,
    pub view_mode: Option<&'a crate::ui::interactive::state_manager::ViewMode>,
    pub show_today_shortcut: bool,
    pub has_bracket_data: bool,
}

/// A footer segment: plain white text (no background) or a Fastext-style
/// colored block with the given background and foreground ANSI 256 colors.
struct FooterSegment {
    text: &'static str,
    block: Option<(u8, u8)>, // (bg, fg)
}

impl FooterSegment {
    fn plain(text: &'static str) -> Self {
        Self { text, block: None }
    }

    fn block(text: &'static str, bg: u8, fg: u8) -> Self {
        Self {
            text,
            block: Some((bg, fg)),
        }
    }

    /// Visible width in terminal cells (blocks have one space of padding on each side)
    fn visible_width(&self) -> usize {
        let pad = if self.block.is_some() { 2 } else { 0 };
        self.text.chars().count() + pad
    }
}

// Fastext block colors (authentic teletext red/green/yellow/blue shortcut row)
const FASTEXT_RED: u8 = 196;
const FASTEXT_GREEN: u8 = 46;
const FASTEXT_YELLOW: u8 = 226;
const FASTEXT_BLUE: u8 = 21;
const BLOCK_TEXT_DARK: u8 = 16; // black text for light backgrounds
const BLOCK_TEXT_LIGHT: u8 = 231; // white text for dark backgrounds

/// Builds the view-specific footer segments in Fastext order (red, green, yellow, blue)
fn build_footer_segments(ctx: &FooterContext<'_>) -> Vec<FooterSegment> {
    use crate::ui::interactive::state_manager::ViewMode;

    let mut segments = vec![FooterSegment::plain("q=Lopeta")];

    match ctx.view_mode {
        Some(ViewMode::Standings { live_mode }) => {
            segments.push(FooterSegment::block(
                "s=Ottelut",
                FASTEXT_RED,
                BLOCK_TEXT_LIGHT,
            ));
            segments.push(FooterSegment::block(
                if *live_mode { "l=Live ✓" } else { "l=Live" },
                FASTEXT_GREEN,
                BLOCK_TEXT_DARK,
            ));
        }
        Some(ViewMode::Bracket) => {
            segments.push(FooterSegment::block(
                "p=Ottelut",
                FASTEXT_RED,
                BLOCK_TEXT_LIGHT,
            ));
            segments.push(FooterSegment::block(
                "s=Taulukko",
                FASTEXT_YELLOW,
                BLOCK_TEXT_DARK,
            ));
        }
        Some(ViewMode::Games) | None => {
            segments.push(FooterSegment::block(
                "⇧←Edellinen",
                FASTEXT_RED,
                BLOCK_TEXT_LIGHT,
            ));
            segments.push(FooterSegment::block(
                "⇧→Seuraava",
                FASTEXT_GREEN,
                BLOCK_TEXT_DARK,
            ));
            segments.push(FooterSegment::block(
                "s=Taulukko",
                FASTEXT_YELLOW,
                BLOCK_TEXT_DARK,
            ));
            if ctx.has_bracket_data {
                segments.push(FooterSegment::block(
                    "p=Pudotuspelit",
                    FASTEXT_BLUE,
                    BLOCK_TEXT_LIGHT,
                ));
            }
            if ctx.show_today_shortcut {
                segments.push(FooterSegment::plain("t=Tänään"));
            }
        }
    }

    if ctx.auto_refresh_disabled {
        segments.push(FooterSegment::plain("(Ei päivity)"));
    }

    segments
}

/// Renders footer with view-mode-aware Fastext-style controls
pub fn render_footer_with_view(
    _stdout: &mut Stdout,
    buffer: &mut String,
    ctx: &FooterContext<'_>,
) -> Result<(), AppError> {
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

    let mut segments = build_footer_segments(ctx);

    // Right-side activity indicator reserves 3 cells
    let indicator_width = 3;
    let available = ctx.width.saturating_sub(indicator_width + 1);

    // Drop optional plain hints if the segments don't fit the terminal width
    let total_width = |segs: &[FooterSegment]| -> usize {
        segs.iter().map(|s| s.visible_width()).sum::<usize>() + segs.len().saturating_sub(1)
    };
    while total_width(&segments) > available && segments.len() > 1 {
        // Drop optional plain hints first (oldest first, keeping "q=Lopeta"
        // and the status hint as long as possible); fall back to the last block
        let drop_idx = segments
            .iter()
            .position(|s| s.block.is_none() && s.text != "q=Lopeta")
            .unwrap_or(segments.len() - 1);
        segments.remove(drop_idx);
    }

    // Build the footer line: centered segments with colored blocks
    let visible = total_width(&segments);
    let left_pad = available.saturating_sub(visible) / 2;

    let mut line = String::with_capacity(ctx.width + segments.len() * 16);
    line.push_str(&" ".repeat(left_pad));
    for (i, segment) in segments.iter().enumerate() {
        if i > 0 {
            line.push(' ');
        }
        match segment.block {
            Some((bg, fg)) => {
                line.push_str(&format!(
                    "\x1b[48;5;{bg}m\x1b[38;5;{fg}m {} \x1b[0m",
                    segment.text
                ));
            }
            None => {
                line.push_str(&format!("\x1b[38;5;231m{}\x1b[0m", segment.text));
            }
        }
    }

    // Determine right indicator content and color
    let (right_padding, right_color_code) = if let Some(indicator) = ctx.auto_refresh_indicator {
        let frame = indicator.current_frame();
        (
            format!(" {frame} "),
            get_ansi_code(Color::AnsiValue(231), 231),
        ) // white
    } else if ctx.error_warning_active {
        (" ! ".to_string(), get_ansi_code(Color::AnsiValue(226), 226)) // yellow
    } else {
        ("   ".to_string(), get_ansi_code(Color::AnsiValue(16), 16)) // invisible
    };

    // Pad the gap between segments and the right indicator
    let gap = ctx
        .width
        .saturating_sub(left_pad + visible + indicator_width);

    // Convert 0-based footer_y to 1-based for ANSI cursor positioning
    let footer_code = format!(
        "\x1b[{};1H{}{}\x1b[38;5;{}m{}\x1b[0m",
        ctx.footer_y + 1,
        line,
        " ".repeat(gap),
        right_color_code,
        right_padding,
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
    fn test_footer_bracket_view_keys() {
        let mut buffer = String::new();
        let mut stdout = std::io::stdout();
        let ctx = FooterContext {
            footer_y: 23,
            width: 80,
            auto_refresh_indicator: &None,
            auto_refresh_disabled: false,
            error_warning_active: false,
            season_countdown: &None,
            view_mode: Some(&crate::ui::interactive::state_manager::ViewMode::Bracket),
            show_today_shortcut: false,
            has_bracket_data: true,
        };
        render_footer_with_view(&mut stdout, &mut buffer, &ctx).unwrap();
        // 's' switches to standings from bracket view and is advertised as a Fastext block
        assert!(buffer.contains("s=Taulukko"));
        // Bracket view exits via 'p' (back to games)
        assert!(buffer.contains("p=Ottelut"));
        assert!(!buffer.contains("p=Pudotuspelit"));
    }

    #[test]
    fn test_footer_fastext_blocks_use_colored_backgrounds() {
        let mut buffer = String::new();
        let mut stdout = std::io::stdout();
        let ctx = FooterContext {
            footer_y: 23,
            width: 80,
            auto_refresh_indicator: &None,
            auto_refresh_disabled: false,
            error_warning_active: false,
            season_countdown: &None,
            view_mode: Some(&crate::ui::interactive::state_manager::ViewMode::Games),
            show_today_shortcut: false,
            has_bracket_data: true,
        };
        render_footer_with_view(&mut stdout, &mut buffer, &ctx).unwrap();
        // All four Fastext background colors should appear (red, green, yellow, blue)
        assert!(buffer.contains("\x1b[48;5;196m"));
        assert!(buffer.contains("\x1b[48;5;46m"));
        assert!(buffer.contains("\x1b[48;5;226m"));
        assert!(buffer.contains("\x1b[48;5;21m"));
        // Date navigation labels are present
        assert!(buffer.contains("⇧←Edellinen"));
        assert!(buffer.contains("⇧→Seuraava"));
    }

    #[test]
    fn test_footer_drops_optional_hints_when_too_narrow() {
        let mut buffer = String::new();
        let mut stdout = std::io::stdout();
        let ctx = FooterContext {
            footer_y: 23,
            width: 60, // too narrow for all games-view segments
            auto_refresh_indicator: &None,
            auto_refresh_disabled: true,
            error_warning_active: false,
            season_countdown: &None,
            view_mode: Some(&crate::ui::interactive::state_manager::ViewMode::Games),
            show_today_shortcut: true,
            has_bracket_data: true,
        };
        render_footer_with_view(&mut stdout, &mut buffer, &ctx).unwrap();
        // Plain optional hints are dropped before any Fastext block
        assert!(!buffer.contains("(Ei päivity)"));
        assert!(buffer.contains("s=Taulukko"));
    }

    #[test]
    fn test_footer_none_view_mode() {
        let mut buffer = String::new();
        let mut stdout = std::io::stdout();
        let ctx = FooterContext {
            footer_y: 23,
            width: 80,
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
