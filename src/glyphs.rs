//! Glyph sets for the timeline.
//!
//! A cell packs two samples side by side, each at one of five levels (0 = empty
//! through 4 = full), so a set is a 25-entry table indexed `left * 5 + right`.
//! btop uses the same packing with swappable tables, which is what lets a
//! terminal that cannot draw braille fall back without touching any layout
//! code.
//!
//! Cells also stack vertically: each row covers a slice of the 0..100 range, so
//! three rows of braille give twelve distinct heights.

/// How a value is drawn when one character cell must show two samples.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum GlyphSet {
    /// Braille. Four sub-rows a cell, so the finest line of the three.
    Braille,
    /// Half blocks — `▄▀█`. Two sub-rows a cell, one weight throughout, and
    /// legible in any font that draws a terminal. The default.
    #[default]
    Block,
    /// Box drawing — `╭ ╮ ╰ ╯ ─ │`. One level a row, and the cleanest line of
    /// the four: the corners make a stroke the eye follows without effort.
    Line,
    /// Pure ASCII. One sample per cell; the pair is merged by taking the peak.
    Ascii,
}

impl GlyphSet {
    /// The names `graph` and `glyphs` accept.
    ///
    /// `line` is the box-drawing set: one level a row rather than two, and the
    /// cleanest of the four to read, at the cost of vertical resolution.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "braille" => Some(Self::Braille),
            "block" => Some(Self::Block),
            "line" => Some(Self::Line),
            "ascii" => Some(Self::Ascii),
            _ => None,
        }
    }

    /// Every name, for an error message that lists what it would have taken.
    pub const NAMES: &'static str = "block, braille, line or ascii";

    /// Vertical positions a stroke can take inside one character cell.
    ///
    /// The line's resolution: a graph of `rows` rows resolves `rows * sub_rows`
    /// heights. Braille has four dot rows a cell and the block elements have two
    /// halves, which is the whole of the difference between the sets now that
    /// neither of them fills anything in.
    pub fn sub_rows(self) -> usize {
        match self {
            Self::Braille => 4,
            Self::Block | Self::Ascii => 2,
            // Box drawing has no half-height forms, so a cell is one level. The
            // corners buy legibility with resolution, which on a flat series is
            // the better trade: memory drifting 72→81% is genuinely flat, and
            // four levels a row draw that as wandering noise.
            Self::Line => 1,
        }
    }

    /// The stroke through one cell, from a mask of occupied sub-rows.
    ///
    /// Bit 0 is the bottom sub-row. A mask of zero is a cell the line does not
    /// reach, drawn as nothing at all — which is the point of a line: ink sits
    /// where the information is.
    pub fn stroke(self, mask: u8) -> char {
        match self {
            Self::Block => [' ', '▄', '▀', '█'][(mask & 0b11) as usize],
            // One level a row, so a set bit is the line passing through. The
            // corners come from `corner`, which knows the direction; this is the
            // fallback for the ends of the series.
            Self::Line => {
                if mask & 1 != 0 {
                    '─'
                } else {
                    ' '
                }
            }
            Self::Ascii => [' ', '_', '-', '|'][(mask & 0b11) as usize],
            Self::Braille => {
                // Both dot columns, so the stroke is the full width of the cell
                // rather than a half-width tick.
                const ROW: [u8; 4] = [0x40 | 0x80, 0x04 | 0x20, 0x02 | 0x10, 0x01 | 0x08];
                let bits = ROW
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| mask & (1 << i) != 0)
                    .fold(0u8, |acc, (_, d)| acc | d);
                char::from_u32(0x2800 + u32::from(bits)).unwrap_or(' ')
            }
        }
    }

    /// Samples represented by one character cell.
    /// How many samples a cell of a *one-row* sparkline covers.
    ///
    /// Braille packs two dot columns into a cell, so it shows twice the history
    /// at four vertical levels. The other sets have no sub-cell columns but do
    /// have eight sub-cell *rows* — the eighths ramp — so they trade that width
    /// for nine distinct heights. In one row, height is what there is to read:
    /// a ramp from idle to eight cores drew two levels under the paired
    /// glyphs and draws the whole climb under the ramp.
    pub fn spark_samples_per_cell(self) -> usize {
        match self {
            Self::Braille => 2,
            _ => 1,
        }
    }

    /// One cell of a one-row sparkline at `level` of eight.
    pub fn spark_glyph(self, level: usize) -> char {
        let ramp: [char; 9] = match self {
            Self::Ascii => [' ', '.', '.', '-', '-', '=', '=', '#', '#'],
            _ => [' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'],
        };
        ramp[level.min(8)]
    }

    /// How many samples a column of the timeline covers.
    ///
    /// Two for every set, which used to be a property of the alphabet — braille
    /// packed two dot columns into a cell, so it could show twice the history.
    /// Drawing a *line* between cell peaks needs no horizontal sub-cell
    /// resolution at all, so the density is now free: every set shows the same
    /// window, and the cursor's half-cell marker keeps the same meaning in all
    /// of them.
    pub fn samples_per_cell(self) -> usize {
        let _ = self;
        2
    }

    /// Levels per sample, `0..=4`.
    pub const LEVELS: usize = 5;

    /// The glyph for a pair of levels, each `0..=4`.
    pub fn glyph(self, left: usize, right: usize) -> char {
        let (l, r) = (left.min(4), right.min(4));
        match self {
            Self::Braille => braille(l, r),
            Self::Block => BLOCK[l * Self::LEVELS + r],
            // No sub-cell resolution: show the peak so a spike is never hidden
            // by the sample next to it.
            Self::Line | Self::Ascii => ASCII[l.max(r)],
        }
    }

    /// A rule mark alone, at dot height `1..=4`, for a cell with no data in it.
    ///
    /// Deliberately never merged into a bar. An earlier version OR'd the rule
    /// into the glyph, which meant a cell holding a spike and an idle sample
    /// lit a dot at the rule height in the *data* colour — pixel-identical to
    /// the idle sample having reached the threshold. Half the samples in the
    /// row could be misread. The rule now yields wherever data is present.
    pub fn rule_glyph(self, level: usize) -> char {
        let k = level.clamp(1, 4);
        match self {
            Self::Braille => {
                char::from_u32(0x2800 + (LEFT_DOTS[k - 1] | RIGHT_DOTS[k - 1])).unwrap_or(' ')
            }
            Self::Block => '─',
            // Dashed, not `─`: that is the Line set's own stroke, so a solid
            // rule would be the same character the series draws — a reference
            // line indistinguishable from data at every colour tier.
            // Dashed, not `─`: that is the Line set's own stroke, so a solid
            // rule would be the same character the series draws — a reference
            // line indistinguishable from data at every colour tier.
            Self::Line => '┄',
            // `.`, not `-`. The ascii strokes are ` _ - |`, so a `-` rule was
            // the same character a mid-height sample draws — the collision the
            // Line set had, and older.
            Self::Ascii => '.',
        }
    }

    /// The seam drawn where time is missing from the buffer.
    ///
    /// Deliberately not a data glyph. Box-drawing in a braille graph reads as
    /// foreign the moment you see it, which is the point: a gap is a statement
    /// about the axis, not about the machine. A dot pattern here would be one
    /// more thing to tell apart from a low bar.
    pub fn gap_glyph(self) -> char {
        match self {
            Self::Ascii => ':',
            _ => '\u{250a}',
        }
    }

    /// Marker showing which half of a cell the scrub cursor sits on. Without
    /// this, packing two samples per cell would halve cursor precision.
    /// The mark under the sample the cursor is on.
    ///
    /// A cell covers two samples, so a *half* is what the marker has to be:
    /// `▌` for the older of the pair, `▐` for the newer. Marking the whole cell
    /// would halve scrub precision in the one place precision is the point.
    pub fn cursor_marker(self, right_half: bool) -> char {
        match self {
            Self::Ascii => '^',
            _ if right_half => '▐',
            _ => '▌',
        }
    }
}

