pub mod common;
pub mod detailed;
pub mod goals;
pub mod players;
pub mod schedule;

// Re-export all public types for backward compatibility
pub use common::{GameData, HasGoalEvents, HasTeams};
pub use detailed::{DetailedGame, DetailedGameResponse, DetailedTeam};
pub use goals::{GoalEvent, GoalEventData};
pub use players::Player;
pub use schedule::{ScheduleApiGame, ScheduleGame, ScheduleResponse, ScheduleTeam};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clone_implementations() {
        let goal_event = GoalEvent {
            scorer_player_id: 12345,
            log_time: "18:30:00".to_string(),
            game_time: 900,
            period: 1,
            event_id: 1,
            home_team_score: 1,
            away_team_score: 0,
            winning_goal: false,
            goal_types: vec!["EV".to_string()],
            assistant_player_ids: vec![67890, 11111],
            video_clip_url: Some("https://example.com/video.mp4".to_string()),
            scorer_player: None,
        };
        let cloned_goal = goal_event.clone();
        assert_eq!(goal_event.scorer_player_id, cloned_goal.scorer_player_id);
        assert_eq!(goal_event.log_time, cloned_goal.log_time);

        let team = ScheduleTeam {
            team_id: Some("HIFK".to_string()),
            team_placeholder: None,
            team_name: Some("HIFK Helsinki".to_string()),
            goals: 2,
            time_out: None,
            powerplay_instances: 3,
            powerplay_goals: 1,
            short_handed_instances: 1,
            short_handed_goals: 0,
            ranking: Some(5),
            game_start_date_time: Some("2024-01-15T18:30:00Z".to_string()),
            goal_events: vec![goal_event.clone()],
        };
        let cloned_team = team.clone();
        assert_eq!(team.team_id, cloned_team.team_id);
        assert_eq!(team.goals, cloned_team.goals);

        let game = ScheduleGame {
            id: 12345,
            season: 2024,
            start: "2024-01-15T18:30:00Z".to_string(),
            end: Some("2024-01-15T21:00:00Z".to_string()),
            home_team: team.clone(),
            away_team: team,
            finished_type: Some("ENDED_DURING_REGULAR_TIME".to_string()),
            started: true,
            ended: true,
            game_time: 3600,
            serie: "runkosarja".to_string(),
        };
        let cloned_game = game.clone();
        assert_eq!(game.id, cloned_game.id);
        assert_eq!(game.season, cloned_game.season);
    }

    #[test]
    fn test_debug_implementations() {
        let goal_event = GoalEvent {
            scorer_player_id: 12345,
            log_time: "18:30:00".to_string(),
            game_time: 900,
            period: 1,
            event_id: 1,
            home_team_score: 1,
            away_team_score: 0,
            winning_goal: false,
            goal_types: vec!["EV".to_string()],
            assistant_player_ids: vec![67890, 11111],
            video_clip_url: Some("https://example.com/video.mp4".to_string()),
            scorer_player: None,
        };
        let debug_string = format!("{goal_event:?}");
        assert!(debug_string.contains("GoalEvent"));
        assert!(debug_string.contains("12345"));

        let team = ScheduleTeam {
            team_id: Some("HIFK".to_string()),
            team_placeholder: None,
            team_name: Some("HIFK Helsinki".to_string()),
            goals: 2,
            time_out: None,
            powerplay_instances: 3,
            powerplay_goals: 1,
            short_handed_instances: 1,
            short_handed_goals: 0,
            ranking: Some(5),
            game_start_date_time: Some("2024-01-15T18:30:00Z".to_string()),
            goal_events: vec![goal_event.clone()],
        };
        let debug_string = format!("{team:?}");
        assert!(debug_string.contains("ScheduleTeam"));

        let game = ScheduleGame {
            id: 12345,
            season: 2024,
            start: "2024-01-15T18:30:00Z".to_string(),
            end: Some("2024-01-15T21:00:00Z".to_string()),
            home_team: team.clone(),
            away_team: team,
            finished_type: Some("ENDED_DURING_REGULAR_TIME".to_string()),
            started: true,
            ended: true,
            game_time: 3600,
            serie: "runkosarja".to_string(),
        };
        let debug_string = format!("{game:?}");
        assert!(debug_string.contains("ScheduleGame"));
        assert!(debug_string.contains("12345"));
    }
}
