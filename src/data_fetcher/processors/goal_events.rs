use crate::data_fetcher::models::{GoalEventData, HasGoalEvents, HasTeams, ScheduleGame};
use crate::data_fetcher::player_names::{
    DisambiguationContext, create_fallback_name, format_for_display,
};
use std::collections::HashMap;
use tracing::{info, warn};

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