/// The box-drawing glyph for a cell, given where the line enters and leaves.
///
/// The corners are what make this set worth having: `╭` and `╯` turn a staircase
/// of dashes into a stroke the eye follows without effort. They need the
/// direction of travel, which a bitmask cannot carry — hence a function of its
/// own rather than an arm of [`GlyphSet::stroke`].
pub fn box_glyph(from: f32, to: f32, row: usize, rows: usize, ceiling: f32) -> char {
    if rows == 0 {
        return ' ';
    }
    let place = |v: f32| {
        let h = if ceiling > 0.0 {
            v / ceiling * rows as f32
        } else {
            0.0
        };
        rows - 1 - (h.clamp(0.0, rows as f32 - 0.001) as usize)
    };
    let (a, b) = (place(from), place(to));
    match (row == a, row == b) {
        // Flat: the line arrives and leaves at the same height.
        (true, true) => '─',
        // The near end of a step. Which corner depends on which way it turns.
        (true, false) => {
            if a > b {
                '╰'
            } else {
                '╮'
            }
        }
        (false, true) => {
            if a > b {
                '╭'
            } else {
                '╯'
            }
        }
        // Strictly between the two ends: the vertical part of the step.
        _ if (a.min(b)..=a.max(b)).contains(&row) => '│',
        _ => ' ',
    }
}

