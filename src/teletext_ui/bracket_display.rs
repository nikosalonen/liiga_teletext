use crate::data_fetcher::models::bracket::{BracketMatchup, BracketPhase, PlayoffBracket};
use crate::teletext_ui::core::TeletextRow;

// ANSI 256 color codes for bracket display
const CYAN: u8 = 51;
const YELLOW: u8 = 226;
const GREEN: u8 = 46;
const WHITE: u8 = 231;
const DIM: u8 = 240;

const RESET: &str = "\x1b[0m";

/// Minimum terminal width for tree layout, varies by number of phases.
/// 3-phase bracket (QF/SF/Final): 60 columns
/// 4+ phase bracket (includes first round or bronze): 75 columns
fn min_tree_width(phase_count: usize) -> u16 {
    if phase_count >= 4 { 75 } else { 60 }
}

/// Maximum team name length, varies by number of phases.
fn max_team_len(phase_count: usize) -> usize {
    if phase_count >= 4 { 7 } else { 9 }
}

/// Truncates a team name to fit within `max_len` characters.
/// Uses char-safe truncation and appends "." if truncated.
pub fn truncate_team_name(name: &str, max_len: usize) -> String {
    if max_len == 0 {
        return String::new();
    }
    let char_count = name.chars().count();
    if char_count <= max_len {
        return name.to_string();
    }
    // Truncate to (max_len - 1) chars and append "."
    let truncated: String = name.chars().take(max_len - 1).collect();
    format!("{truncated}.")
}

/// Renders the complete playoff bracket as a list of `TeletextRow::BracketLine` entries.
///
/// Chooses between stacked (narrow terminal) and tree (wide terminal) layout
/// based on `terminal_width` and the number of playoff phases.
pub fn render_bracket(bracket: &PlayoffBracket, terminal_width: u16) -> Vec<TeletextRow> {
    if !bracket.has_data || bracket.phases.is_empty() {
        return vec![
            TeletextRow::BracketLine(String::new()),
            TeletextRow::BracketLine(format!(
                "{} PUDOTUSPELIT EIV\u{00C4}T OLE K\u{00C4}YNNISS\u{00C4} {}",
                color(WHITE),
                RESET
            )),
        ];
    }

    let phase_count = bracket.phases.len();
    if terminal_width < min_tree_width(phase_count) {
        render_stacked(bracket, terminal_width)
    } else {
        render_tree(bracket, terminal_width)
    }
}

// ---------------------------------------------------------------------------
// Stacked layout (narrow terminals)
// ---------------------------------------------------------------------------

/// Renders bracket in stacked/vertical mode: each phase listed sequentially
/// with matchups displayed as `Team1  W1 - W2  Team2`.
fn render_stacked(bracket: &PlayoffBracket, terminal_width: u16) -> Vec<TeletextRow> {
    let mut rows: Vec<TeletextRow> = Vec::new();
    let max_name = (terminal_width as usize).saturating_sub(16).min(12);

    for phase in &bracket.phases {
        // Phase header
        rows.push(TeletextRow::BracketLine(String::new()));
        rows.push(TeletextRow::BracketLine(format!(
            "{}{}{}",
            color(CYAN),
            phase.name,
            RESET
        )));

        for m in &phase.matchups {
            let t1 = truncate_team_name(&m.team1, max_name);
            let t2 = truncate_team_name(&m.team2, max_name);
            let (w1_color, w2_color) = win_colors(m);
            let live_marker = if m.has_live_game {
                format!(" {}\u{25CF}{}", color(CYAN), RESET)
            } else {
                String::new()
            };
            rows.push(TeletextRow::BracketLine(format!(
                "{}{:<width$}{}  {}{}{} - {}{}{}\
                 {}  {}{:<width$}{}{}",
                team_color(m, true),
                t1,
                RESET,
                w1_color,
                m.team1_wins,
                RESET,
                w2_color,
                m.team2_wins,
                RESET,
                live_marker,
                team_color(m, false),
                t2,
                RESET,
                winner_suffix(m),
                width = max_name,
            )));
        }
    }

    // Champion label
    append_champion(&bracket.phases, &mut rows);
    rows
}

