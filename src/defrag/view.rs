//! Drawing the cluster map. Pure: takes the map, a label and a size, returns the screen as a
//! string. Nothing here touches a filesystem or a terminal, so every frame can be snapshot
//! tested.

use super::model::Map;

/// The smallest terminal `bios defrag` runs in, same floor as `bios setup`.
pub const MIN_WIDTH: usize = 80;
/// See `MIN_WIDTH`.
pub const MIN_HEIGHT: usize = 24;

/// Blank columns kept clear on each side of the grid.
const MARGIN: usize = 2;
/// Rows this screen always reserves outside the grid: the header, a blank line under it, a blank
/// line under the grid, the status or legend row, and the final complete line.
const FIXED_ROWS: usize = 5;

/// The line printed instead of a screen at all, for a directory with nothing (or nothing
/// measurable) in it. Exact and final; see the plan.
pub const NOTHING_TO_DEFRAGMENT: &str = "Nothing here to defragment. Cluster map clean.";

/// The line that appears once the fill finishes, under the legend. Exact and final; see the plan.
const COMPLETE: &str = "Defragmentation complete. It was never fragmented. Any key.";

/// The grid's own size for a terminal of `cols` by `rows`, once this screen's fixed margins and
/// rows are subtracted. Callers must have already checked `cols >= MIN_WIDTH` and
/// `rows >= MIN_HEIGHT`, which keeps both of these comfortably positive.
pub fn grid_size(cols: usize, rows: usize) -> (usize, usize) {
    (
        cols.saturating_sub(MARGIN * 2),
        rows.saturating_sub(FIXED_ROWS),
    )
}

/// The background SGR code the map colours entry `index` with: the same six colours, in the same
/// order, as `sprinkles::STRIPE_PALETTE`'s own foreground ones, since that is the exact six
/// stripe order the plan's own reference (`extras/ultra/ultra.zsh`'s `legend` array) already
/// uses. Cycles past six only in a test; a real map never has more than six entries.
fn background(index: usize) -> u8 {
    crate::sprinkles::STRIPE_PALETTE[index % crate::sprinkles::STRIPE_PALETTE.len()] + 10
}

/// Cuts `s` to `width` visible characters and pads it out to exactly that. `s` must carry no SGR
/// escape of its own; see `fit_runs` for a line built out of several differently coloured pieces.
fn fit(s: &str, width: usize) -> String {
    let mut out: String = s.chars().take(width).collect();
    let len = out.chars().count();
    if len < width {
        out.push_str(&" ".repeat(width - len));
    }
    out
}

/// Concatenates already styled `runs` (each paired with its own visible width, escapes not
/// counted) into one line exactly `width` visible characters wide. A run is added whole or not at
/// all, so a coloured block and the text beside it are never separated by a truncation landing in
/// the middle of one; anything left over is padded with plain spaces.
fn fit_runs(runs: &[(String, usize)], width: usize) -> String {
    let mut out = String::new();
    let mut used = 0;
    for (text, len) in runs {
        if used + len > width {
            break;
        }
        out.push_str(text);
        used += len;
    }
    out.push_str(&" ".repeat(width - used));
    out
}

fn header(map: &Map, label: &str) -> String {
    let total_bytes: u64 = map.entries.iter().map(|e| e.bytes).sum();
    let mb = total_bytes / 1_048_576;
    let n = map.entries.len();
    format!("Cluster map of {label}: {mb} MB in {n} entries.")
}

/// One row of the grid, `MARGIN` blank columns either side of `grid_cols` cells, padded to
/// exactly `cols`. Under `no_color` a revealed cell shows its owning entry's one-based position
/// (1 to 6) instead of a coloured block, since colour is the only thing that told them apart.
fn grid_row(map: &Map, row_index: usize, grid_cols: usize, cols: usize, no_color: bool) -> String {
    let mut out = String::with_capacity(cols * 5);
    out.push_str(&" ".repeat(MARGIN));
    for col in 0..grid_cols {
        let flat = row_index * grid_cols + col;
        match map.cell(flat) {
            Some(owner) if no_color => {
                out.push_str(&((owner + 1) % 10).to_string());
            }
            Some(owner) => {
                out.push_str(&format!("\x1b[{}m \x1b[0m", background(owner)));
            }
            None => out.push(' '),
        }
    }
    let used = MARGIN + grid_cols;
    out.push_str(&" ".repeat(cols.saturating_sub(used)));
    out
}

/// One run of the legend: a coloured block (or, under `no_color`, the entry's one-based digit)
/// followed by its name and size in megabytes, the same shape the plan's own reference draws.
fn legend_run(index: usize, entry: &super::model::Entry, no_color: bool) -> (String, usize) {
    let mb = entry.bytes / 1_048_576;
    let block = if no_color {
        ((index + 1) % 10).to_string()
    } else {
        format!("\x1b[{}m \x1b[0m", background(index))
    };
    let rest = format!(" {} {mb}MB  ", entry.name);
    let visible_len = 1 + rest.chars().count();
    (format!("{block}{rest}"), visible_len)
}

