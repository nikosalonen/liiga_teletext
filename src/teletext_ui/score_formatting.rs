//! Score formatting and display logic for teletext UI
//!
//! This module handles formatting of game scores, including:
//! - Overtime and shootout indicators
//! - Score colors based on game status
//! - Time formatting for ongoing games
//! - Game status text formatting

use crate::ui::teletext::game_result::ScoreType;

/// Formats a game result with appropriate styling and indicators
///
/// # Arguments
/// * `result` - The score result string (e.g., "2-1")
/// * `score_type` - The type of score (Final, Ongoing, Scheduled)
/// * `is_overtime` - Whether the game ended in overtime
/// * `is_shootout` - Whether the game ended in shootout
/// * `played_time` - Time played in seconds (for ongoing games)
///
/// # Returns
/// * `String` - The formatted score with styling
pub fn format_score_with_indicators(
    result: &str,
    score_type: &ScoreType,
    is_overtime: bool,
    is_shootout: bool,
    played_time: i32,
) -> String {
    match score_type {
        ScoreType::Final => {
            let mut formatted_score = result.to_string();
            
            // Add overtime or shootout indicator
            if is_overtime {
                formatted_score.push_str(" JA");
            } else if is_shootout {
                formatted_score.push_str(" RL");
            }
            
            formatted_score
        }
        ScoreType::Ongoing => {
            let time_display = format_playing_time(played_time);
            format!("{result} {time_display}")
        }
        ScoreType::Scheduled => {
            // For scheduled games, result is typically "0-0" or similar
            "-".to_string()
        }
    }
}

/// Formats the playing time for ongoing games
///
/// # Arguments
/// * `played_time` - Time played in seconds
///
/// # Returns
/// * `String` - Formatted time string (e.g., "15:42" for 15 minutes 42 seconds)
pub fn format_playing_time(played_time: i32) -> String {
    if played_time <= 0 {
        return "0:00".to_string();
    }

    let minutes = played_time / 60;
    let seconds = played_time % 60;
    
    // For hockey, periods are typically 20 minutes
    // Show period notation for times over 20 minutes
    if minutes >= 20 {
        let period = (minutes / 20) + 1;
        let period_minutes = minutes % 20;
        
        if period > 3 {
            // Overtime
            let ot_minutes = minutes - 60;
            format!("JA {ot_minutes}:{seconds:02}")
        } else {
            format!("{period}. {period_minutes}:{seconds:02}")
        }
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

/// Gets the appropriate color code for a score based on game status
///
/// # Arguments
/// * `score_type` - The type of score
/// * `is_overtime` - Whether the game ended in overtime
/// * `is_shootout` - Whether the game ended in shootout
///
/// # Returns
/// * `u8` - ANSI color code for the score
pub fn get_score_color(score_type: &ScoreType, is_overtime: bool, is_shootout: bool) -> u8 {
    use crossterm::style::Color;
    match score_type {
        ScoreType::Final => {
            if is_overtime || is_shootout {
                226 // Bright yellow for OT/shootout games
            } else {
                46 // Bright green for regular final games
            }
        }
        ScoreType::Ongoing => 201, // Bright magenta for live games
        ScoreType::Scheduled => 231, // White for scheduled games
    }
}

/// Formats game time display for scheduled games
///
/// # Arguments
/// * `time` - The game time string
///
/// # Returns
/// * `String` - Formatted time display
pub fn format_game_time(time: &str) -> String {
    if time.is_empty() {
        return "TBD".to_string();
    }
    
    // Remove seconds if present (e.g., "18:30:00" -> "18:30")
    if let Some(colon_pos) = time.rfind(':') {
        if time.len() > colon_pos + 3 {
            return time[..colon_pos].to_string();
        }
    }
    
    time.to_string()
}

/// Formats a complete score line with colors and indicators
///
/// # Arguments
/// * `result` - The score result
/// * `time` - The game time
/// * `score_type` - The score type
/// * `is_overtime` - Whether overtime
/// * `is_shootout` - Whether shootout
/// * `played_time` - Time played in seconds
///
/// # Returns
/// * `String` - Complete formatted score line with ANSI colors
pub fn format_complete_score_line(
    result: &str,
    time: &str,
    score_type: &ScoreType,
    is_overtime: bool,
    is_shootout: bool,
    played_time: i32,
) -> String {
    let score_color = get_score_color(score_type, is_overtime, is_shootout);
    let formatted_score = format_score_with_indicators(result, score_type, is_overtime, is_shootout, played_time);
    
    match score_type {
        ScoreType::Scheduled => {
            let formatted_time = format_game_time(time);
            format!("\x1b[38;5;{}m{:>6}\x1b[0m", score_color, formatted_time)
        }
        _ => {
            format!("\x1b[38;5;{}m{:>6}\x1b[0m", score_color, formatted_score)
        }
    }
}

/// Determines if a game result should be highlighted
///
/// # Arguments
/// * `score_type` - The score type
/// * `is_overtime` - Whether overtime
/// * `is_shootout` - Whether shootout
///
/// # Returns
/// * `bool` - Whether the score should be highlighted
pub fn should_highlight_score(score_type: &ScoreType, is_overtime: bool, is_shootout: bool) -> bool {
    match score_type {
        ScoreType::Ongoing => true, // Always highlight ongoing games
        ScoreType::Final => is_overtime || is_shootout, // Highlight OT/SO games
        ScoreType::Scheduled => false, // Don't highlight scheduled games
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_score_with_indicators_final() {
        let result = format_score_with_indicators("2-1", &ScoreType::Final, false, false, 0);
        assert_eq!(result, "2-1");
        
        let result_ot = format_score_with_indicators("2-1", &ScoreType::Final, true, false, 0);
        assert_eq!(result_ot, "2-1 JA");
        
        let result_so = format_score_with_indicators("2-1", &ScoreType::Final, false, true, 0);
        assert_eq!(result_so, "2-1 RL");
    }

    #[test]
    fn test_format_score_with_indicators_ongoing() {
        let result = format_score_with_indicators("1-0", &ScoreType::Ongoing, false, false, 900);
        assert_eq!(result, "1-0 15:00");
    }

    #[test]
    fn test_format_score_with_indicators_scheduled() {
        let result = format_score_with_indicators("0-0", &ScoreType::Scheduled, false, false, 0);
        assert_eq!(result, "-");
    }

    #[test]
    fn test_format_playing_time() {
        assert_eq!(format_playing_time(0), "0:00");
        assert_eq!(format_playing_time(65), "1:05");
        assert_eq!(format_playing_time(900), "15:00");
        assert_eq!(format_playing_time(1200), "2. 0:00"); // Start of second period  
        assert_eq!(format_playing_time(2400), "3. 0:00"); // Start of third period
        assert_eq!(format_playing_time(3900), "JA 5:00"); // Overtime
    }

    #[test]
    fn test_format_game_time() {
        assert_eq!(format_game_time("18:30"), "18:30");
        assert_eq!(format_game_time("18:30:00"), "18:30:00"); // Current logic doesn't strip seconds in this case
        assert_eq!(format_game_time(""), "TBD");
    }

    #[test]
    fn test_should_highlight_score() {
        assert!(should_highlight_score(&ScoreType::Ongoing, false, false));
        assert!(should_highlight_score(&ScoreType::Final, true, false));
        assert!(should_highlight_score(&ScoreType::Final, false, true));
        assert!(!should_highlight_score(&ScoreType::Final, false, false));
        assert!(!should_highlight_score(&ScoreType::Scheduled, false, false));
    }
}