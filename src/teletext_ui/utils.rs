use crossterm::style::Color;

/// Helper function to extract ANSI color code from crossterm Color enum.
/// Provides a fallback value for non-ANSI colors.
pub fn get_ansi_code(color: Color, fallback: u8) -> u8 {
    match color {
        Color::AnsiValue(val) => val,
        _ => fallback,
    }
}