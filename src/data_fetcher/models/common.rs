use super::detailed::{DetailedGame, DetailedTeam};
use super::goals::{GoalEvent, GoalEventData};
use super::schedule::{ScheduleGame, ScheduleTeam};
use crate::teletext_ui::ScoreType;

#[derive(Debug, Clone, Hash)]
pub struct GameData {
    pub home_team: String,
    pub away_team: String,
    pub time: String,
    pub result: String,
    pub score_type: ScoreType,
    pub is_overtime: bool,
    pub is_shootout: bool,
    pub serie: String,
    pub goal_events: Vec<GoalEventData>,
    pub played_time: i32,
    pub start: String,
}

pub trait HasTeams {
    fn home_team(&self) -> &dyn HasGoalEvents;
    fn away_team(&self) -> &dyn HasGoalEvents;
}

pub trait HasGoalEvents {
    fn goal_events(&self) -> &[GoalEvent];
}

impl HasTeams for ScheduleGame {
    fn home_team(&self) -> &dyn HasGoalEvents {
        &self.home_team
    }
    fn away_team(&self) -> &dyn HasGoalEvents {
        &self.away_team
    }
}

impl HasTeams for DetailedGame {
    fn home_team(&self) -> &dyn HasGoalEvents {
        &self.home_team
    }
    fn away_team(&self) -> &dyn HasGoalEvents {
        &self.away_team
    }
}

impl HasGoalEvents for ScheduleTeam {
    fn goal_events(&self) -> &[GoalEvent] {
        &self.goal_events
    }
}

impl HasGoalEvents for DetailedTeam {
    fn goal_events(&self) -> &[GoalEvent] {
        &self.goal_events
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_fetcher::models::detailed::{DetailedGame, DetailedTeam};
    use crate::data_fetcher::models::goals::GoalEvent;
    use crate::data_fetcher::models::schedule::{ScheduleGame, ScheduleTeam};

    fn create_test_goal_event() -> GoalEvent {
        GoalEvent {
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
        }
    }

    fn create_test_schedule_team() -> ScheduleTeam {
        ScheduleTeam {
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
            goal_events: vec![create_test_goal_event()],
        }
    }

    fn create_test_schedule_game() -> ScheduleGame {
        ScheduleGame {
            id: 12345,
            season: 2024,
            start: "2024-01-15T18:30:00Z".to_string(),
            end: Some("2024-01-15T21:00:00Z".to_string()),
            home_team: create_test_schedule_team(),
            away_team: create_test_schedule_team(),
            finished_type: Some("ENDED_DURING_REGULAR_TIME".to_string()),
            started: true,
            ended: true,
            game_time: 3600,
            serie: "runkosarja".to_string(),
        }
    }

    #[test]
    fn test_has_teams_trait_schedule_game() {
        let game = create_test_schedule_game();

        let home_team = game.home_team();
        let away_team = game.away_team();

        // Test that the trait returns the correct teams
        assert_eq!(home_team.goal_events().len(), 1);
        assert_eq!(away_team.goal_events().len(), 1);
    }

    #[test]
    fn test_has_teams_trait_detailed_game() {
        let detailed_game = DetailedGame {
            id: 123,
            season: 2024,
            start: "2024-01-15T18:30:00Z".to_string(),
            end: None,
            home_team: DetailedTeam {
                team_id: "HIFK".to_string(),
                team_name: "HIFK Helsinki".to_string(),
                goals: 2,
                goal_events: vec![create_test_goal_event()],
                penalty_events: vec![],
            },
            away_team: DetailedTeam {
                team_id: "TPS".to_string(),
                team_name: "TPS Turku".to_string(),
                goals: 1,
                goal_events: vec![],
                penalty_events: vec![],
            },
            periods: vec![],
            finished_type: None,
            started: true,
            ended: false,
            game_time: 1200,
            serie: "runkosarja".to_string(),
        };

        let home_team = detailed_game.home_team();
        let away_team = detailed_game.away_team();

        assert_eq!(home_team.goal_events().len(), 1);
        assert_eq!(away_team.goal_events().len(), 0);
    }

    #[test]
    fn test_has_goal_events_trait_schedule_team() {
        let team = create_test_schedule_team();
        assert_eq!(team.goal_events().len(), 1);
        assert_eq!(team.goal_events()[0].scorer_player_id, 12345);
    }

    #[test]
    fn test_has_goal_events_trait_detailed_team() {
        let detailed_team = DetailedTeam {
            team_id: "HIFK".to_string(),
            team_name: "HIFK Helsinki".to_string(),
            goals: 1,
            goal_events: vec![create_test_goal_event()],
            penalty_events: vec![],
        };

        assert_eq!(detailed_team.goal_events().len(), 1);
        assert_eq!(detailed_team.goal_events()[0].scorer_player_id, 12345);
    }

    #[test]
    fn test_game_data_structure() {
        let game_data = GameData {
            home_team: "HIFK".to_string(),
            away_team: "TPS".to_string(),
            time: "18:30".to_string(),
            result: "2-1".to_string(),
            score_type: ScoreType::Final,
            is_overtime: false,
            is_shootout: false,
            serie: "runkosarja".to_string(),
            goal_events: vec![],
            played_time: 3600,
            start: "2024-01-15T18:30:00Z".to_string(),
        };

        assert_eq!(game_data.home_team, "HIFK");
        assert_eq!(game_data.away_team, "TPS");
        assert_eq!(game_data.result, "2-1");
        assert!(!game_data.is_overtime);
        assert!(!game_data.is_shootout);
    }
}