// ---------------------------------------------------------------------------
// Tree layout (wide terminals)
// ---------------------------------------------------------------------------

/// Renders bracket as a tree with box-drawing characters.
///
/// Uses reseeding-aware grouping: SF matchups are linked back to QF matchups
/// by matching team names, so the bracket halves are derived dynamically.
fn render_tree(bracket: &PlayoffBracket, _terminal_width: u16) -> Vec<TeletextRow> {
    let mut rows: Vec<TeletextRow> = Vec::new();
    let phase_count = bracket.phases.len();
    let name_max = max_team_len(phase_count);

    // Separate phases by role
    let (early_phases, sf_phase, final_phase, bronze_phase) = classify_phases(&bracket.phases);

    // Build bracket halves using reseeding-aware grouping
    let halves = build_bracket_halves(&early_phases, sf_phase, name_max);

    for (half_idx, half) in halves.iter().enumerate() {
        if half_idx > 0 {
            rows.push(TeletextRow::BracketLine(String::new()));
        }
        render_half(half, &mut rows, name_max);
    }

    // Final
    if let Some(fp) = final_phase {
        rows.push(TeletextRow::BracketLine(String::new()));
        rows.push(TeletextRow::BracketLine(format!(
            "{}{}{}",
            color(CYAN),
            fp.name,
            RESET
        )));
        for m in &fp.matchups {
            render_matchup_tree(m, &mut rows, name_max);
        }
    }

    // Bronze
    if let Some(bp) = bronze_phase {
        rows.push(TeletextRow::BracketLine(String::new()));
        rows.push(TeletextRow::BracketLine(format!(
            "{}{}{}",
            color(CYAN),
            bp.name,
            RESET
        )));
        for m in &bp.matchups {
            render_matchup_tree(m, &mut rows, name_max);
        }
    }

    // Champion label
    append_champion(&bracket.phases, &mut rows);
    rows
}

/// Classifies phases into early rounds (QF / first round), SF, final, and bronze.
fn classify_phases(
    phases: &[BracketPhase],
) -> (
    Vec<&BracketPhase>,
    Option<&BracketPhase>,
    Option<&BracketPhase>,
    Option<&BracketPhase>,
) {
    let mut early: Vec<&BracketPhase> = Vec::new();
    let mut sf: Option<&BracketPhase> = None;
    let mut final_p: Option<&BracketPhase> = None;
    let mut bronze: Option<&BracketPhase> = None;

    for phase in phases {
        match phase.phase_number {
            4 => bronze = Some(phase),
            5 => final_p = Some(phase),
            3 => sf = Some(phase),
            _ => early.push(phase), // phase 1 (first round) and 2 (QF)
        }
    }
    // Sort early phases by phase number
    early.sort_by_key(|p| p.phase_number);

    (early, sf, final_p, bronze)
}

/// A bracket half: a group of early-round matchups feeding into one SF matchup.
struct BracketHalf<'a> {
    early_matchups: Vec<&'a BracketMatchup>,
    sf_matchup: Option<&'a BracketMatchup>,
    /// Phase headers for early rounds in this half
    early_phase_name: Option<String>,
}