/// Which sub-rows of `row` a stroke between two samples passes through.
///
/// The change that made the timeline readable. It used to fill every cell from
/// the baseline to the value, so a machine sitting at eighty percent memory drew
/// three rows of solid ink and put all of its information in the top one. A line
/// puts ink only where the signal is.
///
/// Joined to the next sample rather than drawn as a point, because a column of
/// unconnected marks reads as scatter. The stroke covers every sub-row between
/// the two values, so a climb is a vertical run and the eye follows one path.
pub fn stroke_in_row(from: f32, to: f32, row: usize, rows: usize, sub: usize, ceiling: f32) -> u8 {
    if rows == 0 || sub == 0 {
        return 0;
    }
    let top = (rows * sub) as f32;
    let place = |v: f32| {
        let h = if ceiling > 0.0 {
            v / ceiling * top
        } else {
            0.0
        };
        // Clamped just under the top so a value at the ceiling lands in the
        // highest sub-row rather than one past it.
        h.clamp(0.0, top - 0.001) as usize
    };
    let (a, b) = (place(from), place(to));
    let mut mask = 0u8;
    for h in a.min(b)..=a.max(b) {
        // `h` counts sub-rows from the bottom; `row` counts rows from the top.
        if rows - 1 - h / sub == row {
            mask |= 1 << (h % sub);
        }
    }
    mask
}

/// Braille cells are `U+2800` plus a dot bitmask:
///
/// ```text
///   dot1 0x01   dot4 0x08
///   dot2 0x02   dot5 0x10
///   dot3 0x04   dot6 0x20
///   dot7 0x40   dot8 0x80
/// ```
///
/// Bars fill upward from the bottom, so level N lights the lowest N dots of its
/// column. Deriving this beats transcribing a 25-entry table: the rule is one
/// line and it is impossible to get a single entry subtly wrong.
fn braille(left: usize, right: usize) -> char {
    char::from_u32(0x2800 + braille_bits(left, right)).unwrap_or(' ')
}

/// Bottom-up dot order for each braille column.
const LEFT_DOTS: [u32; 4] = [0x40, 0x04, 0x02, 0x01];
const RIGHT_DOTS: [u32; 4] = [0x80, 0x20, 0x10, 0x08];

fn braille_bits(left: usize, right: usize) -> u32 {
    let mut bits = 0;
    for dot in LEFT_DOTS.iter().take(left) {
        bits |= dot;
    }
    for dot in RIGHT_DOTS.iter().take(right) {
        bits |= dot;
    }
    bits
}

