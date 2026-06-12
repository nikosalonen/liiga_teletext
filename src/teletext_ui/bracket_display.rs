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
/// Prefers the full-path layout (all rounds side by side with connectors)
/// when the terminal is large enough, falling back to the sequential tree
/// layout and finally the stacked layout on narrow terminals.
pub fn render_bracket(
    bracket: &PlayoffBracket,
    terminal_width: u16,
    terminal_height: u16,
) -> Vec<TeletextRow> {
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

    if let Some(rows) = render_full_path(bracket, terminal_width, terminal_height) {
        return rows;
    }

    let phase_count = bracket.phases.len();
    if terminal_width < min_tree_width(phase_count) {
        render_stacked(bracket, terminal_width)
    } else {
        render_tree(bracket, terminal_width)
    }
}

// ---------------------------------------------------------------------------
// Full-path layout (large terminals): all rounds side by side
// ---------------------------------------------------------------------------

/// Minimum terminal height for the full-path layout. The complete canvas
/// (headers + four quarterfinal blocks + bronze) needs 19 content rows,
/// which corresponds to a 24-line terminal.
const FULL_PATH_MIN_HEIGHT: u16 = 24;

/// A character canvas with per-cell 256-color codes, used for 2D bracket
/// drawing before serializing into `BracketLine` rows.
struct Canvas {
    cells: Vec<Vec<(char, u8)>>,
}

impl Canvas {
    fn new() -> Self {
        Self { cells: Vec::new() }
    }

    fn put(&mut self, x: usize, y: usize, ch: char, code: u8) {
        while self.cells.len() <= y {
            self.cells.push(Vec::new());
        }
        let row = &mut self.cells[y];
        while row.len() <= x {
            row.push((' ', WHITE));
        }
        row[x] = (ch, code);
    }

    fn put_str(&mut self, x: usize, y: usize, text: &str, code: u8) {
        for (i, ch) in text.chars().enumerate() {
            self.put(x + i, y, ch, code);
        }
    }

    /// Serializes the canvas into colored `BracketLine` rows, trimming
    /// trailing whitespace and emitting color escapes only on changes.
    fn into_rows(self) -> Vec<TeletextRow> {
        self.cells
            .into_iter()
            .map(|row| {
                let end = row
                    .iter()
                    .rposition(|(ch, _)| *ch != ' ')
                    .map_or(0, |i| i + 1);
                let mut line = String::new();
                let mut current: Option<u8> = None;
                for (ch, code) in &row[..end] {
                    if *ch != ' ' && current != Some(*code) {
                        line.push_str(&color(*code));
                        current = Some(*code);
                    }
                    line.push(*ch);
                }
                if current.is_some() {
                    line.push_str(RESET);
                }
                TeletextRow::BracketLine(line)
            })
            .collect()
    }
}

/// Display data for one side of a matchup slot.
struct TeamLabel {
    text: String,
    color: u8,
    wins: Option<(u8, u8)>, // (win count, color)
}

/// A matchup positioned on the canvas. `m` is None for rounds that have not
/// been scheduled yet; their labels are then derived from feeder slots.
struct PathSlot<'a> {
    m: Option<&'a BracketMatchup>,
    t1_row: usize,
    t2_row: usize,
    top: TeamLabel,
    bottom: TeamLabel,
}

impl PathSlot<'_> {
    fn mid(&self) -> usize {
        self.t1_row.midpoint(self.t2_row)
    }
}

/// Builds the label shown for an advancing team slot: the feeder's winner
/// when decided, "LIVE" while the feeder series is running, "???" otherwise.
fn advancing_label(feeder: Option<&PathSlot<'_>>, name_max: usize) -> TeamLabel {
    match feeder.and_then(|s| s.m) {
        Some(m) => match &m.winner {
            Some(w) => TeamLabel {
                text: truncate_team_name(w, name_max),
                color: GREEN,
                wins: None,
            },
            None if m.has_live_game => TeamLabel {
                text: "LIVE".to_string(),
                color: CYAN,
                wins: None,
            },
            None => unknown_label(),
        },
        None => unknown_label(),
    }
}