/// Builds bracket halves by linking SF teams back to QF/early-round winners.
///
/// This is the reseeding-aware algorithm:
/// 1. For each SF matchup, find which early-round matchups produced its teams
/// 2. Group early matchups into halves based on SF connections
/// 3. Unmatched early matchups form their own independent groups
fn build_bracket_halves<'a>(
    early_phases: &[&'a BracketPhase],
    sf_phase: Option<&'a BracketPhase>,
    _name_max: usize,
) -> Vec<BracketHalf<'a>> {
    // Collect all early matchups with their phase info
    let all_early: Vec<&BracketMatchup> = early_phases
        .iter()
        .flat_map(|p| p.matchups.iter())
        .collect();

    let early_phase_name = early_phases.last().map(|p| p.name.clone());

    let Some(sf) = sf_phase else {
        // No SF phase: each early matchup is its own group, or all in one half
        if all_early.is_empty() {
            return Vec::new();
        }
        // Split into pairs for visual balance
        let mid = all_early.len().div_ceil(2);
        let mut halves = Vec::new();
        halves.push(BracketHalf {
            early_matchups: all_early[..mid].to_vec(),
            sf_matchup: None,
            early_phase_name: early_phase_name.clone(),
        });
        if mid < all_early.len() {
            halves.push(BracketHalf {
                early_matchups: all_early[mid..].to_vec(),
                sf_matchup: None,
                early_phase_name,
            });
        }
        return halves;
    };

    let mut halves: Vec<BracketHalf> = Vec::new();
    let mut claimed: Vec<bool> = vec![false; all_early.len()];

    for sf_matchup in &sf.matchups {
        let mut connected: Vec<&BracketMatchup> = Vec::new();

        for (i, em) in all_early.iter().enumerate() {
            if claimed[i] {
                continue;
            }
            if matchup_feeds_into(em, sf_matchup) {
                connected.push(em);
                claimed[i] = true;
            }
        }

        halves.push(BracketHalf {
            early_matchups: connected,
            sf_matchup: Some(sf_matchup),
            early_phase_name: early_phase_name.clone(),
        });
    }

    // Any unclaimed early matchups go into a standalone group
    let unclaimed: Vec<&BracketMatchup> = all_early
        .iter()
        .enumerate()
        .filter(|(i, _)| !claimed[*i])
        .map(|(_, m)| *m)
        .collect();

    if !unclaimed.is_empty() {
        halves.push(BracketHalf {
            early_matchups: unclaimed,
            sf_matchup: None,
            early_phase_name,
        });
    }

    halves
}

/// Checks whether an early-round matchup feeds into a semifinal matchup.
///
/// A matchup feeds into an SF if the SF contains one of its teams (either as
/// winner, team1, or team2). This handles both decided and in-progress series.
fn matchup_feeds_into(early: &BracketMatchup, sf: &BracketMatchup) -> bool {
    let sf_teams = [&sf.team1, &sf.team2];
    // Check if the winner of the early matchup appears in SF
    if let Some(ref winner) = early.winner
        && sf_teams.contains(&winner)
    {
        return true;
    }
    // Also check team names directly (for in-progress series where winner may
    // coincidentally share a name, or when teams advance)
    sf_teams
        .iter()
        .any(|t| *t == &early.team1 || *t == &early.team2)
}

/// Renders one bracket half: early-round matchups followed by an optional SF matchup.
fn render_half(half: &BracketHalf, rows: &mut Vec<TeletextRow>, name_max: usize) {
    // Show early phase header if we have early matchups
    if !half.early_matchups.is_empty() {
        if let Some(ref phase_name) = half.early_phase_name {
            rows.push(TeletextRow::BracketLine(format!(
                "{}{}{}",
                color(CYAN),
                phase_name,
                RESET
            )));
        }
        for m in &half.early_matchups {
            render_matchup_tree(m, rows, name_max);
            // Small gap between matchups in same half
        }
    }

    if let Some(sf_m) = half.sf_matchup {
        rows.push(TeletextRow::BracketLine(format!(
            "{}V\u{00C4}LIER\u{00C4}T{}",
            color(CYAN),
            RESET
        )));
        render_matchup_tree(sf_m, rows, name_max);
    }
}

