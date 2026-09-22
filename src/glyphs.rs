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
    #[default]
    Braille,
    /// Half blocks — `▄▀█`. Two sub-rows a cell, one weight throughout, and
    /// legible in any font that draws a terminal. The default.
    Block,
    /// Box drawing — `╭ ╮ ╰ ╯ ─ │`. One level a row, and the cleanest line of
    /// the four: the corners make a stroke the eye follows without effort.
    Line,
    /// Pure ASCII. One sample per cell; the pair is merged by taking the peak.
    Ascii,
}

/// Whether a set fills the area under a value or outlines it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Draw {
    /// A column of ink from the baseline up to the value.
    Bars,
    /// The outline alone, for a series that sits high and flat.
    Line,
}

/// Where a graph's axis starts.
///
/// A separate decision from the character set, and a setting rather than
/// something the graph does on its own. Fitting reclaims the rows a high flat
/// series wastes, but it also changes the *form* — bars encode magnitude by
/// area, so a truncated axis makes 74 look like a third of 84, and a fitted
/// panel has to be drawn as a line to stay honest. That is a trade worth
/// offering and not worth imposing: most people want bars, and bars want zero.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Axis {
    /// From zero, always. Bars, and the encoding is true.
    #[default]
    Zero,
    /// Fitted to the data where that reclaims rows, drawn as a line.
    Fit,
}

impl Axis {
    pub const NAMES: &'static str = "zero or fit";

    /// The name [`Axis::parse`] takes, for writing a config back out.
    pub fn name(self) -> &'static str {
        match self {
            Self::Zero => "zero",
            Self::Fit => "fit",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "zero" => Some(Self::Zero),
            "fit" => Some(Self::Fit),
            _ => None,
        }
    }
}

/// The range a graph's rows cover: what the bottom means and what the top does.
///
/// The ceiling has always adapted to the data. The floor did not — it was zero,
/// always — so a series living between 72% and 85% was drawn across a panel
/// spanning 0 to 100, and the 72 points below the signal were rows of ink that
/// never changed whatever the machine did. Three rows, one of them informative.
///
/// Fitting the floor to the data reclaims them. It also creates the classic
/// misleading chart if it is done to bars, because a bar's *area* encodes its
/// magnitude and truncating the axis makes 74 look like a third of 84. So the
/// form follows the scale rather than the other way round: an axis at zero is
/// drawn as bars, and a fitted axis is drawn as a line, which encodes change
/// and for which a truncated axis is both standard and honest. See
/// [`Scale::fitted`] and its one caller in `glyph_row`.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Scale {
    /// The value at the bottom of the bottom row.
    pub floor: f32,
    /// The value at the top of the top row.
    pub ceiling: f32,
    /// Whether the floor was moved off zero to fit the data.
    pub fitted: bool,
}

impl Scale {
    /// The old behaviour, and still the right one for a series that uses its
    /// range: everything from zero to a legible ceiling.
    pub fn zero(ceiling: f32) -> Self {
        Self {
            floor: 0.0,
            ceiling,
            fitted: false,
        }
    }

    /// The scale for a series spanning `min..=max`, given the ceiling its unit
    /// would pick for a zero-based axis.
    ///
    /// Never fitted unless asked: see [`Axis`]. When asked, fitted only when
    /// the zero-based view would spend less than a third of
    /// the panel on the data — below that the rows below the signal outnumber
    /// the rows carrying it, which is the case worth spending the extra
    /// machinery on. Above it, zero is both honest and no worse.
    ///
    /// The bounds land on a round step rather than on the raw minimum and
    /// maximum, which does three jobs at once: it keeps the series off the
    /// edges of its own panel, it gives the axis a number a person can read,
    /// and it is the hysteresis — as the window slides, `min` and `max` move
    /// continuously while the bounds only move when they cross a step, so the
    /// axis does not flap and neither does the form that follows it.
    pub fn pick(min: f32, max: f32, zero_ceiling: f32, axis: Axis) -> Self {
        let span = max - min;
        if axis == Axis::Zero
            || !span.is_finite()
            || !min.is_finite()
            || zero_ceiling <= 0.0
            || max <= 0.0
            // Fitting only pays when the zero-based view would spend most of
            // the panel below the signal.
            || span * 3.0 >= zero_ceiling
            // And only when there is a signal. A series that does not move has
            // no variation to reclaim rows for, and fitting it produces a
            // degenerate band — a flat 22% drawn between 22 and 23, which reads
            // as a value pinned to a floor rather than as a value not moving.
            // Zero-based, "22 out of 25" says it at a glance.
            || span * 20.0 < zero_ceiling
        {
            return Self::zero(zero_ceiling);
        }
        let step = nice_step(span / 3.0);
        let floor = ((min / step).floor() * step).max(0.0);
        let mut ceiling = (max / step).ceil() * step;
        // A series that never moves lands both bounds on the same step. One
        // step of range keeps it a graph rather than a division by zero.
        if ceiling <= floor {
            ceiling = floor + step;
        }
        // Fitting to a floor of zero is just the zero-based axis, and saying it
        // is fitted would switch the form for no gain.
        if floor <= 0.0 {
            return Self::zero(ceiling.max(zero_ceiling.min(ceiling)));
        }
        Self {
            floor,
            ceiling,
            fitted: true,
        }
    }