fn legend(map: &Map, cols: usize, no_color: bool) -> String {
    let runs: Vec<(String, usize)> = map
        .entries
        .iter()
        .enumerate()
        .map(|(i, e)| legend_run(i, e, no_color))
        .collect();
    fit_runs(&runs, cols)
}

/// The whole screen: `rows` lines, each exactly `cols` visible characters. While the map is still
/// filling, the bottom two lines are the `Reading` row and a blank; once it is done, they become
/// the legend and the completion line. Callers must have already checked `cols >= MIN_WIDTH` and
/// `rows >= MIN_HEIGHT`.
pub fn render(map: &Map, label: &str, cols: usize, rows: usize, no_color: bool) -> String {
    let (grid_cols, grid_rows) = grid_size(cols, rows);
    let mut lines = Vec::with_capacity(rows);
    lines.push(fit(&header(map, label), cols));
    lines.push(" ".repeat(cols));
    for r in 0..grid_rows {
        lines.push(grid_row(map, r, grid_cols, cols, no_color));
    }
    lines.push(" ".repeat(cols));
    if map.done() {
        lines.push(legend(map, cols, no_color));
        lines.push(fit(COMPLETE, cols));
    } else {
        lines.push(fit(&format!("Reading  {}%", map.percent()), cols));
        lines.push(" ".repeat(cols));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::defrag::model::Entry;

    fn strip_sgr(s: &str) -> String {
        let chars: Vec<char> = s.chars().collect();
        let mut out = String::new();
        let mut i = 0;
        while i < chars.len() {
            if chars[i] == '\x1b' {
                while i < chars.len() && chars[i] != 'm' {
                    i += 1;
                }
                i += 1;
                continue;
            }
            out.push(chars[i]);
            i += 1;
        }
        out
    }

    fn entries() -> Vec<Entry> {
        vec![
            Entry {
                name: "src".to_string(),
                bytes: 6_291_456, // 6MB
            },
            Entry {
                name: "docs".to_string(),
                bytes: 3_145_728, // 3MB
            },
            Entry {
                name: "target".to_string(),
                bytes: 1_048_576, // 1MB
            },
        ]
    }

    fn map(cols: usize, rows: usize) -> Map {
        let (grid_cols, grid_rows) = grid_size(cols, rows);
        Map::new(entries(), grid_cols * grid_rows, 7)
    }

    #[test]
    fn every_line_is_exactly_the_terminal_width_at_any_size() {
        for (cols, rows) in [(80, 24), (120, 40), (160, 50)] {
            let out = render(&map(cols, rows), "~/code/project", cols, rows, false);
            for (i, line) in strip_sgr(&out).lines().enumerate() {
                assert_eq!(line.chars().count(), cols, "{cols}x{rows} line {i}");
            }
            assert_eq!(out.lines().count(), rows, "{cols}x{rows}");
        }
    }

    #[test]
    fn the_initial_frame_shows_the_header_and_reading_zero_percent() {
        let out = render(&map(80, 24), "~/code/project", 80, 24, false);
        let plain = strip_sgr(&out);
        assert!(plain
            .lines()
            .next()
            .unwrap()
            .starts_with("Cluster map of ~/code/project: 10 MB in 3 entries."));
        assert!(plain.contains("Reading  0%"));
        assert!(!plain.contains("Any key"));
    }

    #[test]
    fn a_mid_fill_frame_shows_a_percentage_between_the_ends_and_some_cells_painted() {
        let mut m = map(80, 24);
        m.step(m.grid() / 2);
        let out = render(&m, "~/code/project", 80, 24, false);
        let plain = strip_sgr(&out);
        assert!(plain.contains("Reading  50%"));
        // Some, but not all, of the six colour codes appear on a half filled map.
        assert!(out.contains(&format!(
            "\x1b[{}m",
            crate::sprinkles::STRIPE_PALETTE[0] + 10
        )));
    }

    #[test]
    fn the_finished_frame_shows_the_legend_and_the_exact_completion_line() {
        let mut m = map(80, 24);
        m.step(m.grid());
        let out = render(&m, "~/code/project", 80, 24, false);
        let plain = strip_sgr(&out);
        assert!(plain.contains("src 6MB"));
        assert!(plain.contains("docs 3MB"));
        assert!(plain.contains("target 1MB"));
        assert!(plain.contains("Defragmentation complete. It was never fragmented. Any key."));
        assert!(!plain.contains("Reading"));
    }

    #[test]
    fn no_color_emits_no_escape_sequence_at_any_frame() {
        let mut m = map(80, 24);
        for _ in 0..3 {
            let out = render(&m, "~/code/project", 80, 24, true);
            assert!(!out.contains('\x1b'));
            m.step(m.grid());
        }
    }

    #[test]
    fn no_color_marks_cells_and_the_legend_with_the_entrys_own_digit() {
        let mut m = map(80, 24);
        m.step(m.grid());
        let out = render(&m, "~/code/project", 80, 24, true);
        assert!(out.contains("1 src 6MB"));
        assert!(out.contains("2 docs 3MB"));
        assert!(out.contains("3 target 1MB"));
    }

    #[test]
    fn the_screen_still_renders_at_the_floor_size() {
        let out = render(&map(80, 24), "~/code/project", 80, 24, false);
        assert_eq!(out.lines().count(), 24);
    }

    /// The pinned spacing, spelled out literally: two columns of margin either side of the grid,
    /// one blank row above it and one below it, and the legend on its own row under that. A small
    /// synthetic terminal size is used (not a real minimum; `bios defrag` itself still refuses
    /// anything under 80x24) purely so the whole grid fits in a snapshot small enough to read.
    /// `cell_owner` is built straight from `allocate`'s own counts, in entry order, so it (unlike
    /// the fill order) does not depend on the seed: once every cell is revealed, exactly which
    /// cell belongs to which entry is fully deterministic, which is what makes spelling the grid
    /// out literally possible at all.
    #[test]
    fn the_finished_frames_exact_geometry_is_the_pinned_spacing() {
        let cols = 20;
        let rows = 10;
        let (grid_cols, grid_rows) = grid_size(cols, rows);
        assert_eq!((grid_cols, grid_rows), (16, 5));

        let entries = vec![
            Entry {
                name: "a".to_string(),
                bytes: 1_048_576,
            },
            Entry {
                name: "b".to_string(),
                bytes: 1_048_576,
            },
        ];
        let mut m = Map::new(entries, grid_cols * grid_rows, 1);
        m.step(m.grid());

        let out = render(&m, "~/x", cols, rows, true);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), rows);

        // Row 0 is the header; its exact text is `header`'s own concern (see
        // `the_initial_frame_shows_the_header_and_reading_zero_percent`), only its width here.
        assert_eq!(lines[0].chars().count(), cols);

        // One blank row above the grid.
        assert_eq!(lines[1], " ".repeat(cols));

        // The grid: two blank margin columns either side, 40 of the 80 cells each entry (an even
        // split), row major, so entry "a" fills the first two and a half rows and entry "b" the
        // rest.
        let expected_rows = [
            format!("  {}  ", "1".repeat(16)),
            format!("  {}  ", "1".repeat(16)),
            format!("  {}{}  ", "1".repeat(8), "2".repeat(8)),
            format!("  {}  ", "2".repeat(16)),
            format!("  {}  ", "2".repeat(16)),
        ];
        for (i, expected) in expected_rows.iter().enumerate() {
            assert_eq!(&lines[2 + i], expected, "grid row {i}");
        }

        // One blank row below the grid.
        assert_eq!(lines[2 + grid_rows], " ".repeat(cols));

        // The legend, on its own row, directly under that blank row.
        assert_eq!(lines[2 + grid_rows + 1], legend(&m, cols, true));
        assert!(lines[2 + grid_rows + 1].contains("a 1MB"));
        assert!(lines[2 + grid_rows + 1].contains("b 1MB"));

        // The exact completion line, on the row after the legend.
        assert_eq!(lines[2 + grid_rows + 2], fit(COMPLETE, cols));
    }

    /// The same pinned spacing while still filling: a blank row above and below the grid either
    /// way, `Reading` sitting where the legend will later sit, and the completion line's own row
    /// left blank until the map is done.
    #[test]
    fn the_filling_frames_exact_geometry_matches_the_finished_frames() {
        let cols = 20;
        let rows = 10;
        let (grid_cols, grid_rows) = grid_size(cols, rows);

        let entries = vec![Entry {
            name: "a".to_string(),
            bytes: 1,
        }];
        let m = Map::new(entries, grid_cols * grid_rows, 1);
        assert_eq!(m.percent(), 0);

        let out = render(&m, "~/x", cols, rows, true);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), rows);

        assert_eq!(lines[1], " ".repeat(cols), "blank row above the grid");
        for i in 0..grid_rows {
            assert_eq!(
                lines[2 + i],
                " ".repeat(cols),
                "an unrevealed grid row should be blank"
            );
        }
        assert_eq!(
            lines[2 + grid_rows],
            " ".repeat(cols),
            "blank row below the grid"
        );
        assert_eq!(lines[2 + grid_rows + 1], fit("Reading  0%", cols));
        assert_eq!(
            lines[2 + grid_rows + 2],
            " ".repeat(cols),
            "the completion line's row is blank until the map is done"
        );
    }
}