/// Renders a single matchup as 3-line tree block with box-drawing characters:
/// ```text
/// Team1  4 ─┐
///            ├── Winner
/// Team2  2 ─┘
/// ```
fn render_matchup_tree(m: &BracketMatchup, rows: &mut Vec<TeletextRow>, name_max: usize) {
    let t1 = truncate_team_name(&m.team1, name_max);
    let t2 = truncate_team_name(&m.team2, name_max);
    let (w1_color, w2_color) = win_colors(m);

    let t1_padded = format!("{:<width$}", t1, width = name_max);
    let t2_padded = format!("{:<width$}", t2, width = name_max);

    let winner_label = match &m.winner {
        Some(w) => truncate_team_name(w, name_max),
        None if m.has_live_game => format!("{}LIVE{}", color(CYAN), RESET),
        None => format!("{}???{}", color(DIM), RESET),
    };

    let box_color = color(WHITE);

    // Line 1: Team1  W1 ─┐
    rows.push(TeletextRow::BracketLine(format!(
        "{}{}{} {}{}{} {}\u{2500}\u{2510}{}",
        team_color_matchup(m, true),
        t1_padded,
        RESET,
        w1_color,
        m.team1_wins,
        RESET,
        box_color,
        RESET,
    )));

    // Line 2:            ├── Winner
    let spacer = " ".repeat(name_max + 3);
    rows.push(TeletextRow::BracketLine(format!(
        "{}{}\u{251C}\u{2500}\u{2500} {}{}",
        spacer, box_color, RESET, winner_label,
    )));

    // Line 3: Team2  W2 ─┘
    rows.push(TeletextRow::BracketLine(format!(
        "{}{}{} {}{}{} {}\u{2500}\u{2518}{}",
        team_color_matchup(m, false),
        t2_padded,
        RESET,
        w2_color,
        m.team2_wins,
        RESET,
        box_color,
        RESET,
    )));
}

// ---------------------------------------------------------------------------
// Color helpers
// ---------------------------------------------------------------------------

/// Returns ANSI escape to set foreground to the given 256-color code.
fn color(code: u8) -> String {
    format!("\x1b[38;5;{code}m")
}

/// Returns ANSI escape for bold + foreground color.
fn bold_color(code: u8) -> String {
    format!("\x1b[1m\x1b[38;5;{code}m")
}

/// Returns color escape for a team in a matchup.
/// Cyan if the matchup has a live game, otherwise white.
fn team_color_matchup(m: &BracketMatchup, _is_team1: bool) -> String {
    if m.has_live_game {
        color(CYAN)
    } else {
        color(WHITE)
    }
}

/// Returns color escape for stacked layout team display.
fn team_color(m: &BracketMatchup, _is_team1: bool) -> String {
    if m.has_live_game {
        color(CYAN)
    } else {
        color(WHITE)
    }
}

/// Returns ANSI color codes for win counts (team1_wins, team2_wins).
/// A clinching win count (>= req_wins) is displayed in green.
fn win_colors(m: &BracketMatchup) -> (String, String) {
    let w1 = if m.team1_wins >= m.req_wins {
        color(GREEN)
    } else {
        color(YELLOW)
    };
    let w2 = if m.team2_wins >= m.req_wins {
        color(GREEN)
    } else {
        color(YELLOW)
    };
    (w1, w2)
}

/// Appends the "MESTARI: Team" champion label if the final (phase 5) is decided.
fn append_champion(phases: &[BracketPhase], rows: &mut Vec<TeletextRow>) {
    let final_phase = phases.iter().find(|p| p.phase_number == 5);
    if let Some(fp) = final_phase
        && let Some(m) = fp.matchups.first()
        && let Some(ref winner) = m.winner
    {
        rows.push(TeletextRow::BracketLine(String::new()));
        rows.push(TeletextRow::BracketLine(format!(
            "{}MESTARI: {}{}",
            bold_color(GREEN),
            winner,
            RESET,
        )));
    }
}

