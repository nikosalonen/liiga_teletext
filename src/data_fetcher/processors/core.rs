use crate::constants::env_vars;
use crate::data_fetcher::models::{GoalEventData, HasGoalEvents, HasTeams, ScheduleGame};
use crate::data_fetcher::player_names::{
    DisambiguationContext, build_full_name, create_fallback_name, format_for_display,
};
use crate::error::AppError;
use crate::teletext_ui::ScoreType;
use chrono::{DateTime, Local, NaiveTime, TimeZone, Utc};
use std::collections::{HashMap, HashSet};
use tracing;

// Import game status functions from game_status module
use super::game_status::{determine_game_status, format_time};

/// Processes goal events for both teams in a game with team-scoped disambiguation.
/// This enhanced version applies disambiguation separately for home and away teams,
/// ensuring that players with the same last name on the same team are distinguished
/// with first initials (e.g., "Koivu M.", "Koivu S.").
///
/// # Arguments
/// * `game` - A type implementing HasTeams trait containing both home and away team data
/// * `home_players` - A slice of tuples containing (player_id, first_name, last_name) for home team
/// * `away_players` - A slice of tuples containing (player_id, first_name, last_name) for away team
///
/// # Returns
/// * `Vec<GoalEventData>` - A vector of processed goal events in chronological order with disambiguated names
///
/// # Features
/// - Applies team-scoped disambiguation (players on different teams don't affect each other)
/// - Formats player names with first initials when needed (e.g., "Koivu M.", "Koivu S.")
/// - Includes goal timing and score information
/// - Marks special goal types (powerplay, empty net, etc.)
/// - Preserves video clip links when available
/// - Maintains chronological order of goals from both teams
///
/// # Example
/// This function is typically used with game data from the API to create
/// disambiguated goal events that avoid confusion between players with similar names.
/// When multiple players share the same last name on a team, their names are
/// differentiated using first initials (e.g., "Koivu M." vs "Koivu S.").
#[allow(dead_code)] // Used in integration tests
pub fn process_goal_events_with_disambiguation<T>(
    game: &T,
    home_players: &[(i64, String, String)], // (id, first_name, last_name)
    away_players: &[(i64, String, String)], // (id, first_name, last_name)
) -> Vec<GoalEventData>
where
    T: HasTeams,
{
    let mut events = Vec::new();

    // Create disambiguation contexts for each team separately
    let home_context = DisambiguationContext::new(home_players.to_vec());
    let away_context = DisambiguationContext::new(away_players.to_vec());

    // Process home team goals with home team disambiguation
    process_team_goals_with_disambiguation(game.home_team(), &home_context, true, &mut events);

    // Process away team goals with away team disambiguation
    process_team_goals_with_disambiguation(game.away_team(), &away_context, false, &mut events);

    events
}

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