/// Which row of a `rows`-tall graph holds `pct`, and the dot height `1..=4`
/// the rule sits at within that row.
///
/// `None` when the threshold falls outside the graph entirely.
/// `None` when the threshold sits above the ceiling — the common case on an
/// idle machine, and why the rules stop dashing a hundred dots of noise across
/// an otherwise empty graph.
pub fn rule_position_scaled(pct: f32, rows: usize, ceiling: f32) -> Option<(usize, usize)> {
    if !(0.0..=ceiling).contains(&pct) || rows == 0 {
        return None;
    }
    let rows_f = rows as f32;
    for row in 0..rows {
        let high = ceiling * (rows_f - row as f32) / rows_f;
        let low = ceiling * (rows_f - row as f32 - 1.0) / rows_f;
        // Top row owns its upper bound so a 100% threshold has somewhere to go.
        let in_band = if row == 0 { pct <= high } else { pct < high };
        if in_band && pct >= low {
            // Must use the same mapping the bars use. With `ceil` here and
            // `round` there, the rule sat a dot above where a bar of the same
            // percentage lands: at six graph rows an 80% bar had to reach
            // 81.25% before it touched its own 80% line, while `heat_style`
            // already coloured it critical. Two signals, contradicting.
            let level = level_in_row_scaled(pct, row, rows, ceiling).max(1);
            return Some((row, level));
        }
    }
    None
}

/// Quadrant blocks, `left * 5 + right`. Levels 1-2 are the lower half and 3-4
/// the full height, so this set carries two levels per sample rather than four.
#[rustfmt::skip]
const BLOCK: [char; 25] = [
    ' ', '▗', '▗', '▐', '▐',
    '▖', '▄', '▄', '▟', '▟',
    '▖', '▄', '▄', '▟', '▟',
    '▌', '▙', '▙', '█', '█',
    '▌', '▙', '▙', '█', '█',
];

const ASCII: [char; 5] = [' ', '.', ':', '|', '#'];

/// Left-to-right eighths, for a horizontal bar.
const EIGHTHS: [char; 8] = ['▏', '▎', '▍', '▌', '▋', '▊', '▉', '█'];

/// A horizontal bar `width` cells wide, filled to `frac` of full.
///
/// Eighths rather than whole cells: four cells at eight sub-steps each is
/// thirty-two levels, which is enough to compare two rows at a glance. Whole
/// cells would give four, which is not.
///
/// A fraction above 1.0 is real — a threaded process can exceed one core — and
/// returns a full bar. The caller marks the overflow; silently clipping it to
/// full would make 400% and 100% look identical.
pub fn micro_bar(frac: f32, width: usize) -> String {
    if !frac.is_finite() || frac <= 0.0 {
        return " ".repeat(width);
    }
    let eighths = (frac.min(1.0) * (width * 8) as f32).round() as usize;
    let full = eighths / 8;
    let rest = eighths % 8;
    let mut s = String::with_capacity(width);
    for _ in 0..full.min(width) {
        s.push('█');
    }
    if full < width && rest > 0 {
        s.push(EIGHTHS[rest - 1]);
    }
    while s.chars().count() < width {
        s.push(' ');
    }
    s
}

/// Three shades that read as an order without any colour at all.
///
/// The composition bar has to work at the mono tier like everything else here,
/// so the segments separate by glyph density first and hue second: solid, half,
/// empty. Nobody needs to be told which end is which.
pub const SEG_USED: char = '\u{2588}';
pub const SEG_CACHE: char = '\u{2592}';
pub const SEG_FREE: char = '\u{2591}';

/// Split `width` columns between three parts in proportion.
///
/// Largest-remainder rather than plain rounding, so the three always sum to
/// exactly `width` — a bar one column short of its box looks like a rendering
/// fault, and one column long pushes everything after it sideways.
///
/// A part smaller than half a column gets nothing, and that is deliberate. An
/// earlier version gave every non-zero part a floor of one column so a sliver
/// of cache could not vanish; at twelve columns that moved the bar by up to
/// sixteen percentage points, directly beside the figure stating the real one.
/// A picture that contradicts the number next to it is worse than a picture
/// that cannot resolve a third of a percent — and at this width, it genuinely
/// cannot.
pub fn composition(parts: [u64; 3], width: usize) -> [usize; 3] {
    let total: u64 = parts.iter().sum();
    if total == 0 || width == 0 {
        return [0, 0, 0];
    }
    let exact: Vec<f64> = parts
        .iter()
        .map(|&p| p as f64 / total as f64 * width as f64)
        .collect();
    let mut out = [0usize; 3];
    for i in 0..3 {
        out[i] = exact[i] as usize;
    }
    // Hand the leftover columns to the largest remainders.
    let mut assigned: usize = out.iter().sum();
    while assigned < width {
        let i = (0..3)
            .max_by(|&a, &b| (exact[a] - out[a] as f64).total_cmp(&(exact[b] - out[b] as f64)))
            .unwrap_or(0);
        out[i] += 1;
        assigned += 1;
    }
    out
}

