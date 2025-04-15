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
/// A vector of GoalEventData containing processed goal events for both teams in chronological order
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
/// # Arguments
/// * `team` - Team data implementing HasGoalEvents trait
/// * `player_names` - HashMap mapping player IDs to their full names
/// * `is_home_team` - Boolean indicating if this is the home team
/// * `events` - Mutable vector to append processed goal events to
///
/// # Notes
/// - Filters out "RL0" (removed) and "VT0" (cancelled) goals
/// - Formats player names to show only capitalized last name
/// - Handles missing player names by using a generic "Pelaaja {id}" format
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
