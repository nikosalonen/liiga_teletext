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
///
/// Phases are reordered for pagination: when 1. KIERROS is fully decided,
/// it moves to page 2 so that active/later phases appear first.
fn render_stacked(bracket: &PlayoffBracket, terminal_width: u16) -> Vec<TeletextRow> {
    let mut rows: Vec<TeletextRow> = Vec::new();
    let max_name = (terminal_width as usize).saturating_sub(16).min(12);

    let (first_group, second_group) = split_phases_for_pagination(&bracket.phases);

    render_stacked_group(&first_group, &mut rows, max_name);
    if first_group.iter().any(|p| p.phase_number == 5) {
        append_champion(&bracket.phases, &mut rows);
    }

    if !second_group.is_empty() {
        rows.push(TeletextRow::BracketPageBreak);
        render_stacked_group(&second_group, &mut rows, max_name);
        if second_group.iter().any(|p| p.phase_number == 5) {
            append_champion(&bracket.phases, &mut rows);
        }
    }

    rows
}

/// Renders a group of phases in stacked layout.
fn render_stacked_group(phases: &[&BracketPhase], rows: &mut Vec<TeletextRow>, max_name: usize) {
    for phase in phases {
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
            let (t1_color, t2_color) = matchup_team_colors(m);
            let live_marker = if m.has_live_game {
                format!(" {}\u{25CF}{}", color(CYAN), RESET)
            } else {
                String::new()
            };
            rows.push(TeletextRow::BracketLine(format!(
                "{}{:<width$}{}  {}{}{} - {}{}{}\
                 {}  {}{:<width$}{}",
                t1_color,
                t1,
                RESET,
                w1_color,
                m.team1_wins,
                RESET,
                w2_color,
                m.team2_wins,
                RESET,
                live_marker,
                t2_color,
                t2,
                RESET,
                width = max_name,
            )));
        }
    }
}

// ---------------------------------------------------------------------------
// Tree layout (wide terminals)
// ---------------------------------------------------------------------------

/// Renders bracket as a tree with box-drawing characters.
///
/// Each phase is rendered sequentially with its own header and BO label.
/// Phases are reordered for pagination: when 1. KIERROS is fully decided,
/// it moves to page 2 so that active/later phases appear first.
fn render_tree(bracket: &PlayoffBracket, _terminal_width: u16) -> Vec<TeletextRow> {
    let mut rows: Vec<TeletextRow> = Vec::new();
    let phase_count = bracket.phases.len();
    let name_max = max_team_len(phase_count);

    let (first_group, second_group) = split_phases_for_pagination(&bracket.phases);

    render_tree_group(&first_group, &mut rows, name_max);
    if first_group.iter().any(|p| p.phase_number == 5) {
        append_champion(&bracket.phases, &mut rows);
    }

    if !second_group.is_empty() {
        rows.push(TeletextRow::BracketPageBreak);
        render_tree_group(&second_group, &mut rows, name_max);
        if second_group.iter().any(|p| p.phase_number == 5) {
            append_champion(&bracket.phases, &mut rows);
        }
    }

    rows
}

/// Renders a group of phases in tree layout.
fn render_tree_group(phases: &[&BracketPhase], rows: &mut Vec<TeletextRow>, name_max: usize) {
    for (i, phase) in phases.iter().enumerate() {
        if i > 0 {
            rows.push(TeletextRow::BracketLine(String::new()));
        }

        let bo_label = phase
            .matchups
            .first()
            .map(|m| format!(" {}{}{}", color(DIM), series_format(m.req_wins), RESET))
            .unwrap_or_default();
        rows.push(TeletextRow::BracketLine(format!(
            "{}{}{}{bo_label}",
            color(CYAN),
            phase.name,
            RESET
        )));

        for (j, m) in phase.matchups.iter().enumerate() {
            if j > 0 {
                rows.push(TeletextRow::BracketLine(String::new()));
            }
            render_matchup_tree(m, rows, name_max);
        }
    }
}

