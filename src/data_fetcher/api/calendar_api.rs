//! Season schedule for the calendar view (page 224).

use crate::config::Config;
use crate::data_fetcher::api::http_client::create_http_client_with_timeout;
use crate::data_fetcher::api::tournament_logic::{TournamentType, fetch_tournament_games};
use crate::data_fetcher::models::calendar::SeasonCalendar;
use crate::error::AppError;
use tracing::info;

/// Every tournament a season can have games in. Each is one request for the
/// whole season; a tournament that isn't published yet (404) just has no
/// games.
const SEASON_TOURNAMENTS: [TournamentType; 5] = [
    TournamentType::ValmistavatOttelut,
    TournamentType::Runkosarja,
    TournamentType::Playoffs,
    TournamentType::Playout,
    TournamentType::Qualifications,
];

/// Fetches every game of `season` (named by its ending year) for the calendar.
///
/// Fails when the regular season schedule fails, or when nothing answered,
/// so the caller can keep the calendar on screen instead of showing an empty
/// season.
pub async fn fetch_season_calendar(
    config: &Config,
    season: i32,
) -> Result<SeasonCalendar, AppError> {
    let client = create_http_client_with_timeout(config.http_timeout_seconds)?;
    let games = fetch_tournament_games(&client, config, &SEASON_TOURNAMENTS, season).await?;
    info!("Season {season} calendar: {} games", games.len());
    Ok(SeasonCalendar::from_schedule(season, &games))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_fetcher::models::ScheduleApiGame;
    use chrono::NaiveDate;
    use serial_test::serial;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn game(id: i32, start: &str, serie: i32) -> ScheduleApiGame {
        ScheduleApiGame {
            id,
            season: 2027,
            start: start.to_string(),
            home_team_name: "Tappara".to_string(),
            away_team_name: "HIFK".to_string(),
            serie,
            finished_type: None,
            started: false,
            ended: false,
            game_time: None,
            play_off_phase: None,
            play_off_pair: None,
            play_off_req_wins: None,
            home_team_goals: 0,
            away_team_goals: 0,
        }
    }

    async fn config_for(server: &MockServer) -> Config {
        crate::data_fetcher::cache::clear_all_caches().await;
        Config {
            api_domain: server.uri(),
            log_file_path: None,
            http_timeout_seconds: crate::constants::DEFAULT_HTTP_TIMEOUT_SECONDS,
        }
    }

    fn date(text: &str) -> NaiveDate {
        NaiveDate::parse_from_str(text, "%Y-%m-%d").unwrap()
    }

    #[tokio::test]
    #[serial]
    async fn test_calendar_combines_tournaments_and_skips_unpublished_ones() {
        let server = MockServer::start().await;
        let config = config_for(&server).await;
        Mock::given(method("GET"))
            .and(path("/schedule"))
            .and(query_param("tournament", "runkosarja"))
            .and(query_param("season", "2027"))
            .respond_with(ResponseTemplate::new(200).set_body_json(vec![game(
                1,
                "2026-11-27T15:30:00Z",
                1,
            )]))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/schedule"))
            .and(query_param("tournament", "valmistavat_ottelut"))
            .respond_with(ResponseTemplate::new(200).set_body_json(vec![game(
                2,
                "2026-08-14T15:30:00Z",
                5,
            )]))
            .mount(&server)
            .await;
        // Playoffs, playout and qualifications aren't published yet
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let calendar = fetch_season_calendar(&config, 2027).await.unwrap();

        assert_eq!(calendar.season, 2027);
        assert!(calendar.has_games_on(date("2026-11-27")));
        assert!(calendar.has_games_on(date("2026-08-14")));
    }

    #[tokio::test]
    #[serial]
    async fn test_failed_regular_season_fetch_is_an_error() {
        let server = MockServer::start().await;
        let config = config_for(&server).await;
        Mock::given(method("GET"))
            .and(path("/schedule"))
            .and(query_param("tournament", "runkosarja"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_json(Vec::<ScheduleApiGame>::new()))
            .mount(&server)
            .await;

        assert!(fetch_season_calendar(&config, 2027).await.is_err());
    }
}