/// Processes goal events for a single team with team-scoped disambiguation.
/// This enhanced version uses a disambiguation context to resolve player names
/// with first initials when multiple players on the same team share the same last name.
///
/// # Arguments
/// * `team` - Team data implementing HasGoalEvents trait
/// * `disambiguation_context` - Context containing disambiguated player names for this team
/// * `is_home_team` - Boolean indicating if this is the home team
/// * `events` - Mutable vector to append processed goal events to
///
/// # Features
/// - Filters out cancelled and removed goals (RL0, VT0)
/// - Uses team-scoped disambiguation for player names
/// - Handles missing player names gracefully with fallback
/// - Preserves goal metadata like timing and special types
///
/// # Example
/// ```rust
/// use liiga_teletext::data_fetcher::{GoalEventData, models::{HasGoalEvents, ScheduleTeam}};
/// use liiga_teletext::data_fetcher::processors::process_team_goals_with_disambiguation;
/// use liiga_teletext::data_fetcher::player_names::DisambiguationContext;
///
/// let mut events = Vec::new();
/// let players = vec![
///     (123, "Mikko".to_string(), "Koivu".to_string()),
///     (124, "Saku".to_string(), "Koivu".to_string()),
/// ];
/// let context = DisambiguationContext::new(players);
/// let home_team = ScheduleTeam::default();
///
/// process_team_goals_with_disambiguation(&home_team, &context, true, &mut events);
/// // Events will contain disambiguated names: "Koivu M.", "Koivu S."
/// ```
#[allow(dead_code)] // Used in integration tests
pub fn process_team_goals_with_disambiguation(
    team: &dyn HasGoalEvents,
    disambiguation_context: &DisambiguationContext,
    is_home_team: bool,
    events: &mut Vec<GoalEventData>,
) {
    for goal in team.goal_events().iter().filter(|g| {
        !g.goal_types.contains(&"RL0".to_string()) && !g.goal_types.contains(&"VT0".to_string())
    }) {
        events.push(GoalEventData {
            scorer_player_id: goal.scorer_player_id,
            scorer_name: disambiguation_context
                .get_disambiguated_name(goal.scorer_player_id)
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
/// Uses a consistent 12:00 (noon) local-time cutoff year-round (chrono::Local) for authentic teletext-style behavior.
///
/// The cutoff is evaluated in the system's local timezone; noon is treated as the instant the
/// local clock shows 12:00. This matches user expectations and is stable across DST transitions.
///
/// Before noon: Shows yesterday's games (morning preference)
/// After noon: Shows today's games
///
/// This provides consistent user experience regardless of season, allowing users to see
/// previous day's results in the morning and current day's games in the afternoon.
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

    should_show_todays_games_with_time(now_local)
}

/// Determines whether to show today's games or yesterday's games based on a specific time.
/// This is a deterministic helper function that takes a local time and computes the noon cutoff.
///
/// # Arguments
///
/// * `now_local` - The local time to evaluate against the noon cutoff
///
/// # Returns
///
/// `true` if the given time is after noon (12:00), `false` if before noon
///
/// # Examples
///
/// ```
/// use liiga_teletext::data_fetcher::processors::should_show_todays_games_with_time;
/// use chrono::{Local, TimeZone};
///
/// let now_local = Local::now();
/// let show_today = should_show_todays_games_with_time(now_local);
/// ```
pub fn should_show_todays_games_with_time(now_local: DateTime<Local>) -> bool {
    // Year-round cutoff at 12:00 local time (timezone-aware)
    let cutoff_time = NaiveTime::from_hms_opt(12, 0, 0).unwrap();
    let naive_cutoff = now_local.date_naive().and_time(cutoff_time);
    match now_local.timezone().from_local_datetime(&naive_cutoff) {
        chrono::LocalResult::Single(cutoff) => now_local >= cutoff,
        chrono::LocalResult::Ambiguous(_, latest) => now_local >= latest, // prefer later instant
        chrono::LocalResult::None => true, // defensive; noon should exist in all tz rules
    }
}


/// Try to fetch player names for specific player IDs with a reduced timeout
/// This is used as a fallback when cached player names are missing
#[allow(dead_code)]
async fn try_fetch_player_names_for_game(
    api_domain: &str,
    season: i32,
    game_id: i32,
    player_ids: &[i64],
) -> Option<HashMap<i64, String>> {
    use crate::data_fetcher::api::build_game_url;
    use crate::data_fetcher::models::{DetailedGameResponse, Player};
    use reqwest::Client;
    use std::time::Duration;
    use tracing::{debug, warn};

    if player_ids.is_empty() {
        return Some(HashMap::new());
    }

    debug!(
        "Attempting to fetch player names for {} players in game ID {} (season {})",
        player_ids.len(),
        game_id,
        season
    );

    // Get timeout from env var with 5 second default, clamped to safe range (1-30 seconds)
    let timeout_secs = std::env::var(env_vars::API_FETCH_TIMEOUT)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(5)
        .clamp(1, 30);

    // Create a client with a configurable timeout for this fallback attempt
    let client = match Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .build()
    {
        Ok(client) => client,
        Err(e) => {
            warn!("Failed to create HTTP client for player name fetch: {}", e);
            return None;
        }
    };

    // Use the provided API domain and season
    let url = build_game_url(api_domain, season, game_id);

    match client.get(&url).send().await {
        Ok(response) => {
            // Check for HTTP errors before trying to parse JSON
            let response = match response.error_for_status() {
                Ok(response) => response,
                Err(e) => {
                    debug!(
                        "HTTP error for player name fetch (game ID {}): {}",
                        game_id, e
                    );
                    return None;
                }
            };

            match response.json::<DetailedGameResponse>().await {
                Ok(game_response) => {
                    debug!(
                        "Successfully fetched detailed game data for player name lookup (game ID: {})",
                        game_id
                    );

                    let mut player_names = HashMap::new();
                    // Convert player_ids to HashSet for O(1) lookup instead of O(n) contains()
                    let mut wanted_ids: HashSet<i64> = HashSet::with_capacity(player_ids.len());
                    wanted_ids.extend(player_ids.iter().copied());

                    // Helper to process players and extract names for the requested IDs
                    let mut process_players = |players: &[Player]| {
                        for player in players {
                            if wanted_ids.contains(&player.id) {
                                let full_name =
                                    build_full_name(&player.first_name, &player.last_name);
                                let display_name = format_for_display(&full_name);
                                player_names.insert(player.id, display_name);
                            }
                        }
                    };

                    // Process both home and away team players
                    process_players(&game_response.home_team_players);
                    process_players(&game_response.away_team_players);

                    if !player_names.is_empty() {
                        debug!(
                            "Successfully fetched {} player names for game ID {}",
                            player_names.len(),
                            game_id
                        );
                        Some(player_names)
                    } else {
                        debug!(
                            "No player names found for requested IDs in game ID {}",
                            game_id
                        );
                        None
                    }
                }
                Err(e) => {
                    debug!("Failed to parse game response for player names: {}", e);
                    None
                }
            }
        }
        Err(e) => {
            debug!(
                "Failed to fetch game data for player names (game ID {}): {}",
                game_id, e
            );
            None
        }
    }
}

pub async fn create_basic_goal_events(
    game: &ScheduleGame,
    _api_domain: &str,
) -> Vec<GoalEventData> {
    use tracing::{info, warn};

    // If the game has goal events in the response, use them with cached names if available
    if !game.home_team.goal_events.is_empty() || !game.away_team.goal_events.is_empty() {
        info!(
            "Game ID {}: Using goal events from schedule response ({} home, {} away)",
            game.id,
            game.home_team.goal_events.len(),
            game.away_team.goal_events.len()
        );

        // Build names from embedded scorerPlayer when available; fallback to last name or numeric fallback
        let mut basic_names: HashMap<i64, String> = HashMap::new();

        let mut collect_name =
            |scorer_id: i64, maybe_first: Option<&str>, maybe_last: Option<&str>| {
                if let Some(last) = maybe_last {
                    // Prefer last name only per teletext style; add initial only if first is present and needed later
                    basic_names.insert(scorer_id, format_for_display(last));
                } else if let Some(first) = maybe_first {
                    // No last name in payload, use first as display (rare)
                    basic_names.insert(scorer_id, format_for_display(first));
                } else {
                    // Complete fallback
                    basic_names.insert(scorer_id, create_fallback_name(scorer_id));
                }
            };

        for goal in &game.home_team.goal_events {
            if let Some(p) = &goal.scorer_player {
                collect_name(
                    goal.scorer_player_id,
                    Some(&p.first_name),
                    Some(&p.last_name),
                );
            } else {
                collect_name(goal.scorer_player_id, None, None);
            }
        }
        for goal in &game.away_team.goal_events {
            if let Some(p) = &goal.scorer_player {
                collect_name(
                    goal.scorer_player_id,
                    Some(&p.first_name),
                    Some(&p.last_name),
                );
            } else {
                collect_name(goal.scorer_player_id, None, None);
            }
        }

        return process_goal_events(game, &basic_names);
    }

    // If no goal events but game has scores, create placeholder events
    warn!(
        "Game ID {}: No goal events in schedule response, creating {} placeholder events for score {}:{}",
        game.id,
        game.home_team.goals + game.away_team.goals,
        game.home_team.goals,
        game.away_team.goals
    );

    let mut events = Vec::new();

    // Create placeholder events for home team goals
    for i in 0..game.home_team.goals {
        events.push(GoalEventData {
            scorer_player_id: 0,                           // Unknown player ID
            scorer_name: "Tuntematon pelaaja".to_string(), // "Unknown player" in Finnish
            minute: 0,                                     // Unknown time
            home_team_score: i + 1,
            away_team_score: 0, // We don't know the exact progression
            is_winning_goal: false,
            goal_types: vec![],
            is_home_team: true,
            video_clip_url: None,
        });
    }

    // Create placeholder events for away team goals
    for i in 0..game.away_team.goals {
        events.push(GoalEventData {
            scorer_player_id: 0,                           // Unknown player ID
            scorer_name: "Tuntematon pelaaja".to_string(), // "Unknown player" in Finnish
            minute: 0,                                     // Unknown time
            home_team_score: 0,                            // We don't know the exact progression
            away_team_score: i + 1,
            is_winning_goal: false,
            goal_types: vec![],
            is_home_team: false,
            video_clip_url: None,
        });
    }

    if !events.is_empty() {
        info!(
            "Game ID {}: Created {} placeholder goal events",
            game.id,
            events.len()
        );
    }

    events
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
            scorer_player: None,
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
    fn test_should_show_todays_games_deterministic_examples() {
        use chrono::{Local, NaiveTime, TimeZone};
        let today = Local::now();

        let morning_naive = today
            .date_naive()
            .and_time(NaiveTime::from_hms_opt(11, 59, 59).unwrap());
        let morning_dt = chrono::Local
            .from_local_datetime(&morning_naive)
            .single()
            .unwrap();
        assert!(
            !should_show_todays_games_with_time(morning_dt),
            "Before noon should show yesterday's games"
        );

        let noon_naive = today
            .date_naive()
            .and_time(NaiveTime::from_hms_opt(12, 0, 0).unwrap());
        let noon_dt = chrono::Local
            .from_local_datetime(&noon_naive)
            .single()
            .unwrap();
        assert!(
            should_show_todays_games_with_time(noon_dt),
            "At/after noon should show today's games"
        );
    }

    #[test]
    fn test_should_show_todays_games_consistency() {
        // Multiple evaluations against the same captured time must be equal
        let now_local = chrono::Local::now();
        let result1 = should_show_todays_games_with_time(now_local);
        let result2 = should_show_todays_games_with_time(now_local);
        assert_eq!(result1, result2);
    }

    #[test]
    fn test_noon_cutoff_behavior() {
        // Test that we use noon (12:00) cutoff year-round for consistent teletext behavior
        // This test is now deterministic by capturing the time once and using the helper function

        use chrono::{Local, NaiveTime};

        let now_local = Local::now();
        let noon_time = NaiveTime::from_hms_opt(12, 0, 0).unwrap();
        let noon_today = now_local.date_naive().and_time(noon_time);
        let is_after_noon = now_local.naive_local() >= noon_today;

        // Use the helper function with the captured time to ensure deterministic behavior
        let result = should_show_todays_games_with_time(now_local);

        // Year-round behavior: result should match whether we're after noon
        assert_eq!(
            result, is_after_noon,
            "Year-round: result should match whether we're after noon"
        );
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

    #[tokio::test]
    async fn test_create_basic_goal_events() {
        let home_goal = create_test_goal_event(123, 600, 1, 0, vec!["EV".to_string()]);
        let away_goal = create_test_goal_event(456, 900, 1, 1, vec!["YV".to_string()]);

        let game = create_test_game(vec![home_goal], vec![away_goal]);

        let events = create_basic_goal_events(&game, "test-api.example.com").await;

        assert_eq!(events.len(), 2);

        // Should use fallback names since no player names cache is provided
        assert_eq!(events[0].scorer_name, "Pelaaja 123");
        assert_eq!(events[1].scorer_name, "Pelaaja 456");
    }

    #[tokio::test]
    async fn test_create_basic_goal_events_empty_game() {
        let game = create_test_game(vec![], vec![]);
        let events = create_basic_goal_events(&game, "test-api.example.com").await;
        assert!(events.is_empty());
    }

    #[tokio::test]
    async fn test_create_basic_goal_events_with_scores_but_no_events() {
        // Test the new fallback logic for games with scores but no goal events
        let mut game = create_test_game(vec![], vec![]);

        // Set scores but keep goal_events empty (simulates schedule response)
        game.home_team.goals = 2;
        game.away_team.goals = 1;

        let events = create_basic_goal_events(&game, "test-api.example.com").await;

        // Should create placeholder events based on scores
        assert_eq!(events.len(), 3); // 2 home + 1 away

        // Check home team events
        let home_events: Vec<_> = events.iter().filter(|e| e.is_home_team).collect();
        assert_eq!(home_events.len(), 2);
        assert_eq!(home_events[0].scorer_name, "Tuntematon pelaaja");
        assert_eq!(home_events[0].home_team_score, 1);
        assert_eq!(home_events[1].home_team_score, 2);

        // Check away team events
        let away_events: Vec<_> = events.iter().filter(|e| !e.is_home_team).collect();
        assert_eq!(away_events.len(), 1);
        assert_eq!(away_events[0].scorer_name, "Tuntematon pelaaja");
        assert_eq!(away_events[0].away_team_score, 1);
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

    // Tests for process_goal_events_with_disambiguation
    #[test]
    fn test_process_goal_events_with_disambiguation_basic() {
        let home_goal = create_test_goal_event(123, 900, 1, 0, vec!["EV".to_string()]);
        let away_goal = create_test_goal_event(456, 1200, 1, 1, vec!["YV".to_string()]);

        let game = create_test_game(vec![home_goal], vec![away_goal]);

        let home_players = vec![
            (123, "Mikko".to_string(), "Koivu".to_string()),
            (124, "Saku".to_string(), "Koivu".to_string()),
        ];
        let away_players = vec![(456, "Teemu".to_string(), "Selänne".to_string())];

        let events = process_goal_events_with_disambiguation(&game, &home_players, &away_players);

        assert_eq!(events.len(), 2);

        // Check home goal - should be disambiguated because two Koivus on home team
        let home_event = &events[0];
        assert_eq!(home_event.scorer_player_id, 123);
        assert_eq!(home_event.scorer_name, "Koivu M.");
        assert!(home_event.is_home_team);

        // Check away goal - should not be disambiguated because only one Selänne on away team
        let away_event = &events[1];
        assert_eq!(away_event.scorer_player_id, 456);
        assert_eq!(away_event.scorer_name, "Selänne");
        assert!(!away_event.is_home_team);
    }

    #[test]
    fn test_process_goal_events_with_disambiguation_team_scoped() {
        // Both teams have a "Koivu" but they shouldn't affect each other's disambiguation
        let home_goal = create_test_goal_event(123, 900, 1, 0, vec!["EV".to_string()]);
        let away_goal = create_test_goal_event(456, 1200, 1, 1, vec!["YV".to_string()]);

        let game = create_test_game(vec![home_goal], vec![away_goal]);

        let home_players = vec![(123, "Mikko".to_string(), "Koivu".to_string())];
        let away_players = vec![(456, "Saku".to_string(), "Koivu".to_string())];

        let events = process_goal_events_with_disambiguation(&game, &home_players, &away_players);

        assert_eq!(events.len(), 2);

        // Both should show as "Koivu" without disambiguation since they're on different teams
        let home_event = &events[0];
        assert_eq!(home_event.scorer_name, "Koivu");
        assert!(home_event.is_home_team);

        let away_event = &events[1];
        assert_eq!(away_event.scorer_name, "Koivu");
        assert!(!away_event.is_home_team);
    }

    #[test]
    fn test_process_goal_events_with_disambiguation_multiple_same_name() {
        let home_goal1 = create_test_goal_event(123, 600, 1, 0, vec!["EV".to_string()]);
        let home_goal2 = create_test_goal_event(124, 900, 2, 0, vec!["EV".to_string()]);
        let home_goal3 = create_test_goal_event(125, 1200, 3, 0, vec!["EV".to_string()]);

        let game = create_test_game(vec![home_goal1, home_goal2, home_goal3], vec![]);

        let home_players = vec![
            (123, "Mikko".to_string(), "Koivu".to_string()),
            (124, "Saku".to_string(), "Koivu".to_string()),
            (125, "Antti".to_string(), "Koivu".to_string()),
        ];
        let away_players = vec![];

        let events = process_goal_events_with_disambiguation(&game, &home_players, &away_players);

        assert_eq!(events.len(), 3);

        // All three should be disambiguated
        assert_eq!(events[0].scorer_name, "Koivu M.");
        assert_eq!(events[1].scorer_name, "Koivu S.");
        assert_eq!(events[2].scorer_name, "Koivu A.");
    }

    #[test]
    fn test_process_goal_events_with_disambiguation_mixed_scenario() {
        let home_goal1 = create_test_goal_event(123, 600, 1, 0, vec!["EV".to_string()]);
        let home_goal2 = create_test_goal_event(124, 900, 2, 0, vec!["EV".to_string()]);
        let home_goal3 = create_test_goal_event(125, 1200, 3, 0, vec!["EV".to_string()]);

        let game = create_test_game(vec![home_goal1, home_goal2, home_goal3], vec![]);

        let home_players = vec![
            (123, "Mikko".to_string(), "Koivu".to_string()),
            (124, "Saku".to_string(), "Koivu".to_string()),
            (125, "Teemu".to_string(), "Selänne".to_string()),
        ];
        let away_players = vec![];

        let events = process_goal_events_with_disambiguation(&game, &home_players, &away_players);

        assert_eq!(events.len(), 3);

        // Koivus should be disambiguated, Selänne should not
        assert_eq!(events[0].scorer_name, "Koivu M.");
        assert_eq!(events[1].scorer_name, "Koivu S.");
        assert_eq!(events[2].scorer_name, "Selänne");
    }

    #[test]
    fn test_process_goal_events_with_disambiguation_missing_player() {
        let home_goal = create_test_goal_event(999, 600, 1, 0, vec!["EV".to_string()]);
        let game = create_test_game(vec![home_goal], vec![]);

        let home_players = vec![(123, "Mikko".to_string(), "Koivu".to_string())];
        let away_players = vec![];

        let events = process_goal_events_with_disambiguation(&game, &home_players, &away_players);

        assert_eq!(events.len(), 1);
        // Should use fallback name for missing player
        assert_eq!(events[0].scorer_name, "Pelaaja 999");
    }

    #[test]
    fn test_process_goal_events_with_disambiguation_empty_teams() {
        let game = create_test_game(vec![], vec![]);
        let home_players = vec![];
        let away_players = vec![];

        let events = process_goal_events_with_disambiguation(&game, &home_players, &away_players);

        assert!(events.is_empty());
    }

    #[test]
    fn test_process_goal_events_with_disambiguation_unicode_names() {
        let home_goal = create_test_goal_event(123, 600, 1, 0, vec!["EV".to_string()]);
        let game = create_test_game(vec![home_goal], vec![]);

        let home_players = vec![
            (123, "Äkäslompolo".to_string(), "Kärppä".to_string()),
            (124, "Östen".to_string(), "Kärppä".to_string()),
        ];
        let away_players = vec![];

        let events = process_goal_events_with_disambiguation(&game, &home_players, &away_players);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].scorer_name, "Kärppä Ä.");
    }

    // Tests for process_team_goals_with_disambiguation
    #[test]
    fn test_process_team_goals_with_disambiguation() {
        let goal1 = create_test_goal_event(123, 600, 1, 0, vec!["EV".to_string()]);
        let goal2 = create_test_goal_event(124, 900, 2, 0, vec!["YV".to_string()]);
        let team = create_test_team_with_goals(vec![goal1, goal2]);

        let players = vec![
            (123, "Mikko".to_string(), "Koivu".to_string()),
            (124, "Saku".to_string(), "Koivu".to_string()),
        ];
        let context = DisambiguationContext::new(players);

        let mut events = Vec::new();
        process_team_goals_with_disambiguation(&team, &context, true, &mut events);

        assert_eq!(events.len(), 2);
        assert_eq!(events[0].scorer_name, "Koivu M.");
        assert_eq!(events[1].scorer_name, "Koivu S.");
        assert!(events[0].is_home_team);
        assert!(events[1].is_home_team);
    }

    #[test]
    fn test_process_team_goals_with_disambiguation_filters_cancelled() {
        let valid_goal = create_test_goal_event(123, 600, 1, 0, vec!["EV".to_string()]);
        let cancelled_goal_rl0 = create_test_goal_event(124, 900, 1, 0, vec!["RL0".to_string()]);
        let cancelled_goal_vt0 = create_test_goal_event(125, 1200, 1, 0, vec!["VT0".to_string()]);

        let team =
            create_test_team_with_goals(vec![valid_goal, cancelled_goal_rl0, cancelled_goal_vt0]);

        let players = vec![
            (123, "Mikko".to_string(), "Koivu".to_string()),
            (124, "Saku".to_string(), "Koivu".to_string()),
            (125, "Antti".to_string(), "Koivu".to_string()),
        ];
        let context = DisambiguationContext::new(players);

        let mut events = Vec::new();
        process_team_goals_with_disambiguation(&team, &context, true, &mut events);

        // Should only have the valid goal, cancelled goals filtered out
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].scorer_player_id, 123);
        assert_eq!(events[0].scorer_name, "Koivu M.");
    }

    #[test]
    fn test_process_team_goals_with_disambiguation_missing_player() {
        let goal = create_test_goal_event(999, 600, 1, 0, vec!["EV".to_string()]);
        let team = create_test_team_with_goals(vec![goal]);

        let players = vec![(123, "Mikko".to_string(), "Koivu".to_string())];
        let context = DisambiguationContext::new(players);

        let mut events = Vec::new();
        process_team_goals_with_disambiguation(&team, &context, true, &mut events);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].scorer_name, "Pelaaja 999");
    }
}
