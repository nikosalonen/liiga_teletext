use chrono::{DateTime, Local, NaiveTime, TimeZone, Utc};

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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Local, NaiveTime, TimeZone};

    #[test]
    fn test_should_show_todays_games_deterministic_examples() {
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
}