fn unknown_label() -> TeamLabel {
    TeamLabel {
        text: "???".to_string(),
        color: DIM,
        wins: None,
    }
}

/// Builds (top, bottom) labels for a slot backed by real matchup data.
/// `swapped` displays team2 on top (used when feeder geometry demands it).
fn matchup_labels(m: &BracketMatchup, name_max: usize, swapped: bool) -> (TeamLabel, TeamLabel) {
    let (c1, c2) = matchup_team_color_codes(m);
    let (w1, w2) = win_color_codes(m);
    let l1 = TeamLabel {
        text: truncate_team_name(&m.team1, name_max),
        color: c1,
        wins: Some((m.team1_wins, w1)),
    };
    let l2 = TeamLabel {
        text: truncate_team_name(&m.team2, name_max),
        color: c2,
        wins: Some((m.team2_wins, w2)),
    };
    if swapped { (l2, l1) } else { (l1, l2) }
}

/// Builds the slots of a later round using classic positional bracket
/// geometry: slot k sits between feeder slots 2k and 2k+1. Matchups are
/// assigned to slots (and oriented top/bottom) so that as many teams as
/// possible line up with the feeder that produced them — with Liiga's
/// re-seeding a perfect assignment is not always possible, in which case
/// the closest match is used.
fn build_round<'a>(
    prev: &[PathSlot<'a>],
    phase: Option<&'a BracketPhase>,
    count: usize,
    name_max: usize,
) -> Vec<PathSlot<'a>> {
    let mut remaining: Vec<&'a BracketMatchup> = phase
        .map(|p| p.matchups.iter().collect())
        .unwrap_or_default();

    (0..count)
        .map(|k| {
            let t1_row = prev[2 * k].mid();
            let t2_row = prev[2 * k + 1].mid();
            let w_top = prev[2 * k].m.and_then(|m| m.winner.as_deref());
            let w_bot = prev[2 * k + 1].m.and_then(|m| m.winner.as_deref());

            // Score each candidate matchup in both orientations: one point
            // per team that aligns with the feeder slot it came from
            let alignment = |m: &BracketMatchup| -> (u32, bool) {
                let team = |t: &str, w: Option<&str>| u32::from(w == Some(t));
                let no_swap = team(&m.team1, w_top) + team(&m.team2, w_bot);
                let swap = team(&m.team2, w_top) + team(&m.team1, w_bot);
                if swap > no_swap {
                    (swap, true)
                } else {
                    (no_swap, false)
                }
            };

            let best = remaining
                .iter()
                .enumerate()
                .max_by_key(|(_, m)| alignment(m).0)
                .map(|(i, _)| i);

            match best {
                Some(i) => {
                    let m = remaining.remove(i);
                    let (_, swapped) = alignment(m);
                    let (top, bottom) = matchup_labels(m, name_max, swapped);
                    PathSlot {
                        m: Some(m),
                        t1_row,
                        t2_row,
                        top,
                        bottom,
                    }
                }
                None => PathSlot {
                    m: None,
                    t1_row,
                    t2_row,
                    top: advancing_label(prev.get(2 * k), name_max),
                    bottom: advancing_label(prev.get(2 * k + 1), name_max),
                },
            }
        })
        .collect()
}