    /// Where `v` sits between the floor and the ceiling: 0.0 at the bottom of
    /// the graph, 1.0 at the top. Outside that range if `v` is off the scale,
    /// which is how a threshold rule knows it has nowhere to go.
    pub fn frac(self, v: f32) -> f32 {
        let span = self.ceiling - self.floor;
        if span <= 0.0 {
            return 0.0;
        }
        (v - self.floor) / span
    }
}

/// A step a person can read: 1, 2 or 5 times a power of ten.
///
/// The axis is for humans, and 70 to 85 in fives is legible where 71.6 to 84.9
/// is arithmetic.
fn nice_step(rough: f32) -> f32 {
    // NaN included, which the negation would have swallowed.
    if !rough.is_finite() || rough <= 0.0 {
        return 1.0;
    }
    let magnitude = 10f32.powf(rough.log10().floor());
    let n = rough / magnitude;
    let m = if n <= 1.0 {
        1.0
    } else if n <= 2.0 {
        2.0
    } else if n <= 5.0 {
        5.0
    } else {
        10.0
    };
    m * magnitude
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

    /// The name [`GlyphSet::parse`] takes, for writing a config back out.
    ///
    /// `Line` is here too: it is one of the four a config file may name, and a
    /// setting that can be read and not written back is one the file cannot
    /// round-trip.
    pub fn name(self) -> &'static str {
        match self {
            Self::Braille => "braille",
            Self::Block => "block",
            Self::Ascii => "ascii",
            Self::Line => "line",
        }
    }

    /// How this set puts a value on the screen.
    ///
    /// Three of the four fill: a column of ink from the baseline up to the
    /// value, which is what a sparkline has always been and what the eye reads
    /// fastest. `Line` draws the outline instead, for the case where a series
    /// is high and flat and the fill would be a wall.
    pub fn draws(self) -> Draw {
        match self {
            Self::Block | Self::Ascii | Self::Braille => Draw::Bars,
            Self::Line => Draw::Line,
        }
    }

    /// Vertical levels one character cell can show.
    ///
    /// This is the set's whole resolution argument. The block elements have an
    /// eighths ramp — `▁▂▃▄▅▆▇█` — so a cell shows eight. Braille has four dot
    /// rows, and spends the difference on width instead: two samples a cell
    /// against the block set's one. Ascii has `_ - ‾` and little else.
    pub fn sub_rows(self) -> usize {
        match self {
            Self::Block => 8,
            Self::Braille => 4,
            Self::Ascii => 3,
            // Box drawing has no part-height forms, so a cell is one level.
            Self::Line => 1,
        }
    }

    /// Whether this set draws each half of a cell from its own sample.
    ///
    /// Braille can, at no cost: its two dot columns are independent, and each
    /// fills from the bottom to its own level with the same four steps a
    /// full-width bar has. So every sample owns one column of the picture from
    /// the moment it is drawn until it scrolls off, and the graph moves by one
    /// column a sample.
    ///
    /// The others cannot without losing something worth more. Block's paired
    /// form is quadrants — two steps a column where its bar has eight — and
    /// ASCII and box drawing have no part-cell forms at all. They draw a cell
    /// as one bar at its peak and step a cell every other sample, which is the
    /// trade their alphabets make.
    pub fn pairs_in_a_cell(self) -> bool {
        matches!(self, Self::Braille)
    }

    /// One cell of a bar, given how much of this row the value fills.
    ///
    /// `level` runs 0 (nothing) to [`sub_rows`] (the whole cell).
    pub fn bar(self, level: usize) -> char {
        let k = level.min(self.sub_rows());
        match self {
            Self::Block => [' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'][k],
            Self::Ascii => [' ', '_', '-', '#'][k],
            Self::Braille => {
                // Both dot columns at one level. What the timeline draws is the
                // paired form, `glyph` below, one sample to a column — see
                // `pairs_in_a_cell` for why. This stays for a cell that has only
                // one value to show at full width.
                const UP: [u8; 4] = [0x40 | 0x80, 0x04 | 0x20, 0x02 | 0x10, 0x01 | 0x08];
                let bits = UP[..k].iter().fold(0u8, |acc, d| acc | d);
                char::from_u32(0x2800 + u32::from(bits)).unwrap_or(' ')
            }
            Self::Line => {
                if k > 0 {
                    '─'
                } else {
                    ' '
                }
            }
        }
    }

    /// The character this set draws an empty cell with.
    ///
    /// Not always a space: braille's blank is `U+2800`, a real character that
    /// happens to have no dots raised. Comparing against `' '` instead meant
    /// "is this cell empty" was false for every braille cell on screen — which
    /// silently switched off every threshold rule in that set, because the rule
    /// only fills cells the series does not reach.
    pub fn blank(self) -> char {
        self.bar(0)
    }

    /// One cell of an outline, given where the line enters and leaves.
    ///
    /// Box drawing wherever the font has it: the corners are what turn a
    /// staircase of dashes into a stroke the eye follows. Ascii gets the
    /// nearest thing it has, since a fitted axis must not be drawn as bars in
    /// any set and `ascii` is the set that has no box drawing.
    pub fn line(self, from: f32, to: f32, row: usize, rows: usize) -> char {
        let c = box_glyph(from, to, row, rows);
        match self {
            Self::Ascii => match c {
                '─' => '-',
                '│' => '|',
                '╭' | '╮' => '.',
                '╰' | '╯' => '\'',
                other => other,
            },
            _ => c,
        }
    }

    /// Every character this set can draw a value with.
    ///
    /// Written down so a rule can be checked against it: a reference line
    /// spelled like the series is a reference line nobody can find.
    #[cfg(test)]
    pub fn alphabet(self) -> Vec<char> {
        let mut out: Vec<char> = (0..=self.sub_rows()).map(|k| self.bar(k)).collect();
        // The table sparkline, which draws in the same set.
        out.extend((0..=8).map(|k| self.spark_glyph(k)));
        if self == Self::Line {
            for &(from, to) in &[(0.0f32, 0.0f32), (0.0, 1.0), (1.0, 0.0), (1.0, 1.0)] {
                for row in 0..3 {
                    out.push(box_glyph(from, to, row, 3));
                }
            }
        }
        if self.spark_samples_per_cell() == 2 {
            // The paired form, which packs two samples into one cell.
            for l in 0..=4 {
                for r in 0..=4 {
                    out.push(self.glyph(l, r));
                }
            }
        }
        out
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
    /// The set whose alphabet is actually in force, given how the graph draws.
    ///
    /// A fitted axis is drawn as a line in every set (see `Scale`), and the line
    /// is box drawing wherever the font has it. So a `block` panel drawing a
    /// fitted series is spelling its data in the `Line` set's characters, and
    /// must take that set's rule too — `block` rules with `─`, which is exactly
    /// what box drawing draws a flat stretch of series with.
    pub fn drawn_as(self, draws: Draw) -> Self {
        match (draws, self) {
            // Ascii has no box drawing and keeps its own marks throughout.
            (Draw::Line, Self::Ascii) => Self::Ascii,
            (Draw::Line, _) => Self::Line,
            (Draw::Bars, _) => self,
        }
    }

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
            // `~`, which is the only mark left. Ascii draws values with
            // ` _ - # . : = |` across its bar and its sparkline, and a rule
            // spelled like any of them is a rule that reads as a sample — the
            // collision the Line set had, and older.
            Self::Ascii => '~',
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
    /// The mark under the cell the cursor is in.
    pub fn cursor_marker(self) -> char {
        match self {
            Self::Ascii => '^',
            // One mark, not a half. `▌` and `▐` said which of a cell's two
            // samples the cursor was on, which is real information and cost
            // more than it was worth: the mark flipped between the two halves
            // on every single keypress, and a cursor that jitters sideways as
            // you scrub reads as a fault in the program. The exact lag is in
            // the header, stated in seconds, where it can be read rather than
            // inferred from which half of a character is filled.
            _ => '▲',
        }
    }
}

