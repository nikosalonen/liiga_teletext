//! Startup check for the timezone that `chrono::Local` actually resolved to.
//!
//! `chrono` resolves the local zone by trying `TZ`, then `/etc/localtime`, then
//! the system zone name — and ends that chain with `unwrap_or_else(TimeZone::utc)`
//! (see `chrono`'s `offset::local::unix`). When every step fails it hands back
//! UTC with no error of any kind.
//!
//! That matters here because every game time on screen goes through
//! [`format_time`](crate::data_fetcher::processors::format_time), which converts
//! via `chrono::Local`. A silent fallback therefore shifts the entire page by the
//! local UTC offset with nothing to indicate anything went wrong — an 18.30 game
//! renders as 15.30 and looks perfectly plausible.
//!
//! We cannot repair the offset without bundling our own copy of the timezone
//! database, so this module only reports the problem.
//!
//! # Reading the machine's zone without going through `TZ`
//!
//! The comparison only works against a source of truth that `TZ` cannot move.
//! `iana_time_zone::get_timezone()` is *not* one: it honours `TZ`, so with
//! `TZ=UTC` on a Helsinki machine it reports `GMT` and the discrepancy vanishes
//! before we can see it. The `/etc/localtime` symlink is unaffected by `TZ` and
//! still names the configured zone, so that is what we read.

/// Where the timezone database lives on the Unix systems `chrono` supports.
/// Matches `chrono`'s own `TZDB_LOCATION`.
#[cfg(unix)]
const TZDB_DIR: &str = "/usr/share/zoneinfo";

/// Marks the start of the zone name inside a `/etc/localtime` symlink target,
/// e.g. `/var/db/timezone/zoneinfo/Europe/Helsinki` on macOS.
#[cfg(unix)]
const ZONEINFO_MARKER: &str = "/zoneinfo/";

/// A reason the process is rendering times at UTC when the machine is
/// configured for a different zone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimezoneProblem {
    /// `TZ` resolves to UTC and overrides the machine's configured zone. Covers
    /// both an explicit `TZ=UTC` and an empty `TZ`, which POSIX also defines as
    /// UTC and which usually comes from a shell or wrapper exporting a variable
    /// that was never assigned.
    TzEnvForcesUtc {
        tz_value: String,
        system_zone: String,
    },
    /// No `TZ` was set and none of the zone data `chrono` looks for could be
    /// read, so its resolution chain bottomed out at UTC.
    ZoneDataUnreadable { system_zone: String },
}

impl TimezoneProblem {
    /// An explanation aimed at the person running the app, naming the zone they
    /// expected and what to do about it.
    pub fn message(&self) -> String {
        match self {
            TimezoneProblem::TzEnvForcesUtc {
                tz_value,
                system_zone,
            } => {
                let setting = if tz_value.is_empty() {
                    "TZ is set to an empty value".to_string()
                } else {
                    format!("TZ is set to '{tz_value}'")
                };
                format!(
                    "{setting}, so game times are being shown at UTC+00:00 instead of \
                     your system timezone ({system_zone}). Unset TZ, or set it to a real \
                     zone name, to see correct start times."
                )
            }
            TimezoneProblem::ZoneDataUnreadable { system_zone } => format!(
                "Timezone data for {system_zone} could not be read, so game times are \
                 being shown at UTC+00:00 and will be wrong by your local UTC offset. \
                 Check that /etc/localtime and /usr/share/zoneinfo are readable."
            ),
        }
    }
}

/// Checks whether this process resolved a usable local timezone.
///
/// Returns `None` when everything is fine, or when the situation is merely
/// suspicious rather than demonstrably wrong.
///
/// Only meaningful on Unix: on Windows `chrono` reads the zone through the Win32
/// API and has no `/etc/localtime` fallback chain to fail, so the check is a
/// no-op there.
#[cfg(unix)]
pub fn check() -> Option<TimezoneProblem> {
    use chrono::{Local, Offset};

    let local_offset_seconds = Local::now().offset().fix().local_minus_utc();
    let tz_env = std::env::var("TZ").ok();
    let system_zone = configured_system_zone();

    diagnose(
        local_offset_seconds,
        tz_env.as_deref(),
        system_zone.as_deref(),
        zone_data_readable(system_zone.as_deref()),
    )
}

#[cfg(not(unix))]
pub fn check() -> Option<TimezoneProblem> {
    None
}

