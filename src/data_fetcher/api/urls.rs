//! URL building utilities for API endpoints

/// Builds a tournament URL for fetching game data.
/// This constructs the API endpoint for a specific tournament and date.
///
/// # Arguments
/// * `api_domain` - The base API domain
/// * `tournament` - The tournament identifier
/// * `date` - The date in YYYY-MM-DD format
///
/// # Returns
/// * `String` - The complete tournament URL
///
/// # Example
/// ```
/// use liiga_teletext::data_fetcher::api::build_tournament_url;
///
/// let url = build_tournament_url("https://api.example.com", "runkosarja", "2024-01-15");
/// assert_eq!(url, "https://api.example.com/games?tournament=runkosarja&date=2024-01-15");
/// ```
pub fn build_tournament_url(api_domain: &str, tournament: &str, date: &str) -> String {
    format!("{api_domain}/games?tournament={tournament}&date={date}")
}

/// Builds a game URL for fetching detailed game data.
/// This constructs the API endpoint for a specific game by season and game ID.
///
/// # Arguments
/// * `api_domain` - The base API domain
/// * `season` - The season year
/// * `game_id` - The unique game identifier
///
/// # Returns
/// * `String` - The complete game URL
///
/// # Example
/// ```
/// use liiga_teletext::data_fetcher::api::build_game_url;
///
/// let url = build_game_url("https://api.example.com", 2024, 12345);
/// assert_eq!(url, "https://api.example.com/games/2024/12345");
/// ```
pub fn build_game_url(api_domain: &str, season: i32, game_id: i32) -> String {
    format!("{api_domain}/games/{season}/{game_id}")
}

/// Builds a schedule URL for fetching season schedule data.
/// This constructs the API endpoint for a specific tournament and season.
///
/// # Arguments
/// * `api_domain` - The base API domain
/// * `season` - The season year
///
/// # Returns
/// * `String` - The complete schedule URL
///
/// # Example
/// ```
/// use liiga_teletext::data_fetcher::api::build_schedule_url;
///
/// let url = build_schedule_url("https://api.example.com", 2024);
/// assert_eq!(url, "https://api.example.com/schedule?tournament=runkosarja&week=1&season=2024");
/// ```
pub fn build_schedule_url(api_domain: &str, season: i32) -> String {
    format!("{api_domain}/schedule?tournament=runkosarja&week=1&season={season}")
}

/// Builds a schedule URL for a specific tournament type.
/// This constructs the API endpoint for a specific tournament and season.
///
/// # Arguments
/// * `api_domain` - The base API domain
/// * `tournament` - The tournament type (runkosarja, playoffs, playout, qualifications, valmistavat_ottelut)
/// * `season` - The season year
///
/// # Returns
/// * `String` - The complete schedule URL
///
/// # Example
/// ```
/// use liiga_teletext::data_fetcher::api::build_tournament_schedule_url;
///
/// let url = build_tournament_schedule_url("https://api.example.com", "playoffs", 2024);
/// assert_eq!(url, "https://api.example.com/schedule?tournament=playoffs&week=1&season=2024");
/// ```
pub fn build_tournament_schedule_url(api_domain: &str, tournament: &str, season: i32) -> String {
    format!("{api_domain}/schedule?tournament={tournament}&week=1&season={season}")
}

/// Creates a tournament key for caching and identification purposes.
/// This combines tournament name and date into a unique identifier.
///
/// # Arguments
/// * `tournament` - The tournament identifier
/// * `date` - The date in YYYY-MM-DD format
///
/// # Returns
/// * `String` - The tournament key (e.g., "runkosarja-2024-01-15")
///
/// # Example
/// ```
/// use liiga_teletext::data_fetcher::api::create_tournament_key;
///
/// let key = create_tournament_key("runkosarja", "2024-01-15");
/// assert_eq!(key, "runkosarja-2024-01-15");
/// ```
pub fn create_tournament_key(tournament: &str, date: &str) -> String {
    format!("{tournament}-{date}")
}