/// Draws a matchup slot at column `x`: team labels, win counts, and the
/// bracket connector pointing toward the winner at the slot's mid row.
fn draw_slot(canvas: &mut Canvas, x: usize, slot: &PathSlot<'_>, name_max: usize) {
    let mid = slot.mid();
    let bar_x = x + name_max + 4;

    let top_won = slot.top.wins.is_some_and(|(w, _)| {
        slot.m
            .is_some_and(|m| m.winner.is_some() && w >= m.req_wins)
    });
    let bottom_won = slot.bottom.wins.is_some_and(|(w, _)| {
        slot.m
            .is_some_and(|m| m.winner.is_some() && w >= m.req_wins)
    });
    let decided = top_won || bottom_won;
    let box_code = if decided { GREEN } else { WHITE };

    // Team rows: name, win count, and the corner bracket on the active side
    for (label, row, corner, won_opposite) in [
        (&slot.top, slot.t1_row, '\u{2510}', bottom_won),
        (&slot.bottom, slot.t2_row, '\u{2518}', top_won),
    ] {
        canvas.put_str(x, row, &label.text, label.color);
        if let Some((wins, win_color)) = label.wins {
            canvas.put_str(x + name_max + 1, row, &wins.to_string(), win_color);
        }
        if !won_opposite {
            canvas.put(bar_x - 1, row, '\u{2500}', box_code);
            canvas.put(bar_x, row, corner, box_code);
        }
    }

    // Vertical bar segments between the corners and the connector arm.
    // Only the winner's side is drawn for decided series.
    if !bottom_won {
        for y in (slot.t1_row + 1)..mid {
            canvas.put(bar_x, y, '\u{2502}', box_code);
        }
    }
    if !top_won {
        for y in (mid + 1)..slot.t2_row {
            canvas.put(bar_x, y, '\u{2502}', box_code);
        }
    }

    // Connector arm at the mid row, leading into the next round's slot
    let connector = if top_won {
        '\u{2514}' // └
    } else if bottom_won {
        '\u{250C}' // ┌
    } else {
        '\u{251C}' // ├
    };
    canvas.put(bar_x, mid, connector, box_code);
    canvas.put_str(bar_x + 1, mid, "\u{2500}\u{2500}", box_code);
}