/// The zone the machine is configured for, read from the `/etc/localtime`
/// symlink so that `TZ` cannot influence the answer.
///
/// Returns `None` when `/etc/localtime` is a plain copy of the zone file rather
/// than a symlink, which carries no zone name. Falling back to
/// `iana_time_zone` there would reintroduce the `TZ` sensitivity this function
/// exists to avoid.
#[cfg(unix)]
fn configured_system_zone() -> Option<String> {
    let target = std::fs::read_link("/etc/localtime").ok()?;
    zone_name_from_link_target(target.to_str()?)
}

/// Extracts `Europe/Helsinki` from a target such as
/// `/var/db/timezone/zoneinfo/Europe/Helsinki`.
#[cfg(unix)]
fn zone_name_from_link_target(target: &str) -> Option<String> {
    let start = target.rfind(ZONEINFO_MARKER)? + ZONEINFO_MARKER.len();
    let name = target.get(start..)?;
    if name.is_empty() {
        return None;
    }
    Some(name.to_string())
}

/// Whether any of the zone data `chrono` consults can actually be opened.
#[cfg(unix)]
fn zone_data_readable(system_zone: Option<&str>) -> bool {
    if std::fs::File::open("/etc/localtime").is_ok() {
        return true;
    }
    // Reject anything that could climb out of the database directory; the name
    // comes from the OS, but this is only ever an existence check.
    match system_zone {
        Some(zone) if !zone.contains("..") && !zone.starts_with('/') => {
            std::fs::File::open(format!("{TZDB_DIR}/{zone}")).is_ok()
        }
        _ => false,
    }
}

/// The decision itself, split out from environment access so it can be tested
/// without mutating process-global state.
///
/// `system_zone` must come from a `TZ`-independent source; see the module docs.
fn diagnose(
    local_offset_seconds: i32,
    tz_env: Option<&str>,
    system_zone: Option<&str>,
    zone_data_readable: bool,
) -> Option<TimezoneProblem> {
    // Any non-zero offset means a real zone was resolved; both the fallback and
    // a UTC-forcing TZ can only ever produce +00:00.
    if local_offset_seconds != 0 {
        return None;
    }

    // Without a zone name there is nothing to contradict the UTC result, so we
    // have no grounds to claim it is wrong.
    let system_zone = system_zone?;
    if is_utc_equivalent(system_zone) {
        return None;
    }

    match tz_env {
        // Reached only when TZ produced +00:00 while the machine is configured
        // for something else — TZ is overriding the system zone.
        Some(tz_value) => Some(TimezoneProblem::TzEnvForcesUtc {
            tz_value: tz_value.to_string(),
            system_zone: system_zone.to_string(),
        }),
        None if !zone_data_readable => Some(TimezoneProblem::ZoneDataUnreadable {
            system_zone: system_zone.to_string(),
        }),
        // Readable zone data, no TZ, and a UTC result is what a genuine
        // UTC+00:00 zone such as Europe/London in winter looks like.
        None => None,
    }
}

