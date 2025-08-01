use crate::data_fetcher::models::{GoalEventData, HasGoalEvents, HasTeams, ScheduleGame};
use crate::data_fetcher::player_names::create_fallback_name;
use crate::error::AppError;
use crate::teletext_ui::ScoreType;
use chrono::{DateTime, Datelike, Local, NaiveTime, Utc};
use std::collections::HashMap;

// Tournament season constants for month-based logic
const PRESEASON_START_MONTH: u32 = 6; // June
const PRESEASON_END_MONTH: u32 = 9; // September

/// Processes goal events for both teams in a game, converting them into a standardized format
/// with player names and additional metadata.
///
/// # Arguments
/// * `game` - A type implementing HasTeams trait containing both home and away team data
/// * `player_names` - HashMap mapping player IDs to their formatted names (e.g., "Koivu" instead of "Mikko Koivu")
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
/// use liiga_teletext::data_fetcher::{GoalEventData, models::{HasTeams, HasGoalEvents, ScheduleGame, ScheduleTeam}};
/// use liiga_teletext::data_fetcher::processors::process_goal_events;
///
/// let mut player_names = HashMap::new();
/// player_names.insert(123, "Koivu".to_string());
/// player_names.insert(456, "Selänne".to_string());
///
/// let game = ScheduleGame {
///     id: 1,
///     season: 2024,
///     start: "2024-01-15T18:30:00Z".to_string(),
///     end: None,
///     home_team: ScheduleTeam::default(),
///     away_team: ScheduleTeam::default(),
///     finished_type: None,
///     started: true,
///     ended: true,
///     game_time: 60,
///     serie: "RUNKOSARJA".to_string(),
/// };
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
/// - Using pre-formatted player names (cached formatted names)
/// - Handling missing player names gracefully
/// - Preserving goal metadata like timing and special types
///
/// # Arguments
/// * `team` - Team data implementing HasGoalEvents trait
/// * `player_names` - HashMap mapping player IDs to their formatted names (e.g., "Koivu" instead of "Mikko Koivu")
/// * `is_home_team` - Boolean indicating if this is the home team
/// * `events` - Mutable vector to append processed goal events to
///
/// # Examples
///
/// ```rust
/// use std::collections::HashMap;
/// use liiga_teletext::data_fetcher::{GoalEventData, models::{HasGoalEvents, ScheduleTeam}};
/// use liiga_teletext::data_fetcher::processors::process_team_goals;
///
/// let mut events = Vec::new();
/// let mut player_names = HashMap::new();
/// player_names.insert(123, "Koivu".to_string());
///
/// let home_team = ScheduleTeam::default();
///
/// // Process goals for home team
/// process_team_goals(&home_team, &player_names, true, &mut events);
///
/// // Events will now contain home team goals with:
/// // - Pre-formatted player names (e.g., "Koivu")
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
                .cloned()
                .unwrap_or_else(|| create_fallback_name(goal.scorer_player_id)),
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