/// Formats series length from req_wins, e.g., req_wins=3 → "BO5", req_wins=4 → "BO7", req_wins=1 → "BO1"
fn series_format(req_wins: u8) -> String {
    let games = req_wins as u16 * 2 - 1;
    format!("BO{games}")
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
    let (t1_color, t2_color) = matchup_team_colors(m);

    let t1_padded = format!("{:<width$}", t1, width = name_max);
    let t2_padded = format!("{:<width$}", t2, width = name_max);

    let team1_won = m.winner.as_ref().is_some_and(|w| *w == m.team1);
    let team2_won = m.winner.as_ref().is_some_and(|w| *w == m.team2);

    let decided = team1_won || team2_won;
    let box_color = if decided { color(GREEN) } else { color(WHITE) };

    let winner_label = match &m.winner {
        Some(w) => format!(
            "{}{}{}",
            color(GREEN),
            truncate_team_name(w, name_max),
            RESET
        ),
        None if m.has_live_game => format!("{}LIVE{}", color(CYAN), RESET),
        None => format!("{}???{}", color(DIM), RESET),
    };

    // Line 1: Team1  W1 ─┐  (no bracket on loser side when team2 won)
    let top_bracket = if team2_won {
        "  ".to_string()
    } else {
        format!("{}\u{2500}\u{2510}{}", box_color, RESET)
    };
    rows.push(TeletextRow::BracketLine(format!(
        "{}{}{} {}{}{} {}",
        t1_color, t1_padded, RESET, w1_color, m.team1_wins, RESET, top_bracket,
    )));

    // Line 2: connector points toward winner
    //   ├── (undecided)  └── (team1 won)  ┌── (team2 won)
    let connector = if team1_won {
        '\u{2514}' // └
    } else if team2_won {
        '\u{250C}' // ┌
    } else {
        '\u{251C}' // ├
    };
    let spacer = " ".repeat(name_max + 4);
    rows.push(TeletextRow::BracketLine(format!(
        "{}{}{}\u{2500}\u{2500} {}{}",
        spacer, box_color, connector, RESET, winner_label,
    )));

    // Line 3: Team2  W2 ─┘  (no bracket on loser side when team1 won)
    let bottom_bracket = if team1_won {
        "  ".to_string()
    } else {
        format!("{}\u{2500}\u{2518}{}", box_color, RESET)
    };
    rows.push(TeletextRow::BracketLine(format!(
        "{}{}{} {}{}{} {}",
        t2_color, t2_padded, RESET, w2_color, m.team2_wins, RESET, bottom_bracket,
    )));
}

// ---------------------------------------------------------------------------
// Phase pagination
// ---------------------------------------------------------------------------

