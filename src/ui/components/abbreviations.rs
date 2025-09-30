/// Returns the abbreviated form of a team name for compact display.
///
/// This function maps full team names to their 3-4 character abbreviations
/// commonly used in Finnish hockey. For unknown team names, it generates
/// a fallback by taking letters only, converting to uppercase, and using
/// the first 3-4 characters.
///
/// # Arguments
/// * `team_name` - The full team name to abbreviate
///
/// # Returns
/// * `String` - The abbreviated team name
///
/// # Examples
/// ```
/// use liiga_teletext::get_team_abbreviation;
///
/// assert_eq!(get_team_abbreviation("Tappara"), "TAP");
/// assert_eq!(get_team_abbreviation("HIFK"), "IFK");
/// assert_eq!(get_team_abbreviation("HC Blues"), "HCB");
/// assert_eq!(get_team_abbreviation("K-Espoo"), "KES");
/// ```
#[allow(dead_code)]
pub fn get_team_abbreviation(team_name: &str) -> String {
    match team_name {
        // Current Liiga teams (2024-25 season)
        "Tappara" => "TAP".to_string(),
        "HIFK" => "IFK".to_string(),
        "TPS" => "TPS".to_string(),
        "JYP" => "JYP".to_string(),
        "Ilves" => "ILV".to_string(),
        "KalPa" => "KAL".to_string(),
        "Kärpät" => "KÄR".to_string(),
        "Lukko" => "LUK".to_string(),
        "Pelicans" => "PEL".to_string(),
        "SaiPa" => "SAI".to_string(),
        "Sport" => "SPO".to_string(),
        "HPK" => "HPK".to_string(),
        "Jukurit" => "JUK".to_string(),
        "Ässät" => "ÄSS".to_string(),
        "KooKoo" => "KOO".to_string(),
        "K-Espoo" => "KES".to_string(),

        // Alternative team name formats that might appear in API
        "HIFK Helsinki" => "IFK".to_string(),
        "TPS Turku" => "TPS".to_string(),
        "Tampereen Tappara" => "TAP".to_string(),
        "Tampereen Ilves" => "ILV".to_string(),
        "Jyväskylän JYP" => "JYP".to_string(),
        "Kuopion KalPa" => "KAL".to_string(),
        "Oulun Kärpät" => "KÄR".to_string(),
        "Rauman Lukko" => "LUK".to_string(),
        "Lahden Pelicans" => "PEL".to_string(),
        "Lappeenrannan SaiPa" => "SAI".to_string(),
        "Vaasan Sport" => "SPO".to_string(),
        "Hämeenlinnan HPK" => "HPK".to_string(),
        "Mikkelin Jukurit" => "JUK".to_string(),
        "Porin Ässät" => "ÄSS".to_string(),
        "Kouvolan KooKoo" => "KOO".to_string(),

        // Fallback for unknown team names - extract letters only, uppercase, take first 3-4 chars
        _ => {
            let letters_only: String = team_name
                .chars()
                .filter(|c| c.is_alphabetic())
                .collect::<String>()
                .to_uppercase();

            if letters_only.len() >= 3 {
                letters_only[..3.min(letters_only.len())].to_string()
            } else if !letters_only.is_empty() {
                letters_only
            } else {
                // If no letters found, use original team name as last resort
                team_name.to_string()
            }
        }
    }
}