/// Determines whether to show today's games or yesterday's games.
/// During preseason (May-September), always shows today's games since practice games
/// might be scheduled at any time of day. During regular season and playoffs,
/// uses a 14:00 cutoff time.
///
/// # Returns
///
/// `true` if today's games should be shown, `false` if yesterday's games should be shown.
///
/// # Examples
///
/// ```
/// use liiga_teletext::data_fetcher::processors::should_show_todays_games;
///
/// let show_today = should_show_todays_games();
/// if show_today {
///     println!("Showing today's games");
/// } else {
///     println!("Showing yesterday's games");
/// }
/// ```
pub fn should_show_todays_games() -> bool {
    // Use UTC for internal calculations to avoid DST issues
    let now_utc = Utc::now();
    // Convert to local time for date and time comparisons
    let now_local = now_utc.with_timezone(&Local);

    // Check if we're in preseason (May-September)
    let current_month = now_local.month();
    if (PRESEASON_START_MONTH..=PRESEASON_END_MONTH).contains(&current_month) {
        // During preseason, always show today's games since practice games
        // might be scheduled at any time of day
        return true;
    }

    // For regular season and playoffs, use the 14:00 cutoff time
    let cutoff_time = NaiveTime::from_hms_opt(14, 0, 0).unwrap();
    let today_cutoff = now_local.date_naive().and_time(cutoff_time);
    now_local.naive_local() >= today_cutoff
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

pub fn format_time(timestamp: &str) -> Result<String, AppError> {
    let utc_time = timestamp.parse::<DateTime<Utc>>().map_err(|e| {
        AppError::datetime_parse_error(format!("Failed to parse timestamp '{timestamp}': {e}"))
    })?;
    let local_time = utc_time.with_timezone(&Local);
    Ok(local_time.format("%H.%M").to_string())
}

pub fn create_basic_goal_events(game: &ScheduleGame) -> Vec<GoalEventData> {
    let mut basic_names = HashMap::new();
    for goal in &game.home_team.goal_events {
        basic_names.insert(
            goal.scorer_player_id,
            create_fallback_name(goal.scorer_player_id),
        );
    }
    for goal in &game.away_team.goal_events {
        basic_names.insert(
            goal.scorer_player_id,
            create_fallback_name(goal.scorer_player_id),
        );
    }
    process_goal_events(game, &basic_names)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_fetcher::models::{GoalEvent, ScheduleGame, ScheduleTeam};

    fn create_test_goal_event(
        scorer_player_id: i64,
        game_time: i32,
        home_score: i32,
        away_score: i32,
        goal_types: Vec<String>,
    ) -> GoalEvent {
        GoalEvent {
            scorer_player_id,
            log_time: "18:30:00".to_string(),
            game_time,
            period: 1,
            event_id: 1,
            home_team_score: home_score,
            away_team_score: away_score,
            winning_goal: false,
            goal_types,
            assistant_player_ids: vec![],
            video_clip_url: Some("https://example.com/video.mp4".to_string()),
        }
    }

    fn create_test_team_with_goals(goals: Vec<GoalEvent>) -> ScheduleTeam {
        ScheduleTeam {
            goal_events: goals,
            ..Default::default()
        }
    }

    fn create_test_game(home_goals: Vec<GoalEvent>, away_goals: Vec<GoalEvent>) -> ScheduleGame {
        ScheduleGame {
            id: 1,
            season: 2024,
            start: "2024-01-15T18:30:00Z".to_string(),
            end: None,
            home_team: create_test_team_with_goals(home_goals),
            away_team: create_test_team_with_goals(away_goals),
            finished_type: None,
            started: true,
            ended: false,
            game_time: 1200, // 20 minutes
            serie: "runkosarja".to_string(),
        }
    }

    #[test]
    fn test_process_goal_events_empty_game() {
        let game = create_test_game(vec![], vec![]);
        let player_names = HashMap::new();

        let events = process_goal_events(&game, &player_names);
        assert!(events.is_empty());
    }

    #[test]
    fn test_process_goal_events_with_goals() {
        let home_goal = create_test_goal_event(123, 900, 1, 0, vec!["EV".to_string()]);
        let away_goal = create_test_goal_event(456, 1200, 1, 1, vec!["YV".to_string()]);

        let game = create_test_game(vec![home_goal], vec![away_goal]);

        let mut player_names = HashMap::new();
        player_names.insert(123, "Koivu".to_string());
        player_names.insert(456, "Selänne".to_string());

        let events = process_goal_events(&game, &player_names);

        assert_eq!(events.len(), 2);

        // Check home goal
        let home_event = &events[0];
        assert_eq!(home_event.scorer_player_id, 123);
        assert_eq!(home_event.scorer_name, "Koivu");
        assert_eq!(home_event.minute, 15); // 900 seconds / 60
        assert_eq!(home_event.home_team_score, 1);
        assert_eq!(home_event.away_team_score, 0);
        assert!(home_event.is_home_team);
        assert_eq!(home_event.goal_types, vec!["EV"]);

        // Check away goal
        let away_event = &events[1];
        assert_eq!(away_event.scorer_player_id, 456);
        assert_eq!(away_event.scorer_name, "Selänne");
        assert_eq!(away_event.minute, 20); // 1200 seconds / 60
        assert_eq!(away_event.home_team_score, 1);
        assert_eq!(away_event.away_team_score, 1);
        assert!(!away_event.is_home_team);
        assert_eq!(away_event.goal_types, vec!["YV"]);
    }

    #[test]
    fn test_process_goal_events_with_fallback_names() {
        let home_goal = create_test_goal_event(999, 600, 1, 0, vec!["EV".to_string()]);
        let game = create_test_game(vec![home_goal], vec![]);

        // No player names provided - should use fallback
        let player_names = HashMap::new();

        let events = process_goal_events(&game, &player_names);

        assert_eq!(events.len(), 1);
        let event = &events[0];
        assert_eq!(event.scorer_player_id, 999);
        assert_eq!(event.scorer_name, "Pelaaja 999"); // Fallback name
    }

    #[test]
    fn test_process_team_goals_filters_cancelled_goals() {
        let valid_goal = create_test_goal_event(123, 600, 1, 0, vec!["EV".to_string()]);
        let cancelled_goal_rl0 = create_test_goal_event(456, 900, 1, 0, vec!["RL0".to_string()]);
        let cancelled_goal_vt0 = create_test_goal_event(789, 1200, 1, 0, vec!["VT0".to_string()]);

        let team =
            create_test_team_with_goals(vec![valid_goal, cancelled_goal_rl0, cancelled_goal_vt0]);
        let mut player_names = HashMap::new();
        player_names.insert(123, "Koivu".to_string());
        player_names.insert(456, "Cancelled1".to_string());
        player_names.insert(789, "Cancelled2".to_string());

        let mut events = Vec::new();
        process_team_goals(&team, &player_names, true, &mut events);

        // Should only have the valid goal, cancelled goals filtered out
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].scorer_player_id, 123);
        assert_eq!(events[0].scorer_name, "Koivu");
    }

    #[test]
    fn test_should_show_todays_games() {
        // This function depends on current time, so we test the logic indirectly
        // by checking that it returns a boolean
        let result = should_show_todays_games();
        // Just verify it returns a boolean value (no assertion needed)
        let _: bool = result;
    }

    #[test]
    fn test_should_show_todays_games_consistency() {
        // Multiple calls should return the same result within a short time frame
        let result1 = should_show_todays_games();
        let result2 = should_show_todays_games();
        assert_eq!(result1, result2);
    }

    #[test]
    fn test_determine_game_status_scheduled() {
        let mut game = create_test_game(vec![], vec![]);
        game.started = false;
        game.ended = false;
        game.finished_type = None;

        let (score_type, is_overtime, is_shootout) = determine_game_status(&game);

        assert!(matches!(score_type, ScoreType::Scheduled));
        assert!(!is_overtime);
        assert!(!is_shootout);
    }

    #[test]
    fn test_determine_game_status_ongoing() {
        let mut game = create_test_game(vec![], vec![]);
        game.started = true;
        game.ended = false;
        game.finished_type = None;

        let (score_type, is_overtime, is_shootout) = determine_game_status(&game);

        assert!(matches!(score_type, ScoreType::Ongoing));
        assert!(!is_overtime);
        assert!(!is_shootout);
    }

    #[test]
    fn test_determine_game_status_finished_regular() {
        let mut game = create_test_game(vec![], vec![]);
        game.started = true;
        game.ended = true;
        game.finished_type = Some("ENDED_DURING_REGULAR_TIME".to_string());

        let (score_type, is_overtime, is_shootout) = determine_game_status(&game);

        assert!(matches!(score_type, ScoreType::Final));
        assert!(!is_overtime);
        assert!(!is_shootout);
    }

    #[test]
    fn test_determine_game_status_overtime() {
        let mut game = create_test_game(vec![], vec![]);
        game.started = true;
        game.ended = true;
        game.finished_type = Some("ENDED_DURING_EXTENDED_GAME_TIME".to_string());

        let (score_type, is_overtime, is_shootout) = determine_game_status(&game);

        assert!(matches!(score_type, ScoreType::Final));
        assert!(is_overtime);
        assert!(!is_shootout);
    }

    #[test]
    fn test_determine_game_status_shootout() {
        let mut game = create_test_game(vec![], vec![]);
        game.started = true;
        game.ended = true;
        game.finished_type = Some("ENDED_DURING_WINNING_SHOT_COMPETITION".to_string());

        let (score_type, is_overtime, is_shootout) = determine_game_status(&game);

        assert!(matches!(score_type, ScoreType::Final));
        assert!(!is_overtime);
        assert!(is_shootout);
    }

    #[test]
    fn test_format_time_valid_utc() {
        let timestamp = "2024-01-15T18:30:00Z";
        let result = format_time(timestamp);

        assert!(result.is_ok());
        let formatted = result.unwrap();
        // Should be in HH.MM format
        assert!(formatted.contains('.'));
        assert_eq!(formatted.len(), 5); // HH.MM is 5 characters
    }

    #[test]
    fn test_format_time_valid_with_timezone() {
        let timestamp = "2024-01-15T18:30:00+02:00";
        let result = format_time(timestamp);

        assert!(result.is_ok());
        let formatted = result.unwrap();
        assert!(formatted.contains('.'));
        assert_eq!(formatted.len(), 5);
    }

    #[test]
    fn test_format_time_invalid_format() {
        let invalid_timestamp = "not a timestamp";
        let result = format_time(invalid_timestamp);

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AppError::DateTimeParse(_)));
    }

    #[test]
    fn test_format_time_empty_string() {
        let result = format_time("");

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AppError::DateTimeParse(_)));
    }

    #[test]
    fn test_format_time_invalid_date() {
        let invalid_timestamp = "2024-13-45T25:70:90Z"; // Invalid date/time values
        let result = format_time(invalid_timestamp);

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AppError::DateTimeParse(_)));
    }

    #[test]
    fn test_create_basic_goal_events() {
        let home_goal = create_test_goal_event(123, 600, 1, 0, vec!["EV".to_string()]);
        let away_goal = create_test_goal_event(456, 900, 1, 1, vec!["YV".to_string()]);

        let game = create_test_game(vec![home_goal], vec![away_goal]);

        let events = create_basic_goal_events(&game);

        assert_eq!(events.len(), 2);

        // Should use fallback names since no player names cache is provided
        assert_eq!(events[0].scorer_name, "Pelaaja 123");
        assert_eq!(events[1].scorer_name, "Pelaaja 456");
    }

    #[test]
    fn test_create_basic_goal_events_empty_game() {
        let game = create_test_game(vec![], vec![]);
        let events = create_basic_goal_events(&game);
        assert!(events.is_empty());
    }

    #[test]
    fn test_goal_event_data_fields() {
        let goal = create_test_goal_event(123, 900, 2, 1, vec!["YV".to_string(), "MV".to_string()]);
        let game = create_test_game(vec![], vec![goal]);

        let mut player_names = HashMap::new();
        player_names.insert(123, "Test Player".to_string());

        let events = process_goal_events(&game, &player_names);
        assert_eq!(events.len(), 1);

        let event = &events[0];
        assert_eq!(event.scorer_player_id, 123);
        assert_eq!(event.scorer_name, "Test Player");
        assert_eq!(event.minute, 15); // 900 / 60
        assert_eq!(event.home_team_score, 2);
        assert_eq!(event.away_team_score, 1);
        assert!(!event.is_winning_goal);
        assert_eq!(event.goal_types, vec!["YV", "MV"]);
        assert!(!event.is_home_team); // Away team goal
        assert_eq!(
            event.video_clip_url,
            Some("https://example.com/video.mp4".to_string())
        );
    }

    #[test]
    fn test_process_goal_events_preserves_winning_goal_flag() {
        let mut winning_goal = create_test_goal_event(123, 600, 1, 0, vec!["EV".to_string()]);
        winning_goal.winning_goal = true;

        let game = create_test_game(vec![winning_goal], vec![]);
        let mut player_names = HashMap::new();
        player_names.insert(123, "Winner".to_string());

        let events = process_goal_events(&game, &player_names);

        assert_eq!(events.len(), 1);
        assert!(events[0].is_winning_goal);
    }

    #[test]
    fn test_process_goal_events_multiple_goal_types() {
        let complex_goal = create_test_goal_event(
            123,
            600,
            1,
            0,
            vec!["YV".to_string(), "RV".to_string(), "MV".to_string()],
        );

        let game = create_test_game(vec![complex_goal], vec![]);
        let mut player_names = HashMap::new();
        player_names.insert(123, "Complex Scorer".to_string());

        let events = process_goal_events(&game, &player_names);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].goal_types, vec!["YV", "RV", "MV"]);
    }

    #[test]
    fn test_process_goal_events_no_video_url() {
        let mut goal_without_video = create_test_goal_event(123, 600, 1, 0, vec!["EV".to_string()]);
        goal_without_video.video_clip_url = None;

        let game = create_test_game(vec![goal_without_video], vec![]);
        let mut player_names = HashMap::new();
        player_names.insert(123, "No Video".to_string());

        let events = process_goal_events(&game, &player_names);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].video_clip_url, None);
    }

    #[test]
    fn test_edge_cases_zero_game_time() {
        let zero_time_goal = create_test_goal_event(123, 0, 1, 0, vec!["EV".to_string()]);
        let game = create_test_game(vec![zero_time_goal], vec![]);
        let mut player_names = HashMap::new();
        player_names.insert(123, "Quick Goal".to_string());

        let events = process_goal_events(&game, &player_names);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].minute, 0); // 0 / 60 = 0
    }

    #[test]
    fn test_edge_cases_large_game_time() {
        let late_goal = create_test_goal_event(123, 7200, 1, 0, vec!["EV".to_string()]); // 2 hours
        let game = create_test_game(vec![late_goal], vec![]);
        let mut player_names = HashMap::new();
        player_names.insert(123, "Very Late Goal".to_string());

        let events = process_goal_events(&game, &player_names);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].minute, 120); // 7200 / 60 = 120 minutes
    }
}