/// The box-drawing glyph for a cell, given where the line enters and leaves.
///
/// The corners are what make this set worth having: `╭` and `╯` turn a staircase
/// of dashes into a stroke the eye follows without effort. They need the
/// direction of travel, which a bitmask cannot carry — hence a function of its
/// own rather than an arm of [`GlyphSet::stroke`].
pub fn box_glyph(from: f32, to: f32, row: usize, rows: usize) -> char {
    if rows == 0 {
        return ' ';
    }
    // `from` and `to` are fractions of the graph's `Scale`, so the ceiling has
    // already been divided out and a fitted axis needs no special case here.
    let place = |frac: f32| {
        let h = if frac.is_finite() {
            frac * rows as f32
        } else {
            0.0
        };
        rows - 1 - (h.clamp(0.0, rows as f32 - 0.001) as usize)
    };
    let (a, b) = (place(from), place(to));
    match (row == a, row == b) {
        // Flat: the line arrives and leaves at the same height.
        (true, true) => '─',
        // The cell's left end, where the line arrives from the previous cell,
        // so this corner always opens to the *left* and turns towards `b`.
        // `╰` and `╯` were the wrong way round here and in the arm below, which
        // drew a rise as two corners both opening left: a dead end above a dead
        // end, where the eye expects a step.
        (true, false) => {
            if a > b {
                // Rising: in from the left, out upwards.
                '╯'
            } else {
                // Falling: in from the left, out downwards.
                '╮'
            }
        }
        // The cell's right end, where the line leaves for the next cell, so
        // this corner always opens to the *right* and turns back towards `a`.
        (false, true) => {
            if a > b {
                // Rising: in from below, out to the right.
                '╭'
            } else {
                // Falling: in from above, out to the right.
                '╰'
            }
        }
        // Strictly between the two ends: the vertical part of the step.
        _ if (a.min(b)..=a.max(b)).contains(&row) => '│',
        _ => ' ',
    }
}

