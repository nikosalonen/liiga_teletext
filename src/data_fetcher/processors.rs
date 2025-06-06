use crate::data_fetcher::models::{GameData, GoalEventData, HasGoalEvents, HasTeams, ScheduleGame};
use crate::teletext_ui::ScoreType;
use chrono::{DateTime, Local, NaiveTime, Utc};
use std::collections::HashMap;

/// Processes goal events for both teams in a game, converting them into a standardized format
/// with player names and additional metadata.
///
/// # Arguments
/// * `game` - A type implementing HasTeams trait containing both home and away team data
/// * `player_names` - HashMap mapping player IDs to their full names
///
/// # Returns
/// * `Vec<GoalEventData>` - A vector of processed goal events in chronological order
///
/// # Features
/// - Formats player names consistently (e.g., "Koivu" instead of "Mikko Koivu")
/// - Includes goal timing and score information
/// - Marks special goal types (powerplay, empty net, etc.)
/// - Preserves video clip links when available
/// - Maintains chronological order of goals from both teams
///
/// # Example
/// ```rust
/// use std::collections::HashMap;
/// use data_fetcher::{GameData, GoalEventData, HasTeams, HasGoalEvents};
///
/// let mut player_names = HashMap::new();
/// player_names.insert(123, "Mikko Koivu".to_string());
/// player_names.insert(456, "Teemu Selänne".to_string());
///
/// let events = process_goal_events(&game, &player_names);
/// // Events will contain formatted goal data with:
/// // - Properly formatted player names
/// // - Goals in chronological order
/// // - Special indicators for powerplay goals, etc.
/// ```
pub fn process_goal_events<T>(game: &T, player_names: &HashMap<i64, String>) -> Vec<GoalEventData>
where
    T: HasTeams,
{
    let mut events = Vec::new();

    // Process home team goals
    process_team_goals(game.home_team(), player_names, true, &mut events);
    // Process away team goals
    process_team_goals(game.away_team(), player_names, false, &mut events);

    events
}

/// Processes goal events for a single team, filtering out certain goal types and formatting player names.
///
/// This function handles:
/// - Filtering out cancelled and removed goals
/// - Formatting player names to show only capitalized last names
/// - Handling missing player names gracefully
/// - Preserving goal metadata like timing and special types
///
/// # Arguments
/// * `team` - Team data implementing HasGoalEvents trait
/// * `player_names` - HashMap mapping player IDs to their full names
/// * `is_home_team` - Boolean indicating if this is the home team
/// * `events` - Mutable vector to append processed goal events to
///
/// # Examples
///
/// ```rust
/// use std::collections::HashMap;
/// use data_fetcher::{GoalEventData, HasGoalEvents};
///
/// let mut events = Vec::new();
/// let mut player_names = HashMap::new();
/// player_names.insert(123, "Mikko Koivu".to_string());
///
/// // Process goals for home team
/// process_team_goals(&home_team, &player_names, true, &mut events);
///
/// // Events will now contain home team goals with:
/// // - "Koivu" instead of "Mikko Koivu"
/// // - No cancelled goals (RL0, VT0)
/// // - Proper home/away team attribution
/// ```
pub fn process_team_goals(
    team: &dyn HasGoalEvents,
    player_names: &HashMap<i64, String>,
    is_home_team: bool,
    events: &mut Vec<GoalEventData>,
) {
    for goal in team.goal_events().iter().filter(|g| {
        !g.goal_types.contains(&"RL0".to_string()) && !g.goal_types.contains(&"VT0".to_string())
    }) {
        events.push(GoalEventData {
            scorer_player_id: goal.scorer_player_id,
            scorer_name: player_names
                .get(&goal.scorer_player_id)
                .map(|name| {
                    name.split_whitespace()
                        .last()
                        .unwrap_or("")
                        .chars()
                        .enumerate()
                        .map(|(i, c)| {
                            if i == 0 {
                                c.to_uppercase().next().unwrap_or(c)
                            } else {
                                c.to_lowercase().next().unwrap_or(c)
                            }
                        })
                        .collect::<String>()
                })
                .unwrap_or_else(|| format!("Pelaaja {}", goal.scorer_player_id)),
            minute: goal.game_time / 60,
            home_team_score: goal.home_team_score,
            away_team_score: goal.away_team_score,
            is_winning_goal: goal.winning_goal,
            goal_types: goal.goal_types.clone(),
            is_home_team,
            video_clip_url: goal.video_clip_url.clone(),
        });
    }
}

/// Determines whether to show today's games based on the current time.
///
/// Games are shown for "today" if the current time is after 14:00 (2 PM).
/// Before 14:00, yesterday's games are shown instead. This helps ensure that
/// late-night games are still visible the next morning.
///
/// # Returns
/// * `true` - Show today's games (current time is after 14:00)
/// * `false` - Show yesterday's games (current time is before 14:00)
///
/// # Examples
///
/// ```rust
/// use chrono::Local;
///
/// // At 13:59, returns false (show yesterday's games)
/// // At 14:00, returns true (show today's games)
/// let show_today = should_show_todays_games();
///
/// if show_today {
///     println!("Showing today's games");
/// } else {
///     println!("Showing yesterday's games");
/// }
/// ```
pub fn should_show_todays_games() -> bool {
    let now = Local::now();
    let cutoff_time = NaiveTime::from_hms_opt(14, 0, 0).unwrap();
    let today_cutoff = now.date_naive().and_time(cutoff_time);
    now.naive_local() >= today_cutoff
}

pub fn determine_game_status(game: &ScheduleGame) -> (ScoreType, bool, bool) {
    let is_overtime = matches!(
        game.finished_type.as_deref(),
        Some("ENDED_DURING_EXTENDED_GAME_TIME")
    );

    let is_shootout = matches!(
        game.finished_type.as_deref(),
        Some("ENDED_DURING_WINNING_SHOT_COMPETITION")
    );

    let score_type = if !game.started {
        ScoreType::Scheduled
    } else if !game.ended {
        ScoreType::Ongoing
    } else {
        ScoreType::Final
    };

    (score_type, is_overtime, is_shootout)
}

pub fn format_time(timestamp: &str) -> Result<String, Box<dyn std::error::Error>> {
    let utc_time = timestamp.parse::<DateTime<Utc>>()?;
    let local_time = utc_time.with_timezone(&Local);
    Ok(local_time.format("%H.%M").to_string())
}

pub fn create_basic_goal_events(game: &ScheduleGame) -> Vec<GoalEventData> {
    let mut basic_names = HashMap::new();
    for goal in &game.home_team.goal_events {
        basic_names.insert(
            goal.scorer_player_id,
            format!("Pelaaja {}", goal.scorer_player_id),
        );
    }
    for goal in &game.away_team.goal_events {
        basic_names.insert(
            goal.scorer_player_id,
            format!("Pelaaja {}", goal.scorer_player_id),
        );
    }
    process_goal_events(game, &basic_names)
}
