use crossterm::style::Color;

// Constants for teletext appearance
pub fn header_bg() -> Color {
    Color::AnsiValue(21)
} // Bright blue
pub fn header_fg() -> Color {
    Color::AnsiValue(21)
} // Bright blue
pub fn subheader_fg() -> Color {
    Color::AnsiValue(46)
} // Bright green
pub fn result_fg() -> Color {
    Color::AnsiValue(46)
} // Bright green
pub fn text_fg() -> Color {
    Color::AnsiValue(231)
} // Pure white
pub fn home_scorer_fg() -> Color {
    Color::AnsiValue(51)
} // Bright cyan
pub fn away_scorer_fg() -> Color {
    Color::AnsiValue(51)
} // Bright cyan
pub fn winning_goal_fg() -> Color {
    Color::AnsiValue(201)
} // Bright magenta
pub fn goal_type_fg() -> Color {
    Color::AnsiValue(226)
} // Bright yellow
pub fn title_bg() -> Color {
    Color::AnsiValue(46)
} // Bright green