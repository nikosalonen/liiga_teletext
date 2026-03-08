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
pub fn get_team_abbreviation(team_name: &str) -> String {
    match team_name {
        // Current Liiga teams (2024-25 season)
        "Tappara" | "Tampereen Tappara" => "TAP".to_string(),
        "HIFK" | "HIFK Helsinki" => "IFK".to_string(),
        "TPS" | "TPS Turku" => "TPS".to_string(),
        "JYP" | "Jyväskylän JYP" => "JYP".to_string(),
        "Ilves" | "Tampereen Ilves" => "ILV".to_string(),
        "KalPa" => "KAL".to_string(),
        "Kuopion KalPa" => "KUO".to_string(),
        "Kärpät" | "Oulun Kärpät" => "KÄR".to_string(),
        "Lukko" | "Rauman Lukko" => "LUK".to_string(),
        "Pelicans" | "Lahden Pelicans" => "PEL".to_string(),
        "SaiPa" | "Lappeenrannan SaiPa" => "SAI".to_string(),
        "Sport" | "Vaasan Sport" => "SPO".to_string(),
        "HPK" | "Hämeenlinnan HPK" => "HPK".to_string(),
        "Jukurit" | "Mikkelin Jukurit" => "JUK".to_string(),
        "Ässät" | "Porin Ässät" => "ÄSS".to_string(),
        "KooKoo" | "Kouvolan KooKoo" => "KOO".to_string(),
        "K-Espoo" => "KES".to_string(),

        // Fallback for unknown team names - extract letters only, uppercase, take first 3-4 chars
        _ => {
            let letters_only: String = team_name
                .chars()
                .filter(|c| c.is_alphabetic())
                .collect::<String>()
                .to_uppercase();

            if letters_only.len() >= 3 {
                letters_only.chars().take(3).collect()
            } else if letters_only.is_empty() {
                // If no letters found, return original string
                team_name.to_string()
            } else {
                // Less than 3 letters, return what we have
                letters_only
            }
        }
    }
}