/// Renders the bracket with every round side by side, connectors routed to
/// the actual feeder matchups, and rounds that are not yet scheduled shown
/// as "???" placeholders so the whole path to the championship is visible.
///
/// Returns None when the terminal is too small or the bracket does not
/// match the expected Liiga playoff structure (the caller then falls back
/// to the sequential layouts).
fn render_full_path(
    bracket: &PlayoffBracket,
    terminal_width: u16,
    terminal_height: u16,
) -> Option<Vec<TeletextRow>> {
    if terminal_height < FULL_PATH_MIN_HEIGHT {
        return None;
    }

    let phase = |n: i32| bracket.phases.iter().find(|p| p.phase_number == n);
    let (r1, qf, sf, fin, bronze) = (phase(1), phase(2), phase(3), phase(5), phase(4));

    // The path layout assumes the regular Liiga structure: an optional
    // first round feeding the quarterfinals, then 4 -> 2 -> 1 matchups.
    if qf.is_none() && sf.is_none() && fin.is_none() {
        return None;
    }
    if r1.is_some_and(|p| p.matchups.len() > 4)
        || qf.is_some_and(|p| p.matchups.len() > 4)
        || sf.is_some_and(|p| p.matchups.len() > 2)
        || fin.is_some_and(|p| p.matchups.len() > 1)
    {
        return None;
    }

    let has_r1 = r1.is_some_and(|p| !p.matchups.is_empty());
    let cols = 3 + usize::from(has_r1);

    // Column pitch is name + space + win digit + space + "─┐" + "├── " arm
    let usable = (terminal_width as usize).saturating_sub(4);
    let name_max = usable.saturating_sub(8 * cols) / (cols + 1);
    let name_max = name_max.min(12);
    if name_max < 7 {
        return None;
    }
    let pitch = name_max + 8;

    let base_y = 2; // Row 0: phase headers, row 1: blank
    let mut canvas = Canvas::new();

    // --- Quarterfinals: the positional base grid (4 blocks of 3 rows) ---
    let qf_slots: Vec<PathSlot<'_>> = (0..4)
        .map(|i| {
            let m = qf.and_then(|p| p.matchups.get(i));
            let (t1_row, t2_row) = (base_y + i * 4, base_y + i * 4 + 2);
            let (top, bottom) = match m {
                Some(m) => matchup_labels(m, name_max, false),
                None => (unknown_label(), unknown_label()),
            };
            PathSlot {
                m,
                t1_row,
                t2_row,
                top,
                bottom,
            }
        })
        .collect();

    let sf_slots = build_round(&qf_slots, sf, 2, name_max);
    let fin_slots = build_round(&sf_slots, fin, 1, name_max);

    // --- First round: anchored to the quarterfinal slot its winner occupies ---
    let r1_slots: Vec<PathSlot<'_>> = if let Some(r1_phase) = r1.filter(|_| has_r1) {
        let r1_matchups = &r1_phase.matchups;
        let mut anchored: Vec<usize> = Vec::new();
        for (i, m) in r1_matchups.iter().enumerate() {
            let anchor = m.winner.as_deref().and_then(|w| {
                qf_slots.iter().find_map(|s| {
                    let qm = s.m?;
                    if qm.team1 == w {
                        Some(s.t1_row)
                    } else if qm.team2 == w {
                        Some(s.t2_row)
                    } else {
                        None
                    }
                })
            });
            anchored.push(anchor.unwrap_or(base_y + i * 4 + 1));
        }
        // Anchors too close together would overlap; fall back to stacking
        let mut sorted = anchored.clone();
        sorted.sort_unstable();
        if sorted.windows(2).any(|w| w[1] - w[0] < 3) {
            anchored = (0..r1_matchups.len()).map(|i| base_y + i * 4 + 1).collect();
        }
        r1_matchups
            .iter()
            .zip(anchored)
            .map(|(m, mid)| {
                let (top, bottom) = matchup_labels(m, name_max, false);
                PathSlot {
                    m: Some(m),
                    t1_row: mid - 1,
                    t2_row: mid + 1,
                    top,
                    bottom,
                }
            })
            .collect()
    } else {
        Vec::new()
    };

    // --- Draw phase headers and all columns left to right ---
    let mut col = 0;
    let mut draw_column = |canvas: &mut Canvas, slots: &[PathSlot<'_>], header: &str| {
        let x = col * pitch;
        canvas.put_str(x, 0, &truncate_team_name(header, pitch - 1), CYAN);
        for slot in slots {
            draw_slot(canvas, x, slot, name_max);
        }
        col += 1;
        x
    };

    if has_r1 {
        draw_column(&mut canvas, &r1_slots, "1. KIERROS");
    }
    draw_column(&mut canvas, &qf_slots, "PUOLIV\u{00C4}LIER\u{00C4}T");
    draw_column(&mut canvas, &sf_slots, "V\u{00C4}LIER\u{00C4}T");
    let fin_x = draw_column(&mut canvas, &fin_slots, "FINAALI");

    // --- Champion label after the final's connector arm ---
    let champion = advancing_label(fin_slots.first(), name_max);
    let champion_text = match fin_slots
        .first()
        .and_then(|s| s.m)
        .and_then(|m| m.winner.as_ref())
    {
        Some(_) => format!("\u{2605} {}", champion.text),
        None => champion.text.clone(),
    };
    if let Some(f) = fin_slots.first() {
        canvas.put_str(
            fin_x + name_max + 8,
            f.mid(),
            &champion_text,
            champion.color,
        );
    }

    // --- Bronze game in the free space under the final column ---
    if let Some(m) = bronze.and_then(|p| p.matchups.first()) {
        let header_y = base_y + 13;
        canvas.put_str(
            fin_x,
            header_y,
            &truncate_team_name("PRONSSIOTTELU", pitch - 1),
            CYAN,
        );
        let (top, bottom) = matchup_labels(m, name_max, false);
        let slot = PathSlot {
            m: Some(m),
            t1_row: header_y + 1,
            t2_row: header_y + 3,
            top,
            bottom,
        };
        draw_slot(&mut canvas, fin_x, &slot, name_max);
        let label = advancing_label(Some(&slot), name_max);
        canvas.put_str(fin_x + name_max + 8, slot.mid(), &label.text, label.color);
    }

    Some(canvas.into_rows())
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

