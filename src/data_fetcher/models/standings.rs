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
