use serde::Deserialize;

/// API response from /standings/?season={season}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StandingsResponse {
    pub season: Vec<ApiStandingsTeam>,
    #[serde(default)]
    pub playoffs_lines: Vec<u16>,
}

/// A single team entry from the standings API
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiStandingsTeam {
    pub team_id: String,
    pub team_name: String,
    pub ranking: u16,
    pub live_ranking: u16,
    pub games: u16,
    pub wins: u16,
    pub overtime_wins: u16,
    pub losses: u16,
    pub overtime_losses: u16,
    pub points: u16,
    pub live_points: u16,
    pub goals: u16,
    pub goals_against: u16,
    #[allow(dead_code)]
    pub live_goals: u16,
    #[allow(dead_code)]
    pub live_goals_against: u16,
}

/// Internal standings entry used for rendering
#[derive(Debug, Clone)]
pub struct StandingsEntry {
    pub team_name: String,
    pub team_id: String,
    pub games_played: u16,
    pub wins: u16,
    pub ot_wins: u16,
    pub ot_losses: u16,
    pub losses: u16,
    pub goals_for: u16,
    pub goals_against: u16,
    pub points: u16,
    pub live_points_delta: Option<i16>,
    pub live_position_change: Option<i16>,
}

impl StandingsEntry {
    #[allow(dead_code)]
    pub fn goal_difference(&self) -> i16 {
        self.goals_for as i16 - self.goals_against as i16
    }
}

impl From<&ApiStandingsTeam> for StandingsEntry {
    fn from(api: &ApiStandingsTeam) -> Self {
        let live_points_delta = if api.live_points != api.points {
            Some(api.live_points as i16 - api.points as i16)
        } else {
            None
        };

        let live_position_change = if api.live_ranking != api.ranking {
            // positive = moved up (lower ranking number is better)
            Some(api.ranking as i16 - api.live_ranking as i16)
        } else {
            None
        };

        Self {
            team_name: api.team_name.clone(),
            team_id: api.team_id.clone(),
            games_played: api.games,
            wins: api.wins,
            ot_wins: api.overtime_wins,
            ot_losses: api.overtime_losses,
            losses: api.losses,
            goals_for: api.goals,
            goals_against: api.goals_against,
            points: api.points,
            live_points_delta,
            live_position_change,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_api_team(
        points: u16,
        live_points: u16,
        ranking: u16,
        live_ranking: u16,
    ) -> ApiStandingsTeam {
        ApiStandingsTeam {
            team_id: "TPS".to_string(),
            team_name: "TPS".to_string(),
            ranking,
            live_ranking,
            games: 40,
            wins: 20,
            overtime_wins: 5,
            losses: 10,
            overtime_losses: 5,
            points,
            live_points,
            goals: 100,
            goals_against: 80,
            live_goals: 100,
            live_goals_against: 80,
        }
    }

    #[test]
    fn test_from_api_no_live_changes() {
        let api = make_api_team(60, 60, 3, 3);
        let entry = StandingsEntry::from(&api);

        assert_eq!(entry.team_name, "TPS");
        assert_eq!(entry.points, 60);
        assert_eq!(entry.games_played, 40);
        assert_eq!(entry.wins, 20);
        assert_eq!(entry.ot_wins, 5);
        assert_eq!(entry.ot_losses, 5);
        assert_eq!(entry.losses, 10);
        assert_eq!(entry.goals_for, 100);
        assert_eq!(entry.goals_against, 80);
        assert_eq!(entry.live_points_delta, None);
        assert_eq!(entry.live_position_change, None);
    }

    #[test]
    fn test_from_api_live_points_gained() {
        let api = make_api_team(60, 63, 3, 3);
        let entry = StandingsEntry::from(&api);

        assert_eq!(entry.live_points_delta, Some(3));
        assert_eq!(entry.live_position_change, None);
    }

    #[test]
    fn test_from_api_live_position_moved_up() {
        // ranking 5 → live_ranking 3 = moved up 2 positions
        let api = make_api_team(60, 63, 5, 3);
        let entry = StandingsEntry::from(&api);

        assert_eq!(entry.live_points_delta, Some(3));
        assert_eq!(entry.live_position_change, Some(2)); // positive = moved up
    }

    #[test]
    fn test_from_api_live_position_moved_down() {
        // ranking 3 → live_ranking 5 = moved down 2 positions
        let api = make_api_team(60, 60, 3, 5);
        let entry = StandingsEntry::from(&api);

        assert_eq!(entry.live_position_change, Some(-2)); // negative = moved down
    }

    #[test]
    fn test_goal_difference() {
        let api = make_api_team(60, 60, 1, 1);
        let entry = StandingsEntry::from(&api);

        assert_eq!(entry.goal_difference(), 20); // 100 - 80
    }

    #[test]
    fn test_standings_response_deserialize() {
        let json = r#"{
            "season": [{
                "teamId": "TPS",
                "teamName": "TPS",
                "ranking": 1,
                "liveRanking": 1,
                "games": 10,
                "wins": 8,
                "overtimeWins": 1,
                "losses": 0,
                "overtimeLosses": 1,
                "points": 26,
                "livePoints": 26,
                "goals": 35,
                "goalsAgainst": 20,
                "liveGoals": 35,
                "liveGoalsAgainst": 20
            }],
            "playoffsLines": [6, 10]
        }"#;

        let response: StandingsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.season.len(), 1);
        assert_eq!(response.season[0].team_id, "TPS");
        assert_eq!(response.season[0].ranking, 1);
        assert_eq!(response.playoffs_lines, vec![6, 10]);
    }

    #[test]
    fn test_standings_response_missing_playoffs_lines() {
        let json = r#"{
            "season": []
        }"#;

        let response: StandingsResponse = serde_json::from_str(json).unwrap();
        assert!(response.season.is_empty());
        assert!(response.playoffs_lines.is_empty());
    }
}
