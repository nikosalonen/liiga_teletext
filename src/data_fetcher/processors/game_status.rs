use crate::data_fetcher::models::ScheduleGame;
use crate::error::AppError;
use crate::teletext_ui::ScoreType;
use chrono::{DateTime, Local, Utc};

/// Determines the status of a game (scheduled, ongoing, or final) along with overtime/shootout flags.
///
/// This function uses enhanced live game detection that checks multiple indicators:
/// - Official started flag from the API
/// - Game clock time combined with timing checks
/// - Recent events indicating live play
///
/// # Arguments
///
/// * `game` - Reference to a ScheduleGame containing game state information
///
/// # Returns
///
/// A tuple of (ScoreType, is_overtime, is_shootout) where:
/// - ScoreType indicates if the game is Scheduled, Ongoing, or Final
/// - is_overtime indicates if the game ended in overtime
/// - is_shootout indicates if the game ended in a shootout
///
/// # Examples
///
/// ```rust
/// use liiga_teletext::data_fetcher::models::ScheduleGame;
/// use liiga_teletext::data_fetcher::processors::determine_game_status;
///
/// let game = ScheduleGame {
///     id: 1,
///     season: 2024,
///     start: "2024-01-15T18:30:00Z".to_string(),
///     end: None,
///     home_team: Default::default(),
///     away_team: Default::default(),
///     finished_type: None,
///     started: true,
///     ended: false,
///     game_time: 1200,
///     serie: "RUNKOSARJA".to_string(),
/// };
///
/// let (score_type, is_overtime, is_shootout) = determine_game_status(&game);
/// ```
pub fn determine_game_status(game: &ScheduleGame) -> (ScoreType, bool, bool) {
    let is_overtime = matches!(
        game.finished_type.as_deref(),
        Some("ENDED_DURING_EXTENDED_GAME_TIME")
    );

    let is_shootout = matches!(
        game.finished_type.as_deref(),
        Some("ENDED_DURING_WINNING_SHOT_COMPETITION")
    );

    // Enhanced live game detection: Check multiple indicators, not just started field
    let is_actually_live = game.started ||  // Official started flag
        (game.game_time > 0 && is_game_likely_live(game)) || // Game clock + timing check
        has_recent_events(game); // Recent events indicate live play

    let score_type = if !is_actually_live {
        ScoreType::Scheduled
    } else if !game.ended {
        ScoreType::Ongoing
    } else {
        ScoreType::Final
    };

    // Enhanced logging for better debugging of game state transitions
    tracing::debug!(
        "Game {} status: started={}, ended={}, score_type={:?}, game_time={}, home_goals={}, away_goals={}, is_actually_live={}",
        game.id,
        game.started,
        game.ended,
        score_type,
        game.game_time,
        game.home_team.goals,
        game.away_team.goals,
        is_actually_live
    );

    // Log when enhanced detection detects live game that started=false doesn't
    if is_actually_live && !game.started {
        tracing::info!(
            "Enhanced detection: Game {} ({} vs {}) detected as live despite started=false - game_time: {}s, recent_events: {}",
            game.id,
            game.home_team.team_name.as_deref().unwrap_or("Unknown"),
            game.away_team.team_name.as_deref().unwrap_or("Unknown"),
            game.game_time,
            has_recent_events(game)
        );
    }

    // Log additional details for ongoing games
    if score_type == ScoreType::Ongoing {
        tracing::info!(
            "Ongoing game detected: {} vs {} (ID: {}) - game_time: {}s, score: {}-{}",
            game.home_team.team_name.as_deref().unwrap_or("Unknown"),
            game.away_team.team_name.as_deref().unwrap_or("Unknown"),
            game.id,
            game.game_time,
            game.home_team.goals,
            game.away_team.goals
        );
    }

    (score_type, is_overtime, is_shootout)
}

/// Formats a UTC timestamp into local time in HH.MM format.
///
/// # Arguments
///
/// * `timestamp` - A string containing an RFC3339 formatted UTC timestamp
///
/// # Returns
///
/// * `Result<String, AppError>` - Formatted time string (e.g., "18.30") or an error
///
/// # Examples
///
/// ```rust
/// use liiga_teletext::data_fetcher::processors::format_time;
///
/// let timestamp = "2024-01-15T18:30:00Z";
/// let formatted = format_time(timestamp).unwrap();
/// // Returns something like "20.30" depending on local timezone
/// ```
pub fn format_time(timestamp: &str) -> Result<String, AppError> {
    let utc_time = timestamp.parse::<DateTime<Utc>>().map_err(|e| {
        AppError::datetime_parse_error(format!("Failed to parse timestamp '{timestamp}': {e}"))
    })?;
    let local_time = utc_time.with_timezone(&Local);
    Ok(local_time.format("%H.%M").to_string())
}

/// Checks if a game has recent events indicating it's actually live
/// even if the started field hasn't been updated yet.
///
/// This function looks for goal events that occurred within the last 5 minutes,
/// which is a strong indicator that the game is actively being played.
///
/// # Arguments
///
/// * `game` - Reference to a ScheduleGame to check for recent events
///
/// # Returns
///
/// * `bool` - true if recent events (within 5 minutes) are detected
fn has_recent_events(game: &ScheduleGame) -> bool {
    let now = Utc::now();
    let recent_threshold = chrono::Duration::minutes(5); // Events within 5 minutes

    // Check for recent goal events from both teams
    let has_recent_goals = [&game.home_team.goal_events, &game.away_team.goal_events]
        .iter()
        .flat_map(|events| events.iter())
        .any(|event| {
            if let Ok(event_time) = chrono::DateTime::parse_from_rfc3339(&event.log_time) {
                let time_diff = now.signed_duration_since(event_time.with_timezone(&Utc));
                time_diff >= chrono::Duration::zero() && time_diff <= recent_threshold
            } else {
                false
            }
        });

    if has_recent_goals {
        tracing::debug!("Recent goal events detected in game {0}", game.id);
        return true;
    }

    false
}

/// Determines if a game with game_time > 0 is likely actually live.
///
/// This function applies heuristics to determine if a game with a non-zero game clock
/// is actually being played right now, as opposed to old/stale data.
///
/// # Heuristics
///
/// A game is considered likely live if:
/// 1. Game was supposed to start within the last 3 hours (not old data)
/// 2. Current time is not too far before the scheduled start (-15 min buffer)
/// 3. We're not dealing with very old test data (more than 6 months old)
///
/// # Arguments
///
/// * `game` - Reference to a ScheduleGame to evaluate
///
/// # Returns
///
/// * `bool` - true if the game is likely actually live
fn is_game_likely_live(game: &ScheduleGame) -> bool {
    let now = Utc::now();

    if let Ok(game_start) = chrono::DateTime::parse_from_rfc3339(&game.start) {
        let time_since_start = now.signed_duration_since(game_start.with_timezone(&Utc));

        // Only consider it live if:
        // 1. Game was supposed to start within the last 3 hours (not old data)
        // 2. Current time is not too far before the scheduled start (-15 min buffer)
        // 3. We're not dealing with very old test data (more than 6 months old)
        let is_recent_game = time_since_start <= chrono::Duration::hours(3)
            && time_since_start >= chrono::Duration::minutes(-15);
        let is_not_ancient = time_since_start <= chrono::Duration::days(180); // 6 months

        if is_recent_game && is_not_ancient {
            tracing::debug!(
                "Game {} likely live: game_time={}, time_since_start={:?}",
                game.id,
                game.game_time,
                time_since_start
            );
            return true;
        }
    }

    false
}