/// Axis steps below one core, in percent.
///
/// Hand-chosen, because the interesting resolution on a quiet machine is at the
/// bottom. Above 100 the steps just double — see [`ceiling_for`] — so there is
/// nothing to write down: every entry that used to sit up there produced
/// exactly what doubling produces, which is how they came to be deleted.
///
/// Steps rather than the peak itself, so the axis is stable while scrubbing
/// instead of breathing with every sample. Stable, not fixed: a scrub that
/// brings a 900% burst into the buffer does move it from 800 to 1600, which is
/// why the panel prints the figure rather than leaving it to be inferred.
const CEILINGS: [f32; 4] = [10.0, 25.0, 50.0, 100.0];

/// The axis ceiling for a given peak.
///
/// A fixed 0..100 axis means an idle machine draws one lit row and eight blank
/// ones — the largest panel on screen showing almost nothing. Scaling to the
/// peak fills the graph, and printing the ceiling keeps it honest: the axis
/// says what it is.
pub fn ceiling_for(peak: f32) -> f32 {
    CEILINGS
        .iter()
        .copied()
        .find(|&c| peak <= c)
        .unwrap_or_else(|| {
            // Past one core, keep doubling.
            //
            // A per-process figure is not a share of the machine: a virtual
            // machine on three cores is 300%, and htop reports the same. The
            // ladder used to stop at 100 and clamp every such process to the
            // top of its graph — a ramp to 800% drawn solid for seven eighths
            // of its length, which is not a clipped graph but no graph at all.
            //
            // Unbounded rather than a longer table, because a 96-core machine
            // can put one process at 9600%: an axis stopping at the largest
            // number somebody once wrote down is the same clipping, one step
            // further out.
            let mut c = CEILINGS[CEILINGS.len() - 1];
            while c < peak {
                c *= 2.0;
            }
            c
        })
}