/// Generates a suffix string indicating the series winner (for stacked layout).
fn winner_suffix(m: &BracketMatchup) -> String {
    if let Some(ref _w) = m.winner {
        String::new() // Winner is implicit from score
    } else {
        String::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_fetcher::models::bracket::{BracketMatchup, BracketPhase, PlayoffBracket};

    fn make_matchup(
        team1: &str,
        team2: &str,
        t1w: u8,
        t2w: u8,
        phase: i32,
        pair: i32,
    ) -> BracketMatchup {
        let req_wins = if phase == 4 { 1 } else { 4 };
        let is_decided = t1w >= req_wins || t2w >= req_wins;
        let winner = if is_decided {
            if t1w >= req_wins {
                Some(team1.to_string())
            } else {
                Some(team2.to_string())
            }
        } else {
            None
        };
        BracketMatchup {
            phase,
            pair,
            serie: 2,
            team1: team1.to_string(),
            team2: team2.to_string(),
            team1_wins: t1w,
            team2_wins: t2w,
            req_wins,
            is_decided,
            has_live_game: false,
            winner,
        }
    }

    fn make_bracket(phases: Vec<BracketPhase>) -> PlayoffBracket {
        PlayoffBracket {
            season: "2025-2026".to_string(),
            phases,
            has_data: true,
        }
    }

    /// Extracts the raw text content from BracketLine rows (for assertion matching).
    fn lines_text(rows: &[TeletextRow]) -> String {
        rows.iter()
            .map(|r| match r {
                TeletextRow::BracketLine(s) => s.clone(),
                _ => String::new(),
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn test_no_data_message() {
        let bracket = PlayoffBracket {
            season: "2025-2026".to_string(),
            phases: vec![],
            has_data: false,
        };
        let rows = render_bracket(&bracket, 80);
        let text = lines_text(&rows);
        assert!(
            text.contains("K\u{00C4}YNNISS\u{00C4}"),
            "Expected 'KÄYNNISSÄ' in output, got: {text}"
        );
    }

    #[test]
    fn test_narrow_terminal_stacked_fallback() {
        let phases = vec![BracketPhase {
            phase_number: 2,
            name: "PUOLIV\u{00C4}LIER\u{00C4}T".to_string(),
            matchups: vec![
                make_matchup("HIFK", "TPS", 3, 1, 2, 1),
                make_matchup("Lukko", "KooKoo", 2, 2, 2, 2),
            ],
        }];
        let bracket = make_bracket(phases);
        let rows = render_bracket(&bracket, 40);
        let text = lines_text(&rows);
        // Stacked layout should contain team names
        assert!(text.contains("HIFK"), "Expected 'HIFK' in stacked output");
        assert!(text.contains("TPS"), "Expected 'TPS' in stacked output");
        assert!(text.contains("Lukko"), "Expected 'Lukko' in stacked output");
        assert!(
            text.contains("KooKoo"),
            "Expected 'KooKoo' in stacked output"
        );
        // Should not contain box-drawing tree chars (those are tree-only)
        // (Stacked uses simple "W1 - W2" format)
        assert!(
            text.contains(" - "),
            "Expected 'W1 - W2' separator in stacked output"
        );
    }

    #[test]
    fn test_tree_contains_box_drawing_chars() {
        let phases = vec![BracketPhase {
            phase_number: 2,
            name: "PUOLIV\u{00C4}LIER\u{00C4}T".to_string(),
            matchups: vec![make_matchup("HIFK", "TPS", 4, 1, 2, 1)],
        }];
        let bracket = make_bracket(phases);
        let rows = render_bracket(&bracket, 80);
        let text = lines_text(&rows);
        // Tree layout uses box-drawing characters
        let has_box_chars = text.contains('\u{2500}')    // ─
            || text.contains('\u{2510}')  // ┐
            || text.contains('\u{2518}')  // ┘
            || text.contains('\u{251C}'); // ├
        assert!(
            has_box_chars,
            "Expected box-drawing characters in tree output, got: {text}"
        );
    }

    #[test]
    fn test_team_name_truncation() {
        assert_eq!(truncate_team_name("Pelicans", 5), "Peli.");
    }

    #[test]
    fn test_team_name_no_truncation_when_fits() {
        assert_eq!(truncate_team_name("HIFK", 7), "HIFK");
    }

    #[test]
    fn test_decided_series_winner_shown() {
        let phases = vec![
            BracketPhase {
                phase_number: 2,
                name: "PUOLIV\u{00C4}LIER\u{00C4}T".to_string(),
                matchups: vec![
                    make_matchup("HIFK", "TPS", 4, 2, 2, 1),
                    make_matchup("Lukko", "KooKoo", 4, 0, 2, 2),
                ],
            },
            BracketPhase {
                phase_number: 3,
                name: "V\u{00C4}LIER\u{00C4}T".to_string(),
                matchups: vec![make_matchup("HIFK", "Lukko", 4, 1, 3, 1)],
            },
        ];
        let bracket = make_bracket(phases);
        let rows = render_bracket(&bracket, 80);
        let text = lines_text(&rows);
        // The decided series should show the winning team name and its score
        assert!(
            text.contains("HIFK"),
            "Expected winning team 'HIFK' in output"
        );
        assert!(text.contains('4'), "Expected winning score '4' in output");
    }

    #[test]
    fn test_truncation_edge_cases() {
        // Exactly at limit
        assert_eq!(truncate_team_name("HIFK", 4), "HIFK");
        // One over
        assert_eq!(truncate_team_name("HIFK!", 4), "HIF.");
        // Empty name
        assert_eq!(truncate_team_name("", 5), "");
        // Max len 0
        assert_eq!(truncate_team_name("HIFK", 0), "");
        // Max len 1
        assert_eq!(truncate_team_name("AB", 1), ".");
    }

    #[test]
    fn test_champion_label_shown() {
        let phases = vec![BracketPhase {
            phase_number: 5,
            name: "FINAALI".to_string(),
            matchups: vec![make_matchup("HIFK", "Lukko", 4, 2, 5, 1)],
        }];
        let bracket = make_bracket(phases);
        let rows = render_bracket(&bracket, 80);
        let text = lines_text(&rows);
        assert!(
            text.contains("MESTARI"),
            "Expected 'MESTARI' label in output"
        );
        assert!(
            text.contains("HIFK"),
            "Expected champion team name in output"
        );
    }

    #[test]
    fn test_reseeding_dynamic_grouping() {
        // After QFs, reseeding means #1 seed plays lowest remaining seed.
        // QF results: HIFK(1) beats TPS(8), Tappara(2) beats KooKoo(7),
        //             Lukko(3) beats Ilves(6), Kärpät(4) beats Pelicans(5)
        // Reseeded SF: HIFK(1) vs Kärpät(4), Tappara(2) vs Lukko(3)
        // This means QF pair 1 (HIFK-TPS) groups with QF pair 4 (Kärpät-Pelicans),
        // NOT with QF pair 2.
        let phases = vec![
            BracketPhase {
                phase_number: 2,
                name: "PUOLIV\u{00C4}LIER\u{00C4}T".to_string(),
                matchups: vec![
                    make_matchup("HIFK", "TPS", 4, 1, 2, 1),
                    make_matchup("Tappara", "KooKoo", 4, 2, 2, 2),
                    make_matchup("Lukko", "Ilves", 4, 3, 2, 3),
                    make_matchup("K\u{00E4}rp\u{00E4}t", "Pelicans", 4, 0, 2, 4),
                ],
            },
            BracketPhase {
                phase_number: 3,
                name: "V\u{00C4}LIER\u{00C4}T".to_string(),
                matchups: vec![
                    // Reseeded: #1 HIFK vs #4 Kärpät
                    make_matchup("HIFK", "K\u{00E4}rp\u{00E4}t", 2, 1, 3, 1),
                    // Reseeded: #2 Tappara vs #3 Lukko
                    make_matchup("Tappara", "Lukko", 3, 2, 3, 2),
                ],
            },
        ];
        let bracket = make_bracket(phases);
        let rows = render_bracket(&bracket, 80);
        let text = lines_text(&rows);

        // Verify both SF matchups and their feeder QF matchups appear
        assert!(text.contains("HIFK"), "Expected HIFK in output");
        assert!(
            text.contains("K\u{00E4}rp\u{00E4}"),
            "Expected Kärpät in output (possibly truncated)"
        );
        assert!(text.contains("Tappara"), "Expected Tappara in output");
        assert!(text.contains("Lukko"), "Expected Lukko in output");
    }
}