/// Splits bracket phases into two groups for pagination.
///
/// When 1. KIERROS (phase 1) is fully decided, moves it to the second group
/// so that active/later phases appear on page 1. When phase 1 is still active,
/// it stays on page 1 and later phases go to page 2.
fn split_phases_for_pagination(
    phases: &[BracketPhase],
) -> (Vec<&BracketPhase>, Vec<&BracketPhase>) {
    let phase1 = phases.iter().find(|p| p.phase_number == 1);

    if let Some(p1) = phase1
        && phases.len() > 1
    {
        let all_decided = !p1.matchups.is_empty() && p1.matchups.iter().all(|m| m.is_decided);

        if all_decided {
            // Phase 1 complete: later phases first (page 1), phase 1 on page 2
            let first: Vec<_> = phases.iter().filter(|p| p.phase_number != 1).collect();
            let second: Vec<_> = phases.iter().filter(|p| p.phase_number == 1).collect();
            (first, second)
        } else {
            // Phase 1 active: phase 1 on page 1, later phases on page 2
            let first: Vec<_> = phases.iter().filter(|p| p.phase_number == 1).collect();
            let second: Vec<_> = phases.iter().filter(|p| p.phase_number != 1).collect();
            (first, second)
        }
    } else {
        // Single phase or no phase 1: no split needed
        (phases.iter().collect(), Vec::new())
    }
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
fn team_color(m: &BracketMatchup) -> String {
    if m.has_live_game {
        color(CYAN)
    } else {
        color(WHITE)
    }
}

/// Returns (team1_color, team2_color) for a matchup.
/// Winner is green, loser is dim, undecided uses `team_color`.
fn matchup_team_colors(m: &BracketMatchup) -> (String, String) {
    match &m.winner {
        Some(w) if *w == m.team1 => (color(GREEN), color(DIM)),
        Some(_) => (color(DIM), color(GREEN)),
        None => (team_color(m), team_color(m)),
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
    fn test_qf_and_sf_rendered_with_separate_headers() {
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
                    make_matchup("HIFK", "K\u{00E4}rp\u{00E4}t", 2, 1, 3, 1),
                    make_matchup("Tappara", "Lukko", 3, 2, 3, 2),
                ],
            },
        ];
        let bracket = make_bracket(phases);
        let rows = render_bracket(&bracket, 80);
        let text = lines_text(&rows);

        // Both phases get their own header
        assert!(
            text.contains("PUOLIV\u{00C4}LIER\u{00C4}T"),
            "Expected QF header in output"
        );
        assert!(
            text.contains("V\u{00C4}LIER\u{00C4}T"),
            "Expected SF header in output"
        );
        // All teams present
        assert!(text.contains("HIFK"), "Expected HIFK in output");
        assert!(
            text.contains("K\u{00E4}rp\u{00E4}"),
            "Expected K\u{00E4}rp\u{00E4}t in output (possibly truncated)"
        );
        assert!(text.contains("Tappara"), "Expected Tappara in output");
        assert!(text.contains("Lukko"), "Expected Lukko in output");
        // QF header appears before SF header
        let qf_pos = text.find("PUOLIV\u{00C4}LIER\u{00C4}T").unwrap();
        let sf_pos = text.find("V\u{00C4}LIER\u{00C4}T").unwrap();
        assert!(qf_pos < sf_pos, "QF header should appear before SF header");
    }

    #[test]
    fn test_multi_early_phases_separate_headers() {
        // Regression test for #114: phases 1, 2, and 3 must each get
        // their own header with correct BO label.
        let mut r1_matchup1 = make_matchup("Lukko", "HPK", 3, 1, 1, 1);
        r1_matchup1.req_wins = 3; // BO5
        let mut r1_matchup2 = make_matchup("JYP", "Pelicans", 1, 3, 1, 2);
        r1_matchup2.req_wins = 3;
        let phases = vec![
            BracketPhase {
                phase_number: 1,
                name: "1. KIERROS".to_string(),
                matchups: vec![r1_matchup1, r1_matchup2],
            },
            BracketPhase {
                phase_number: 2,
                name: "PUOLIV\u{00C4}LIER\u{00C4}T".to_string(),
                matchups: vec![
                    make_matchup("HIFK", "Lukko", 2, 1, 2, 1),
                    make_matchup("Tappara", "Pelicans", 0, 0, 2, 2),
                ],
            },
            BracketPhase {
                phase_number: 3,
                name: "V\u{00C4}LIER\u{00C4}T".to_string(),
                matchups: vec![make_matchup("???", "???", 0, 0, 3, 1)],
            },
        ];
        let bracket = make_bracket(phases);
        let rows = render_bracket(&bracket, 80);
        let text = lines_text(&rows);

        // Each phase has its own header
        assert!(text.contains("1. KIERROS"), "Expected 1. KIERROS header");
        assert!(
            text.contains("PUOLIV\u{00C4}LIER\u{00C4}T"),
            "Expected QF header"
        );
        assert!(
            text.contains("V\u{00C4}LIER\u{00C4}T"),
            "Expected SF header"
        );
        // Correct BO labels
        assert!(text.contains("BO5"), "Expected BO5 for 1. KIERROS");
        assert!(text.contains("BO7"), "Expected BO7 for QF/SF");
        // Phase ordering
        let r1_pos = text.find("1. KIERROS").unwrap();
        let qf_pos = text.find("PUOLIV\u{00C4}LIER\u{00C4}T").unwrap();
        let sf_pos = text.find("V\u{00C4}LIER\u{00C4}T").unwrap();
        assert!(r1_pos < qf_pos, "1. KIERROS should appear before QF");
        assert!(qf_pos < sf_pos, "QF should appear before SF");
        // All teams present
        assert!(text.contains("Lukko"), "Missing Lukko");
        assert!(text.contains("HPK"), "Missing HPK");
        assert!(text.contains("HIFK"), "Missing HIFK");
        assert!(text.contains("Tappara"), "Missing Tappara");
    }

    #[test]
    fn test_four_matchups_single_phase_all_visible() {
        let phases = vec![BracketPhase {
            phase_number: 1,
            name: "1. KIERROS".to_string(),
            matchups: vec![
                make_matchup("Lukko", "HPK", 1, 0, 1, 1),
                make_matchup("JYP", "Pelicans", 0, 1, 1, 2),
                make_matchup("KalPa", "HIFK", 1, 0, 1, 3),
                make_matchup("Assat", "K-Espoo", 0, 0, 1, 4),
            ],
        }];
        let bracket = make_bracket(phases);
        let rows = render_bracket(&bracket, 80);
        let text = lines_text(&rows);

        // All 4 matchups must appear
        assert!(text.contains("Lukko"), "Missing Lukko");
        assert!(text.contains("HPK"), "Missing HPK");
        assert!(text.contains("JYP"), "Missing JYP");
        assert!(text.contains("Pelicans"), "Missing Pelicans");
        assert!(text.contains("KalPa"), "Missing KalPa");
        assert!(text.contains("HIFK"), "Missing HIFK");
        assert!(text.contains("Assat"), "Missing Assat");
        assert!(text.contains("K-Espoo"), "Missing K-Espoo");

        // Count how many matchup blocks (each has ─┐)
        let matchup_count = rows
            .iter()
            .filter(|r| match r {
                TeletextRow::BracketLine(s) => s.contains('\u{2510}'), // ┐
                _ => false,
            })
            .count();
        assert_eq!(
            matchup_count, 4,
            "Expected 4 matchup blocks, got {matchup_count}"
        );
    }

    /// Helper to create a decided matchup (is_decided=true, winner set).
    fn make_decided_matchup(
        team1: &str,
        team2: &str,
        t1w: u8,
        t2w: u8,
        phase: i32,
        pair: i32,
        req_wins: u8,
    ) -> BracketMatchup {
        let is_decided = t1w >= req_wins || t2w >= req_wins;
        let winner = if t1w >= req_wins {
            Some(team1.to_string())
        } else if t2w >= req_wins {
            Some(team2.to_string())
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

    #[test]
    fn test_decided_phase1_moves_to_page2() {
        // When all 1. KIERROS matchups are decided, later phases come first
        let phases = vec![
            BracketPhase {
                phase_number: 1,
                name: "1. KIERROS".to_string(),
                matchups: vec![
                    make_decided_matchup("Lukko", "HPK", 3, 1, 1, 1, 3),
                    make_decided_matchup("JYP", "Pelicans", 1, 3, 1, 2, 3),
                ],
            },
            BracketPhase {
                phase_number: 2,
                name: "PUOLIV\u{00C4}LIER\u{00C4}T".to_string(),
                matchups: vec![
                    make_matchup("HIFK", "Lukko", 2, 1, 2, 1),
                    make_matchup("Tappara", "Pelicans", 0, 0, 2, 2),
                ],
            },
        ];
        let bracket = make_bracket(phases);
        let rows = render_bracket(&bracket, 80);
        let text = lines_text(&rows);

        // QF should appear BEFORE 1. KIERROS (because R1 is decided → page 2)
        let qf_pos = text.find("PUOLIV\u{00C4}LIER\u{00C4}T").unwrap();
        let r1_pos = text.find("1. KIERROS").unwrap();
        assert!(qf_pos < r1_pos, "QF should come before decided 1. KIERROS");

        // Page break marker should be present between them
        assert!(
            rows.iter()
                .any(|r| matches!(r, TeletextRow::BracketPageBreak)),
            "Expected a BracketPageBreak between phase groups"
        );
    }

    #[test]
    fn test_active_phase1_stays_on_page1() {
        // When 1. KIERROS has undecided matchups, it stays first
        let phases = vec![
            BracketPhase {
                phase_number: 1,
                name: "1. KIERROS".to_string(),
                matchups: vec![
                    make_decided_matchup("Lukko", "HPK", 3, 1, 1, 1, 3),
                    make_decided_matchup("JYP", "Pelicans", 1, 2, 1, 2, 3), // not decided
                ],
            },
            BracketPhase {
                phase_number: 2,
                name: "PUOLIV\u{00C4}LIER\u{00C4}T".to_string(),
                matchups: vec![make_matchup("HIFK", "Lukko", 0, 0, 2, 1)],
            },
        ];
        let bracket = make_bracket(phases);
        let rows = render_bracket(&bracket, 80);
        let text = lines_text(&rows);

        // 1. KIERROS should appear BEFORE QF (it's still active → page 1)
        let r1_pos = text.find("1. KIERROS").unwrap();
        let qf_pos = text.find("PUOLIV\u{00C4}LIER\u{00C4}T").unwrap();
        assert!(r1_pos < qf_pos, "Active 1. KIERROS should come before QF");

        // Page break should still be present
        assert!(
            rows.iter()
                .any(|r| matches!(r, TeletextRow::BracketPageBreak)),
            "Expected a BracketPageBreak between phase groups"
        );
    }

    #[test]
    fn test_no_phase1_no_page_break() {
        // When there's no phase 1, no page break is inserted
        let phases = vec![
            BracketPhase {
                phase_number: 2,
                name: "PUOLIV\u{00C4}LIER\u{00C4}T".to_string(),
                matchups: vec![make_matchup("HIFK", "TPS", 4, 1, 2, 1)],
            },
            BracketPhase {
                phase_number: 3,
                name: "V\u{00C4}LIER\u{00C4}T".to_string(),
                matchups: vec![make_matchup("HIFK", "Lukko", 0, 0, 3, 1)],
            },
        ];
        let bracket = make_bracket(phases);
        let rows = render_bracket(&bracket, 80);

        assert!(
            !rows
                .iter()
                .any(|r| matches!(r, TeletextRow::BracketPageBreak)),
            "No page break expected when there's no phase 1"
        );
    }

    #[test]
    fn test_single_phase_no_page_break() {
        // Single phase: no page break regardless of decided status
        let phases = vec![BracketPhase {
            phase_number: 1,
            name: "1. KIERROS".to_string(),
            matchups: vec![make_decided_matchup("Lukko", "HPK", 3, 0, 1, 1, 3)],
        }];
        let bracket = make_bracket(phases);
        let rows = render_bracket(&bracket, 80);

        assert!(
            !rows
                .iter()
                .any(|r| matches!(r, TeletextRow::BracketPageBreak)),
            "No page break expected for single phase"
        );
    }
}