/// Zone names that are exactly UTC, where rendering at UTC is correct.
///
/// Matched exactly rather than by prefix: `Etc/GMT+5` is UTC-05:00, not UTC.
fn is_utc_equivalent(zone: &str) -> bool {
    matches!(
        zone,
        "UTC"
            | "Etc/UTC"
            | "UCT"
            | "Etc/UCT"
            | "GMT"
            | "GMT0"
            | "GMT+0"
            | "GMT-0"
            | "Etc/GMT"
            | "Etc/GMT0"
            | "Etc/GMT+0"
            | "Etc/GMT-0"
            | "Greenwich"
            | "Etc/Greenwich"
            | "Universal"
            | "Etc/Universal"
            | "Zulu"
            | "Etc/Zulu"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const EEST: i32 = 3 * 3600;
    const UTC: i32 = 0;

    #[test]
    fn resolved_non_utc_offset_is_never_a_problem() {
        assert_eq!(
            diagnose(EEST, None, Some("Europe/Helsinki"), true),
            None,
            "A non-zero offset means chrono resolved a real zone"
        );
    }

    #[test]
    fn tz_set_to_utc_over_a_non_utc_system_zone_is_reported() {
        // The case observed in macOS Terminal.app: TZ=UTC while /etc/localtime
        // still points at Europe/Helsinki, shifting every game time by 3 hours.
        assert_eq!(
            diagnose(UTC, Some("UTC"), Some("Europe/Helsinki"), true),
            Some(TimezoneProblem::TzEnvForcesUtc {
                tz_value: "UTC".to_string(),
                system_zone: "Europe/Helsinki".to_string(),
            })
        );
    }

    #[test]
    fn empty_tz_env_with_non_utc_system_zone_is_reported() {
        assert_eq!(
            diagnose(UTC, Some(""), Some("Europe/Helsinki"), true),
            Some(TimezoneProblem::TzEnvForcesUtc {
                tz_value: String::new(),
                system_zone: "Europe/Helsinki".to_string(),
            })
        );
    }

    #[test]
    fn tz_forcing_utc_is_fine_when_the_machine_really_is_on_utc() {
        for zone in ["UTC", "Etc/UTC", "GMT", "Etc/GMT", "Universal", "Zulu"] {
            assert_eq!(
                diagnose(UTC, Some("UTC"), Some(zone), true),
                None,
                "{zone} is UTC, so rendering at UTC is correct"
            );
        }
    }

    #[test]
    fn a_tz_naming_a_real_zone_is_not_reported() {
        // TZ=Europe/Helsinki resolves to +03:00, so it never reaches the TZ arm.
        assert_eq!(
            diagnose(EEST, Some("Europe/Helsinki"), Some("Europe/Helsinki"), true),
            None
        );
    }

    #[test]
    fn unreadable_zone_data_with_no_tz_env_is_reported() {
        assert_eq!(
            diagnose(UTC, None, Some("Europe/Helsinki"), false),
            Some(TimezoneProblem::ZoneDataUnreadable {
                system_zone: "Europe/Helsinki".to_string(),
            })
        );
    }

    #[test]
    fn zones_that_are_legitimately_at_utc_are_not_reported() {
        // Europe/London in winter and Atlantic/Reykjavik year-round sit at
        // +00:00 with perfectly healthy zone data and no TZ override. Warning
        // here would be a false positive, which is worse than staying quiet.
        assert_eq!(diagnose(UTC, None, Some("Europe/London"), true), None);
        assert_eq!(diagnose(UTC, None, Some("Atlantic/Reykjavik"), true), None);
    }

    #[test]
    fn etc_gmt_offset_zones_are_not_treated_as_utc() {
        // "Etc/GMT+5" is UTC-05:00, not UTC, so a UTC result there is suspect.
        assert_eq!(
            diagnose(UTC, Some(""), Some("Etc/GMT+5"), true),
            Some(TimezoneProblem::TzEnvForcesUtc {
                tz_value: String::new(),
                system_zone: "Etc/GMT+5".to_string(),
            })
        );
    }

    #[test]
    fn nothing_is_reported_without_a_system_zone_to_compare_against() {
        assert_eq!(
            diagnose(UTC, Some(""), None, false),
            None,
            "Without a system zone we cannot prove the UTC result is wrong"
        );
    }

    #[test]
    fn messages_name_the_zone_and_the_tz_value() {
        let message = TimezoneProblem::TzEnvForcesUtc {
            tz_value: "UTC".to_string(),
            system_zone: "Europe/Helsinki".to_string(),
        }
        .message();
        assert!(message.contains("Europe/Helsinki"), "{message}");
        assert!(message.contains("'UTC'"), "{message}");

        let message = TimezoneProblem::TzEnvForcesUtc {
            tz_value: String::new(),
            system_zone: "Europe/Helsinki".to_string(),
        }
        .message();
        assert!(message.contains("empty value"), "{message}");

        let message = TimezoneProblem::ZoneDataUnreadable {
            system_zone: "Europe/Helsinki".to_string(),
        }
        .message();
        assert!(message.contains("Europe/Helsinki"), "{message}");
        assert!(message.contains("UTC"), "{message}");
    }

    #[cfg(unix)]
    #[test]
    fn zone_names_are_parsed_from_symlink_targets() {
        // macOS
        assert_eq!(
            zone_name_from_link_target("/var/db/timezone/zoneinfo/Europe/Helsinki").as_deref(),
            Some("Europe/Helsinki")
        );
        // Linux
        assert_eq!(
            zone_name_from_link_target("/usr/share/zoneinfo/Europe/Helsinki").as_deref(),
            Some("Europe/Helsinki")
        );
        // Relative target, as used by some distributions
        assert_eq!(
            zone_name_from_link_target("../usr/share/zoneinfo/UTC").as_deref(),
            Some("UTC")
        );
        assert_eq!(zone_name_from_link_target("/etc/some/other/path"), None);
        assert_eq!(zone_name_from_link_target("/usr/share/zoneinfo/"), None);
    }

    #[test]
    fn check_agrees_with_the_running_environment() {
        // The test process has a working timezone, so the real check must be
        // quiet. This guards against the check misfiring on developer machines
        // and in CI.
        assert_eq!(check(), None);
    }
}