/// Split a percentage into the level `0..=4` it occupies in row `row` of a
/// `rows`-tall graph, counting rows from the top, against an axis that tops
/// out at `ceiling`.
///
/// Each row owns a band of the 0..100 range: a value above the band fills the
/// row, below it leaves the row empty, and inside it scales across the five
/// levels.
pub fn level_in_row_scaled(pct: f32, row: usize, rows: usize, ceiling: f32) -> usize {
    let rows = rows.max(1) as f32;
    let ceiling = ceiling.max(1.0);
    let high = ceiling * (rows - row as f32) / rows;
    let low = ceiling * (rows - row as f32 - 1.0) / rows;

    if pct >= high {
        4
    } else if pct <= low {
        0
    } else {
        (((pct - low) * 4.0 / (high - low)).round() as usize).clamp(1, 4)
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn a_process_above_one_core_gets_an_axis_that_fits_it() {
        // A per-process figure is not a share of the machine: a virtual machine
        // on three cores is 300%. The ladder used to stop at 100, so every such
        // process was pinned to the top of its own graph.
        assert_eq!(ceiling_for(285.0), 400.0);
        assert_eq!(ceiling_for(1400.0), 1600.0);
    }

    #[test]
    fn an_axis_beyond_the_table_keeps_doubling() {
        // A 96-core machine can put one process at 9600%. Stopping at the
        // largest number somebody once wrote down is the same clipping, one
        // step up.
        assert_eq!(ceiling_for(9600.0), 12800.0);
        assert!(ceiling_for(100_000.0) >= 100_000.0);
    }

    #[test]
    fn the_axis_still_hugs_a_quiet_machine() {
        // The reason the ladder exists at all: a fixed axis leaves an idle
        // machine drawing one lit row and eight blank ones.
        assert_eq!(ceiling_for(0.0), 10.0);
        assert_eq!(ceiling_for(8.0), 10.0);
        assert_eq!(ceiling_for(60.0), 100.0);
    }

    #[test]
    fn a_waveform_above_one_core_is_still_a_waveform() {
        // The defect in one assertion. Against a ceiling of 100 a sawtooth
        // between 200% and 800% renders as a solid block, top and bottom alike:
        // not a clipped graph but no graph at all.
        let ceiling = ceiling_for(800.0);
        let peak = level_in_row_scaled(800.0, 0, 3, ceiling);
        let trough = level_in_row_scaled(200.0, 0, 3, ceiling);
        assert_ne!(
            peak, trough,
            "the top row cannot tell 200% from 800% at a ceiling of {ceiling}"
        );
    }

    use super::*;

    #[test]
    fn braille_matches_the_known_encoding() {
        // Spot-checked against btop's hand-written table (btop_draw.cpp:90).
        assert_eq!(braille(0, 0), '\u{2800}');
        assert_eq!(braille(0, 1), '⢀');
        assert_eq!(braille(1, 0), '⡀');
        assert_eq!(braille(1, 1), '⣀');
        assert_eq!(braille(2, 2), '⣤');
        assert_eq!(braille(4, 4), '⣿');
        assert_eq!(braille(4, 0), '⡇');
        assert_eq!(braille(0, 4), '⢸');
    }

    #[test]
    fn every_level_pair_is_representable() {
        for set in [GlyphSet::Braille, GlyphSet::Block, GlyphSet::Ascii] {
            for l in 0..GlyphSet::LEVELS {
                for r in 0..GlyphSet::LEVELS {
                    let _ = set.glyph(l, r);
                }
            }
        }
    }

    #[test]
    fn glyphs_grow_monotonically_with_level() {
        // A taller bar must never render as a shorter glyph, which is the way a
        // hand-written table goes wrong.
        for l in 1..GlyphSet::LEVELS {
            let prev = braille(l - 1, 0) as u32;
            assert!(braille(l, 0) as u32 > prev, "left level {l} did not grow");
            let prev = braille(0, l - 1) as u32;
            assert!(braille(0, l) as u32 > prev, "right level {l} did not grow");
        }
    }

    #[test]
    fn out_of_range_levels_saturate_rather_than_panic() {
        assert_eq!(GlyphSet::Braille.glyph(99, 99), '⣿');
        assert_eq!(GlyphSet::Block.glyph(99, 99), '█');
        assert_eq!(GlyphSet::Ascii.glyph(99, 0), '#');
    }

    #[test]
    fn ascii_shows_the_peak_of_the_pair() {
        // Merging by average would hide a spike next to an idle sample, which
        // is the one thing the timeline exists to show.
        assert_eq!(GlyphSet::Ascii.glyph(0, 4), '#');
        assert_eq!(GlyphSet::Ascii.glyph(4, 0), '#');
    }

    #[test]
    fn rows_partition_the_range() {
        // Top row of three covers ~67..100, bottom covers 0..~33.
        assert_eq!(level_in_row_scaled(100.0, 0, 3, 100.0), 4);
        assert_eq!(level_in_row_scaled(0.0, 0, 3, 100.0), 0);
        assert_eq!(level_in_row_scaled(100.0, 2, 3, 100.0), 4);
        assert_eq!(level_in_row_scaled(0.0, 2, 3, 100.0), 0);
        // A mid value fills the lower rows and partly fills its own.
        assert_eq!(level_in_row_scaled(50.0, 2, 3, 100.0), 4);
        assert!((1..=4).contains(&level_in_row_scaled(50.0, 1, 3, 100.0)));
        assert_eq!(level_in_row_scaled(50.0, 0, 3, 100.0), 0);
    }

    #[test]
    fn single_row_spans_the_whole_range() {
        assert_eq!(level_in_row_scaled(0.0, 0, 1, 100.0), 0);
        assert_eq!(level_in_row_scaled(100.0, 0, 1, 100.0), 4);
        assert!((1..=4).contains(&level_in_row_scaled(50.0, 0, 1, 100.0)));
    }

    #[test]
    fn the_rule_is_visible_in_empty_space() {
        for set in [GlyphSet::Braille, GlyphSet::Block, GlyphSet::Ascii] {
            for level in 1..=4 {
                assert_ne!(
                    set.rule_glyph(level),
                    set.glyph(0, 0),
                    "{set:?}: rule at {level} is invisible"
                );
            }
        }
    }

    #[test]
    fn the_rule_spans_both_halves_of_a_braille_cell() {
        // A mark on one column only would read as a speck, not a line.
        let bits = GlyphSet::Braille.rule_glyph(1) as u32 - 0x2800;
        assert_eq!(bits, LEFT_DOTS[0] | RIGHT_DOTS[0]);
    }

    #[test]
    fn the_rule_sits_where_a_bar_of_the_same_value_would_reach() {
        // The bug this guards: the rule used `ceil` and the bars `round`, so
        // at six graph rows an 80% bar had to climb to 81.25% before it
        // touched its own 80% line, while heat_style already called it
        // critical. Two signals, contradicting each other.
        for rows in 1..=12 {
            let (row, level) = rule_position_scaled(80.0, rows, 100.0).unwrap();
            let bar = level_in_row_scaled(80.0, row, rows, 100.0);
            assert_eq!(
                level,
                bar.max(1),
                "{rows} rows: rule at {level}, an 80% bar at {bar}"
            );
        }
    }

    #[test]
    fn rule_position_lands_in_the_right_band() {
        // 80% of a 3-row graph is in the top row, which spans 66.7..100.
        let (row, level) = rule_position_scaled(80.0, 3, 100.0).unwrap();
        assert_eq!(row, 0);
        assert!((1..=4).contains(&level));
        // 50% of a 2-row graph is the boundary between the bands. It resolves
        // to the bottom dot of the upper row, which is that boundary drawn.
        assert_eq!(rule_position_scaled(50.0, 2, 100.0), Some((0, 1)));
        // Extremes stay inside the graph.
        assert_eq!(rule_position_scaled(100.0, 3, 100.0).unwrap().0, 0);
        assert_eq!(rule_position_scaled(0.0, 3, 100.0).unwrap().0, 2);
    }

    #[test]
    fn rule_position_refuses_the_impossible() {
        assert_eq!(rule_position_scaled(120.0, 3, 100.0), None);
        assert_eq!(rule_position_scaled(-1.0, 3, 100.0), None);
        assert_eq!(rule_position_scaled(50.0, 0, 100.0), None);
    }

    #[test]
    fn every_row_of_a_graph_can_hold_the_rule() {
        // Whatever the panel height, the threshold must land somewhere.
        for rows in 1..=8 {
            assert!(
                rule_position_scaled(80.0, rows, 100.0).is_some(),
                "{rows} rows lost the rule"
            );
        }
    }

    #[test]
    fn micro_bar_is_monotonic_and_exact_width() {
        let mut prev = 0usize;
        for i in 0..=100 {
            let b = micro_bar(i as f32 / 100.0, 4);
            assert_eq!(b.chars().count(), 4, "width drifted at {i}%");
            // Ink can only increase with the value, or two rows cannot be
            // compared by looking at them.
            let ink = b.chars().filter(|c| *c != ' ').count();
            assert!(ink >= prev, "bar shrank between {}% and {i}%", i - 1);
            prev = ink;
        }
        assert_eq!(micro_bar(1.0, 4), "████");
        assert_eq!(micro_bar(0.0, 4), "    ");
    }

    #[test]
    fn micro_bar_over_full_saturates_rather_than_overflowing() {
        // A threaded process really can use 400% of a core.
        assert_eq!(micro_bar(4.0, 4), "████");
        assert_eq!(micro_bar(4.0, 4).chars().count(), 4);
    }

    #[test]
    fn micro_bar_handles_nonsense_without_panicking() {
        for f in [f32::NAN, f32::INFINITY, -1.0, -0.0] {
            assert_eq!(micro_bar(f, 4).chars().count(), 4);
        }
        assert_eq!(micro_bar(0.5, 0), "");
    }

    #[test]
    fn parse_round_trips() {
        assert_eq!(GlyphSet::parse("braille"), Some(GlyphSet::Braille));
        assert_eq!(GlyphSet::parse("block"), Some(GlyphSet::Block));
        assert_eq!(GlyphSet::parse("ascii"), Some(GlyphSet::Ascii));
        assert_eq!(GlyphSet::parse("nonsense"), None);
    }
}

#[cfg(test)]
mod composition_tests {
    use super::*;

    #[test]
    fn the_segments_always_fill_the_bar_exactly() {
        // A bar one column short of its box reads as a rendering fault, and one
        // column long pushes everything after it sideways.
        for width in 1..=24usize {
            for parts in [
                [1u64, 1, 1],
                [100, 0, 0],
                [0, 0, 100],
                [999, 1, 1],
                [1, 999, 1],
                [7, 3, 90],
                [u64::MAX / 3, u64::MAX / 3, u64::MAX / 3],
            ] {
                let seg = composition(parts, width);
                assert_eq!(
                    seg.iter().sum::<usize>(),
                    width,
                    "{parts:?} at {width} gave {seg:?}"
                );
            }
        }
    }

    #[test]
    fn the_bar_never_contradicts_the_figure_beside_it() {
        // An earlier version floored every non-zero part at one column so a
        // sliver of cache could not vanish. At twelve columns that moved the
        // bar by up to sixteen points, next to a figure stating the real one.
        for width in [8usize, 12, 20] {
            for parts in [
                [999u64, 1, 1],
                [1, 999, 1],
                [1, 1, 999],
                [500, 499, 1],
                [340, 330, 330],
            ] {
                let total: u64 = parts.iter().sum();
                let seg = composition(parts, width);
                for i in 0..3 {
                    let want = parts[i] as f64 / total as f64 * width as f64;
                    assert!(
                        (seg[i] as f64 - want).abs() <= 1.0,
                        "{parts:?} at {width}: segment {i} drew {} columns for {want:.2}",
                        seg[i]
                    );
                }
            }
        }
    }

    #[test]
    fn a_part_too_small_to_see_is_not_drawn_as_if_it_were() {
        // Half a column is the line: below it, a segment would have to be
        // rounded up past its own size to appear at all.
        let seg = composition([999_999, 1, 1], 12);
        assert_eq!(seg, [12, 0, 0], "a third of a percent was given a column");
        // …and a part that can be resolved still is.
        let seg = composition([10, 1, 1], 12);
        assert!(seg[1] >= 1 && seg[2] >= 1, "{seg:?}");
    }

    #[test]
    fn nothing_at_all_draws_nothing() {
        assert_eq!(composition([0, 0, 0], 8), [0, 0, 0]);
        assert_eq!(composition([1, 2, 3], 0), [0, 0, 0]);
    }

    #[test]
    fn the_shades_read_as_an_order_without_colour() {
        // They have to separate at the mono tier like everything else here.
        assert_ne!(SEG_USED, SEG_CACHE);
        assert_ne!(SEG_CACHE, SEG_FREE);
        assert_ne!(SEG_USED, SEG_FREE);
    }
    #[test]
    fn a_rule_is_never_spelled_like_the_series_it_rules() {
        // A reference line and a row of samples have to be told apart at the
        // mono tier, where both are dim and colour says nothing. Dashing does
        // most of that work, but it only works if the mark itself is not one
        // the series can draw: `─` is the Line set's own stroke, so a solid
        // rule in that set was the picture drawing its own chrome.
        //
        // Braille is the documented exception. Every one of its 256 glyphs is a
        // dot pattern the data can reach, so there is no spare character to
        // reserve, and the separation there rests entirely on the dashing —
        // which `the_rule_is_dashed_so_it_cannot_be_read_as_data` pins.
        for set in [GlyphSet::Block, GlyphSet::Line, GlyphSet::Ascii] {
            let mut drawn: Vec<char> = (0..=255u8).map(|m| set.stroke(m)).collect();
            if set == GlyphSet::Line {
                // Box drawing picks its glyph from the direction of travel, not
                // from a mask, so `stroke` does not enumerate it.
                for &(from, to) in &[(0.0f32, 0.0f32), (0.0, 1.0), (1.0, 0.0), (1.0, 1.0)] {
                    for row in 0..3 {
                        drawn.push(box_glyph(from, to, row, 3, 1.0));
                    }
                }
            }
            for k in 1..=4 {
                let rule = set.rule_glyph(k);
                assert!(
                    !drawn.contains(&rule),
                    "{set:?} rules with {rule:?}, which is also one of its strokes"
                );
            }
        }
    }
}