/// How much of one row a bar reaches, in sub-rows.
///
/// `frac` is where the value sits on the graph's [`Scale`] — 0.0 at the floor,
/// 1.0 at the ceiling. `row` counts from the top and `rows` is the height, so
/// the band this row covers is known; the answer is how far into it it climbs.
/// Zero means the bar is entirely below this row, `sub` that it is entirely
/// above.
///
/// A value with *any* presence in the row lights at least one sub-row. Rounding
/// it away would draw a running machine as a blank cell, which is the one thing
/// this graph must never say.
pub fn fill_in_row(frac: f32, row: usize, rows: usize, sub: usize) -> usize {
    if rows == 0 || sub == 0 || row >= rows {
        return 0;
    }
    let top = (rows * sub) as f32;
    let height = if frac.is_finite() {
        (frac * top).clamp(0.0, top)
    } else {
        0.0
    };
    let floor = ((rows - 1 - row) * sub) as f32;
    if height <= floor {
        return 0;
    }
    ((height - floor).ceil() as usize).min(sub)
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
pub fn rule_position(scale: Scale, pct: f32, rows: usize) -> Option<(usize, usize)> {
    let frac = scale.frac(pct);
    if !(0.0..=1.0).contains(&frac) || rows == 0 {
        return None;
    }
    let rows_f = rows as f32;
    for row in 0..rows {
        let high = (rows_f - row as f32) / rows_f;
        let low = (rows_f - row as f32 - 1.0) / rows_f;
        // Top row owns its upper bound so a threshold at the ceiling has
        // somewhere to go.
        let in_band = if row == 0 { frac <= high } else { frac < high };
        if in_band && frac >= low {
            // Must use the same mapping the bars use. With `ceil` here and
            // `round` there, the rule sat a dot above where a bar of the same
            // percentage lands: at six graph rows an 80% bar had to reach
            // 81.25% before it touched its own 80% line, while `heat_style`
            // already coloured it critical. Two signals, contradicting.
            let level = level_in_row_scaled(frac, row, rows, 1.0).max(1);
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
            let (row, level) = rule_position(Scale::zero(100.0), 80.0, rows).unwrap();
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
        let (row, level) = rule_position(Scale::zero(100.0), 80.0, 3).unwrap();
        assert_eq!(row, 0);
        assert!((1..=4).contains(&level));
        // 50% of a 2-row graph is the boundary between the bands. It resolves
        // to the bottom dot of the upper row, which is that boundary drawn.
        assert_eq!(rule_position(Scale::zero(100.0), 50.0, 2), Some((0, 1)));
        // Extremes stay inside the graph.
        assert_eq!(rule_position(Scale::zero(100.0), 100.0, 3).unwrap().0, 0);
        assert_eq!(rule_position(Scale::zero(100.0), 0.0, 3).unwrap().0, 2);
    }

    #[test]
    fn rule_position_refuses_the_impossible() {
        assert_eq!(rule_position(Scale::zero(100.0), 120.0, 3), None);
        assert_eq!(rule_position(Scale::zero(100.0), -1.0, 3), None);
        assert_eq!(rule_position(Scale::zero(100.0), 50.0, 0), None);
    }

    #[test]
    fn every_row_of_a_graph_can_hold_the_rule() {
        // Whatever the panel height, the threshold must land somewhere.
        for rows in 1..=8 {
            assert!(
                rule_position(Scale::zero(100.0), 80.0, rows).is_some(),
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
            let drawn = set.alphabet();
            for k in 1..=4 {
                let rule = set.rule_glyph(k);
                assert!(
                    !drawn.contains(&rule),
                    "{set:?} rules with {rule:?}, which is also one of its strokes"
                );
            }
        }
    }
    #[test]
    fn a_bar_resolves_every_level_its_set_claims() {
        // `sub_rows` is the set's whole resolution argument, and it is easy to
        // claim eight and draw two: the ramp is indexed by a level nothing
        // checks the spread of. A series climbing through one row's worth of
        // value has to pass through every height that row can draw, and each
        // has to be a different character.
        for set in [GlyphSet::Block, GlyphSet::Braille, GlyphSet::Ascii] {
            let sub = set.sub_rows();
            let seen: std::collections::BTreeSet<char> = (0..=100)
                .map(|k| {
                    // The bottom row of a three-row graph, so the whole sweep
                    // lands inside one cell.
                    let v = k as f32 / 100.0 * (100.0 / 3.0);
                    set.bar(fill_in_row(Scale::zero(100.0).frac(v), 2, 3, sub))
                })
                .collect();
            assert_eq!(
                seen.len(),
                sub + 1,
                "{set:?} claims {sub} levels a cell and drew {}: {seen:?}",
                seen.len()
            );
        }
    }

    #[test]
    fn the_default_is_braille_and_block_is_the_one_that_resolves_more() {
        // Both halves matter, and they pull against each other.
        //
        // `block` resolves eight levels in a cell against braille's four, and
        // for a while it was the default on exactly that argument. It is still
        // the true statement about resolution and it is not the whole question:
        // braille is the look this class of tool has had for a decade, it is
        // what the author wants to read, and a monitor nobody enjoys looking at
        // does not get looked at. That is a preference, and recording it as one
        // is more honest than inventing a metric it wins on.
        //
        // What braille does win on is the sparkline, where it packs two samples
        // into each of ten cells and so shows twice the history in the column
        // that has least room for it.
        assert_eq!(GlyphSet::default(), GlyphSet::Braille);
        assert!(
            GlyphSet::Block.sub_rows() > GlyphSet::Braille.sub_rows(),
            "the trade this comment describes has stopped being true"
        );
        assert_eq!(GlyphSet::Braille.spark_samples_per_cell(), 2);
        for other in [GlyphSet::Block, GlyphSet::Ascii, GlyphSet::Line] {
            assert_eq!(other.spark_samples_per_cell(), 1);
        }
    }

    #[test]
    fn a_running_machine_is_never_rounded_down_to_nothing() {
        // The floor case of "never a fabricated zero". A sample that reaches a
        // hundredth of the way into a row is not zero, and truncating it draws
        // a blank cell — which in this graph means "nothing was recorded", a
        // different and much stronger claim than "almost nothing happened".
        for set in [GlyphSet::Block, GlyphSet::Braille, GlyphSet::Ascii] {
            let sub = set.sub_rows();
            for v in [0.001f32, 0.01, 0.1, 1.0] {
                assert_eq!(
                    fill_in_row(Scale::zero(100.0).frac(v), 2, 3, sub),
                    1,
                    "{set:?} drew {v}% as {} sub-rows",
                    fill_in_row(Scale::zero(100.0).frac(v), 2, 3, sub)
                );
                assert_ne!(
                    set.bar(fill_in_row(Scale::zero(100.0).frac(v), 2, 3, sub)),
                    ' '
                );
            }
            // And an actual zero still draws nothing, or the distinction the
            // case above protects would be lost from the other side.
            assert_eq!(fill_in_row(Scale::zero(100.0).frac(0.0), 2, 3, sub), 0);
        }
    }
    #[test]
    fn a_high_narrow_band_gets_the_whole_panel() {
        // The case this exists for: memory between 72% and 85% on a panel that
        // spanned 0 to 100, so 72 of the 100 points were rows of ink that never
        // changed. The reclaimed rows are the whole feature.
        let s = Scale::pick(72.0, 85.0, 100.0, Axis::Fit);
        assert!(
            s.fitted,
            "a 13-point band on a 100-point axis was not fitted"
        );
        assert!(s.floor >= 70.0 && s.floor <= 72.0, "floor {}", s.floor);
        assert!(
            s.ceiling >= 85.0 && s.ceiling <= 90.0,
            "ceiling {}",
            s.ceiling
        );
        // The data lands inside the panel rather than on its edges, and uses
        // most of it: that ratio *is* the reclaimed resolution.
        assert!(s.frac(72.0) >= 0.0 && s.frac(85.0) <= 1.0);
        assert!(
            s.frac(85.0) - s.frac(72.0) > 0.6,
            "the band uses only {:.0}% of the panel",
            (s.frac(85.0) - s.frac(72.0)) * 100.0
        );
    }

    #[test]
    fn a_series_that_uses_its_range_is_left_at_zero() {
        // Fitting is for the wasted-panel case and nothing else. A series that
        // spans most of its axis is already spending its rows on the signal,
        // and moving its floor would truncate it for no gain — and, because the
        // form follows the axis, would turn honest bars into a line.
        for (min, max) in [(0.0f32, 95.0f32), (2.0, 60.0), (0.0, 40.0)] {
            let s = Scale::pick(min, max, ceiling_for(max), Axis::Fit);
            assert!(!s.fitted, "{min}..{max} was fitted");
            assert_eq!(s.floor, 0.0);
        }
    }

    #[test]
    fn a_series_that_does_not_move_is_left_at_zero() {
        // A flat series has no variation to reclaim rows for, and fitting it
        // gives a degenerate band — 22% drawn between 22 and 23, which reads as
        // a value pinned to a floor rather than as a value not moving. It also
        // made the axis label its own ceiling `23`, which is true and useless.
        for v in [2.0f32, 22.0, 78.0, 99.0] {
            let s = Scale::pick(v, v, ceiling_for(v), Axis::Fit);
            assert!(!s.fitted, "a flat {v}% was fitted to {s:?}");
        }
        // And the near-flat case, which is the same problem one step along:
        // a band too narrow to label distinguishably.
        assert!(!Scale::pick(78.0, 78.4, 100.0, Axis::Fit).fitted);
    }

    #[test]
    fn the_bounds_move_in_steps_so_the_axis_cannot_flap() {
        // The hysteresis. `min` and `max` slide continuously as the window
        // moves; if the bounds tracked them the axis would be relabelled every
        // frame — and because the form follows the axis, a series hovering at
        // the fitting threshold would alternate between bars and a line.
        // Rounding to a readable step is what stops both.
        let base = Scale::pick(72.0, 85.0, 100.0, Axis::Fit);
        for drift in [0.0f32, 0.3, 0.7, 1.1, 1.9] {
            let s = Scale::pick(72.0 + drift, 85.0 - drift, 100.0, Axis::Fit);
            assert_eq!(
                (s.floor, s.ceiling),
                (base.floor, base.ceiling),
                "a drift of {drift} moved the axis"
            );
        }
    }

    #[test]
    fn a_fitted_axis_is_never_drawn_as_bars() {
        // The honesty rule, and the reason the form is not a free choice. A
        // bar's area encodes its magnitude, so on an axis starting at 70 a bar
        // for 74 is a quarter the height of one for 85 — 74 rendered as a
        // quarter of 85. A line encodes change, for which a truncated axis is
        // both standard and honest.
        //
        // Stated here as the invariant rather than only in `glyph_row`, so the
        // next person to add a set has to satisfy it.
        let fitted = Scale::pick(72.0, 85.0, 100.0, Axis::Fit);
        assert!(fitted.fitted);
        for set in [
            GlyphSet::Block,
            GlyphSet::Braille,
            GlyphSet::Line,
            GlyphSet::Ascii,
        ] {
            let draws = if fitted.fitted {
                Draw::Line
            } else {
                set.draws()
            };
            assert_eq!(
                draws,
                Draw::Line,
                "{set:?} would draw a fitted axis as bars"
            );
        }
    }

    #[test]
    fn a_rule_is_never_spelled_like_the_series_in_the_form_being_drawn() {
        // `a_rule_is_never_spelled_like_the_series_it_rules` checks the set
        // against its own alphabet. It is not enough: a fitted axis is drawn as
        // a line in *every* set, so a `block` panel can be spelling its data in
        // box characters while still ruling with `block`'s `─` — which is the
        // box character for a flat stretch of series. Two marks, one glyph, and
        // the reference line reads as data.
        for set in [GlyphSet::Block, GlyphSet::Braille, GlyphSet::Ascii] {
            let effective = set.drawn_as(Draw::Line);
            let drawn = effective.alphabet();
            for k in 1..=4 {
                let rule = effective.rule_glyph(k);
                assert!(
                    !drawn.contains(&rule),
                    "{set:?} drawing a fitted axis rules with {rule:?}, \
                     which is one of the strokes it draws"
                );
            }
        }
    }
    /// Which sides of its cell a box glyph connects: (left, right, up, down).
    fn sides(c: char) -> (bool, bool, bool, bool) {
        match c {
            '─' => (true, true, false, false),
            '│' => (false, false, true, true),
            '╭' => (false, true, false, true),
            '╮' => (true, false, false, true),
            '╰' => (false, true, true, false),
            '╯' => (true, false, true, false),
            ' ' => (false, false, false, false),
            other => panic!("{other:?} is not a box glyph"),
        }
    }

    #[test]
    fn a_line_has_no_loose_ends() {
        // `╰` and `╯` were the wrong way round, so a rise drew as two corners
        // both opening left — a dead end above a dead end, where the eye
        // expects a step. Every test passed with them swapped, because every
        // test asked which characters appeared and none asked whether they
        // joined up.
        //
        // The invariant is connectivity: if a glyph opens downwards, the glyph
        // below it must open upwards, and a glyph that opens left or right must
        // meet one that opens back. A line that does not join is not a line.
        let rows = 4;
        for (from, to) in [
            (0.1f32, 0.9f32),
            (0.9, 0.1),
            (0.5, 0.5),
            (0.2, 0.4),
            (0.95, 0.05),
        ] {
            let column: Vec<char> = (0..rows).map(|r| box_glyph(from, to, r, rows)).collect();
            for r in 0..rows {
                let (_, _, up, down) = sides(column[r]);
                if down {
                    assert!(
                        r + 1 < rows && sides(column[r + 1]).2,
                        "{from}->{to}: row {r} ({:?}) opens down onto nothing",
                        column[r]
                    );
                }
                if up {
                    assert!(
                        r > 0 && sides(column[r - 1]).3,
                        "{from}->{to}: row {r} ({:?}) opens up onto nothing",
                        column[r]
                    );
                }
            }
            // And the column enters on the left at `from`'s row and leaves on
            // the right at `to`'s row, or the step joins nothing horizontally.
            let place = |f: f32| rows - 1 - ((f * rows as f32) as usize).min(rows - 1);
            assert!(
                sides(column[place(from)]).0,
                "{from}->{to}: nothing arrives from the previous cell"
            );
            assert!(
                sides(column[place(to)]).1,
                "{from}->{to}: nothing leaves for the next cell"
            );
        }
    }
}