/// Returns the 256-color code for a team in a matchup.
/// Cyan if the matchup has a live game, otherwise white.
fn team_color_code(m: &BracketMatchup) -> u8 {
    if m.has_live_game { CYAN } else { WHITE }
}

/// Returns (team1, team2) color codes for a matchup.
/// Winner is green, loser is dim, undecided uses `team_color_code`.
fn matchup_team_color_codes(m: &BracketMatchup) -> (u8, u8) {
    match &m.winner {
        Some(w) if *w == m.team1 => (GREEN, DIM),
        Some(_) => (DIM, GREEN),
        None => (team_color_code(m), team_color_code(m)),
    }
}

/// Returns (team1_wins, team2_wins) color codes for a matchup.
/// A clinching win count (>= req_wins) is displayed in green.
fn win_color_codes(m: &BracketMatchup) -> (u8, u8) {
    let w1 = if m.team1_wins >= m.req_wins {
        GREEN
    } else {
        YELLOW
    };
    let w2 = if m.team2_wins >= m.req_wins {
        GREEN
    } else {
        YELLOW
    };
    (w1, w2)
}

/// Returns (team1_color, team2_color) ANSI escapes for a matchup.
fn matchup_team_colors(m: &BracketMatchup) -> (String, String) {
    let (c1, c2) = matchup_team_color_codes(m);
    (color(c1), color(c2))
}

/// Returns ANSI color escapes for win counts (team1_wins, team2_wins).
fn win_colors(m: &BracketMatchup) -> (String, String) {
    let (c1, c2) = win_color_codes(m);
    (color(c1), color(c2))
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
        let rows = render_bracket(&bracket, 80, 23);
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
        let rows = render_bracket(&bracket, 40, 23);
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
        let rows = render_bracket(&bracket, 80, 23);
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
        let rows = render_bracket(&bracket, 80, 23);
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
        let rows = render_bracket(&bracket, 80, 23);
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
        let rows = render_bracket(&bracket, 80, 23);
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
        let rows = render_bracket(&bracket, 80, 23);
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
        let rows = render_bracket(&bracket, 80, 23);
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
        let rows = render_bracket(&bracket, 80, 23);
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
        let rows = render_bracket(&bracket, 80, 23);
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
        let rows = render_bracket(&bracket, 80, 23);

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
        let rows = render_bracket(&bracket, 80, 23);

        assert!(
            !rows
                .iter()
                .any(|r| matches!(r, TeletextRow::BracketPageBreak)),
            "No page break expected for single phase"
        );
    }

    /// Strips ANSI escapes from rendered rows for layout assertions.
    fn plain_lines(rows: &[TeletextRow]) -> Vec<String> {
        rows.iter()
            .map(|r| match r {
                TeletextRow::BracketLine(s) => {
                    let mut out = String::new();
                    let mut chars = s.chars();
                    while let Some(c) = chars.next() {
                        if c == '\x1b' {
                            for n in chars.by_ref() {
                                if n.is_ascii_alphabetic() {
                                    break;
                                }
                            }
                        } else {
                            out.push(c);
                        }
                    }
                    out
                }
                _ => String::new(),
            })
            .collect()
    }

    fn full_playoffs_bracket() -> PlayoffBracket {
        make_bracket(vec![
            BracketPhase {
                phase_number: 1,
                name: "1. KIERROS".to_string(),
                matchups: vec![
                    {
                        let mut m = make_matchup("Jukurit", "Sport", 2, 0, 1, 1);
                        m.req_wins = 2;
                        m.is_decided = true;
                        m.winner = Some("Jukurit".to_string());
                        m
                    },
                    {
                        let mut m = make_matchup("KooKoo", "Ässät", 2, 1, 1, 2);
                        m.req_wins = 2;
                        m.is_decided = true;
                        m.winner = Some("KooKoo".to_string());
                        m
                    },
                ],
            },
            BracketPhase {
                phase_number: 2,
                name: "PUOLIVÄLIERÄT".to_string(),
                matchups: vec![
                    make_matchup("Tappara", "KooKoo", 4, 1, 2, 1),
                    make_matchup("Kärpät", "Jukurit", 4, 2, 2, 2),
                    make_matchup("TPS", "Ilves", 4, 3, 2, 3),
                    make_matchup("Pelicans", "Lukko", 4, 0, 2, 4),
                ],
            },
            BracketPhase {
                phase_number: 3,
                name: "VÄLIERÄT".to_string(),
                matchups: vec![
                    make_matchup("Tappara", "Pelicans", 4, 2, 3, 1),
                    make_matchup("Kärpät", "TPS", 2, 4, 3, 2),
                ],
            },
            BracketPhase {
                phase_number: 4,
                name: "PRONSSIOTTELU".to_string(),
                matchups: vec![make_matchup("Pelicans", "Kärpät", 1, 0, 4, 1)],
            },
            BracketPhase {
                phase_number: 5,
                name: "FINAALI".to_string(),
                matchups: vec![make_matchup("Tappara", "TPS", 4, 2, 5, 1)],
            },
        ])
    }

    #[test]
    fn test_full_path_layout_on_large_terminal() {
        let bracket = full_playoffs_bracket();
        let rows = render_bracket(&bracket, 80, 30);
        let lines = plain_lines(&rows);

        // All path phase headers share the first row
        assert!(lines[0].contains("1. KIERROS"));
        assert!(lines[0].contains("PUOLIVÄLIERÄT"));
        assert!(lines[0].contains("VÄLIERÄT"));
        assert!(lines[0].contains("FINAALI"));

        let text = lines.join("\n");
        // Champion marked after the final
        assert!(text.contains("★ Tappara"));
        // Bronze game is drawn on the same canvas
        assert!(text.contains("PRONSSIOTTELU"));
        // No forced page break: the whole path is one page
        assert!(
            !rows
                .iter()
                .any(|r| matches!(r, TeletextRow::BracketPageBreak))
        );
        // Nothing exceeds the terminal width
        assert!(lines.iter().all(|l| l.chars().count() <= 80));
    }

    #[test]
    fn test_full_path_visual_dump() {
        let bracket = full_playoffs_bracket();
        let rows = render_bracket(&bracket, 80, 30);
        for line in plain_lines(&rows) {
            println!("{line}");
        }
    }

    #[test]
    fn test_full_path_falls_back_on_short_terminal() {
        let bracket = full_playoffs_bracket();
        let rows = render_bracket(&bracket, 80, 23);
        let lines = plain_lines(&rows);
        // Sequential layout: headers are on separate rows
        assert!(!lines[0].contains("FINAALI") || !lines[0].contains("PUOLIVÄLIERÄT"));
    }

    #[test]
    fn test_full_path_synthesizes_unscheduled_rounds() {
        // Only quarterfinals exist (ongoing): SF and final shown as placeholders
        let bracket = make_bracket(vec![BracketPhase {
            phase_number: 2,
            name: "PUOLIVÄLIERÄT".to_string(),
            matchups: vec![
                make_matchup("Tappara", "KooKoo", 2, 1, 2, 1),
                make_matchup("Kärpät", "Jukurit", 1, 2, 2, 2),
                make_matchup("TPS", "Ilves", 3, 3, 2, 3),
                make_matchup("Pelicans", "Lukko", 0, 0, 2, 4),
            ],
        }]);
        let rows = render_bracket(&bracket, 80, 30);
        let lines = plain_lines(&rows);

        assert!(lines[0].contains("VÄLIERÄT"));
        assert!(lines[0].contains("FINAALI"));
        let text = lines.join("\n");
        assert!(text.contains("???"));
    }
}
