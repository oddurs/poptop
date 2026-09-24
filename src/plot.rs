//! Marks, coverage, and the surfaces that fit glyphs to them.
//!
//! The drawing code used to know what braille was. `pairs_in_a_cell` existed
//! because braille's dot columns are independent and block's are not;
//! `fill_in_row` knew about sub-rows; the line mode picked box-drawing corners
//! by inspecting its neighbours while the fill mode picked a level per column.
//! Four sets, three special cases, and nothing shared but the scale.
//!
//! So: a mark is rasterized into **subcell coverage**, and a surface fits a
//! glyph to it (0244).
//!
//! ```text
//! series → scale → marks → subcell coverage → fit → cells
//! ```
//!
//! A surface declares two things — how many subcells a cell holds, and which
//! patterns it has a glyph for. Everything else is shared, so a new set is a
//! table rather than a branch, and the pixel tier is another surface rather
//! than another renderer.
//!
//! **Coverage is geometry, never data.** A subcell is lit because the mark
//! passes through it, and the rule that decides "any presence lights at least
//! one subcell" lives in [`levels_in_row`] — where it was before, so a running
//! machine is never drawn as a blank cell.

use std::sync::LazyLock;

/// What a surface is being asked to draw.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mark {
    /// A column of ink from the floor up to the value.
    Area,
    /// The outline alone, joined across cells.
    Stroke,
}

/// A stroke passing through one cell, in the graph's own fractions: 0.0 at the
/// floor of the scale, 1.0 at the ceiling.
#[derive(Clone, Copy, Debug)]
pub struct Stroke {
    pub from: f32,
    pub to: f32,
    /// Which row of the graph this cell is, counting from the top.
    pub row: usize,
    pub rows: usize,
}

/// A surface: a subcell geometry, and an alphabet to fit to it.
pub trait Surface: Sync {
    /// Subcells a character cell holds: columns, then rows.
    fn sub(&self) -> (usize, usize);

    /// One cell of an area, given how far the fill climbs in each subcolumn —
    /// `0..=sub().1`, counted from the bottom.
    fn fill(&self, levels: &[usize]) -> char;

    /// One cell of a stroke.
    fn stroke(&self, s: Stroke) -> char;

    /// Whether this surface can carry a mark at all. A set with solid fills
    /// and no line joints carries areas and hands strokes to the next set
    /// down (0246).
    fn carries(&self, mark: Mark) -> bool;

    /// A reference mark at `level` of four, for a cell the data does not reach.
    fn rule(&self, level: usize) -> char;

    /// The character this surface draws an empty cell with. Not always a
    /// space: braille's blank is `U+2800`, a real character with no dots
    /// raised, and comparing against `' '` meant no cell was ever empty.
    fn blank(&self) -> char {
        self.fill(&[0, 0])
    }

    fn name(&self) -> &'static str;
}

/// How far a value climbs into one row of a graph, in subcells.
///
/// `frac` is where the value sits on the scale — 0.0 at the floor, 1.0 at the
/// ceiling. `row` counts from the top. Zero means the mark is entirely below
/// this row, `sub` that it is entirely above.
///
/// A value with *any* presence in the row lights at least one subcell.
/// Rounding it away would draw a running machine as a blank cell, which is the
/// one thing this graph must never say.
pub fn levels_in_row(frac: f32, row: usize, rows: usize, sub: usize) -> usize {
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

/// Which subcell row a fraction lands in, counting from the top of the graph.
fn place(frac: f32, rows: usize, sub: usize) -> usize {
    let cells = (rows * sub).max(1);
    let h = if frac.is_finite() {
        frac * cells as f32
    } else {
        0.0
    };
    cells - 1 - (h.clamp(0.0, cells as f32 - 0.001) as usize)
}

// ── surfaces ────────────────────────────────────────────────────────────────

/// A surface whose cells are a vertical ramp: one column, many levels.
///
/// The block elements (`▁▂▃▄▅▆▇█`) and ascii (`_-#`). No horizontal subcells,
/// so a cell shows the peak of the samples in it — which is what their
/// alphabets can say.
pub struct Ramp {
    name: &'static str,
    /// Index 0 is empty, index `len - 1` is a full cell.
    steps: &'static [char],
    rule: char,
    /// Whether this surface has box drawing for its strokes.
    boxes: bool,
    /// Whether it carries areas. Box drawing has no part-height forms, so a
    /// set built on it draws strokes and leaves areas to a set that can.
    fills: bool,
}

impl Surface for Ramp {
    fn sub(&self) -> (usize, usize) {
        (1, self.steps.len() - 1)
    }

    fn fill(&self, levels: &[usize]) -> char {
        let k = levels.iter().copied().max().unwrap_or(0);
        self.steps[k.min(self.steps.len() - 1)]
    }

    fn stroke(&self, s: Stroke) -> char {
        let c = box_glyph(s.from, s.to, s.row, s.rows);
        if self.boxes {
            return c;
        }
        // Ascii has no box drawing and keeps its own marks throughout.
        match c {
            '─' => '-',
            '│' => '|',
            '╭' | '╮' => '.',
            '╰' | '╯' => '\'',
            other => other,
        }
    }

    fn carries(&self, mark: Mark) -> bool {
        mark == Mark::Stroke || self.fills
    }

    fn rule(&self, _: usize) -> char {
        self.rule
    }

    fn name(&self) -> &'static str {
        self.name
    }
}

/// A surface with a glyph for every pattern of lit subcells.
///
/// Braille, quadrants and sextants. The table is the whole definition: a
/// lookup from pattern to codepoint, where bit `x * h + k` is the subcell in
/// column `x`, `k` rows up from the bottom.
pub struct Patterned {
    name: &'static str,
    w: usize,
    h: usize,
    table: &'static LazyLock<Vec<char>>,
    rule: &'static (dyn Fn(usize) -> char + Sync),
    strokes: bool,
}

impl Patterned {
    /// The glyph for a raw pattern, for the tests that check a table against
    /// the codepoints Unicode assigned.
    #[cfg(test)]
    pub fn pattern(&self, bits: u32) -> char {
        self.glyph(bits)
    }

    fn glyph(&self, bits: u32) -> char {
        self.table
            .get(bits as usize)
            .copied()
            .unwrap_or_else(|| self.table[0])
    }
}

impl Surface for Patterned {
    fn sub(&self) -> (usize, usize) {
        (self.w, self.h)
    }

    fn fill(&self, levels: &[usize]) -> char {
        let mut bits = 0u32;
        for x in 0..self.w {
            // One sample to a subcolumn where there is one, and the peak
            // across the cell where the surface has only one column. That is
            // `pairs_in_a_cell` as geometry rather than as a special case: a
            // sample owns a column from the moment it is drawn until it
            // scrolls off, so the only column that ever changes is the newest.
            let level = if levels.len() >= self.w {
                levels[x]
            } else {
                levels.iter().copied().max().unwrap_or(0)
            };
            for k in 0..level.min(self.h) {
                bits |= 1 << (x * self.h + k);
            }
        }
        self.glyph(bits)
    }

    fn stroke(&self, s: Stroke) -> char {
        if !self.strokes {
            return box_glyph(s.from, s.to, s.row, s.rows);
        }
        // The segment across this cell, rasterized into the subgrid: for each
        // subcolumn, the span of subcells the line passes through. Joins need
        // no special case — the cell either side lights the subcells its own
        // half of the segment reaches, and they meet at the edge (0248).
        let mut bits = 0u32;
        let top = self.h * s.rows;
        let (a, b) = (
            place(s.from, s.rows, self.h) as isize,
            place(s.to, s.rows, self.h) as isize,
        );
        let base = (s.row * self.h) as isize;
        for x in 0..self.w {
            // Where the segment sits at the left and right edge of this
            // subcolumn, in subcell rows across the whole graph.
            let t0 = x as f32 / self.w as f32;
            let t1 = (x + 1) as f32 / self.w as f32;
            let at = |t: f32| a as f32 + (b - a) as f32 * t;
            let (lo, hi) = {
                let (p, q) = (at(t0), at(t1));
                (p.min(q), p.max(q))
            };
            let (lo, hi) = (lo.floor() as isize, hi.ceil() as isize - 1);
            for y in lo..=hi {
                if y < base || y >= base + self.h as isize || y < 0 || y >= top as isize {
                    continue;
                }
                let k = self.h - 1 - (y - base) as usize;
                bits |= 1 << (x * self.h + k);
            }
        }
        self.glyph(bits)
    }

    fn carries(&self, mark: Mark) -> bool {
        match mark {
            Mark::Area => true,
            Mark::Stroke => self.strokes,
        }
    }

    fn rule(&self, level: usize) -> char {
        (self.rule)(level)
    }

    fn name(&self) -> &'static str {
        self.name
    }
}

// ── tables ──────────────────────────────────────────────────────────────────

/// Braille cells are `U+2800` plus a dot bitmask:
///
/// ```text
///   dot1 0x01   dot4 0x08
///   dot2 0x02   dot5 0x10
///   dot3 0x04   dot6 0x20
///   dot7 0x40   dot8 0x80
/// ```
///
/// Bars fill upward, so subcell `k` rows from the bottom is the `k`th dot from
/// the bottom of its column. Derived rather than transcribed: the rule is one
/// line and it is impossible to get a single entry subtly wrong.
const DOTS: [[u32; 4]; 2] = [[0x40, 0x04, 0x02, 0x01], [0x80, 0x20, 0x10, 0x08]];

static BRAILLE: LazyLock<Vec<char>> = LazyLock::new(|| {
    (0..256u32)
        .map(|bits| {
            let mut dots = 0u32;
            for (x, column) in DOTS.iter().enumerate() {
                // `DOTS` is listed bottom-up, and so is `k`.
                for (k, dot) in column.iter().enumerate() {
                    if bits & (1 << (x * 4 + k)) != 0 {
                        dots |= dot;
                    }
                }
            }
            char::from_u32(0x2800 + dots).unwrap_or(' ')
        })
        .collect()
});

/// Quadrants: a 2×2 cell, every pattern already in the Block Elements.
static QUADRANT: LazyLock<Vec<char>> = LazyLock::new(|| {
    // Indexed by (top-left, top-right, bottom-left, bottom-right) as bits of
    // this module's layout: bit `x * 2 + k`, `k` up from the bottom.
    const BY_CORNERS: [char; 16] = [
        ' ', '▖', '▘', '▌', '▗', '▄', '▚', '▙', '▝', '▞', '▀', '▛', '▐', '▟', '▜', '█',
    ];
    (0..16usize).map(|bits| BY_CORNERS[bits]).collect()
});

/// Sextants: a 2×3 cell, from Symbols for Legacy Computing (Unicode 13).
///
/// `U+1FB00` onwards holds every pattern *except* the four that were already
/// encoded — blank, the two half-width columns, and full — so the codepoint is
/// the pattern's index among the rest. Unicode numbers the subcells 1..6 from
/// the top left, reading across; this module numbers them up from the bottom,
/// so the two layouts are converted rather than transcribed.
static SEXTANT: LazyLock<Vec<char>> = LazyLock::new(|| {
    let unicode_order = |bits: usize| -> usize {
        let mut n = 0usize;
        for x in 0..2 {
            for k in 0..3 {
                if bits & (1 << (x * 3 + k)) != 0 {
                    // Row from the top, then column: digits 1..6.
                    let digit = (2 - k) * 2 + x;
                    n |= 1 << digit;
                }
            }
        }
        n
    };
    (0..64usize)
        .map(|bits| {
            let n = unicode_order(bits);
            match n {
                0 => ' ',
                // Left column, right column, and full: encoded before the
                // sextants were, as half blocks.
                0b010101 => '▌',
                0b101010 => '▐',
                0b111111 => '█',
                _ => {
                    let skipped = [0b010101, 0b101010].iter().filter(|&&s| s < n).count();
                    char::from_u32(0x1FB00 + (n - 1 - skipped) as u32).unwrap_or(' ')
                }
            }
        })
        .collect()
});

fn braille_rule(level: usize) -> char {
    let k = level.clamp(1, 4);
    char::from_u32(0x2800 + (DOTS[0][k - 1] | DOTS[1][k - 1])).unwrap_or(' ')
}

// ── the sets ────────────────────────────────────────────────────────────────

pub static BRAILLE_SURFACE: Patterned = Patterned {
    name: "braille",
    w: 2,
    h: 4,
    table: &BRAILLE,
    rule: &braille_rule,
    strokes: true,
};

pub static SEXTANT_SURFACE: Patterned = Patterned {
    name: "sextant",
    w: 2,
    h: 3,
    table: &SEXTANT,
    // A dashed reference, so it is never the solid run the series draws.
    rule: &|_| '┄',
    strokes: true,
};

pub static QUADRANT_SURFACE: Patterned = Patterned {
    name: "quadrant",
    w: 2,
    h: 2,
    table: &QUADRANT,
    rule: &|_| '┄',
    strokes: true,
};

pub static BLOCK_SURFACE: Ramp = Ramp {
    name: "block",
    steps: &[' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'],
    rule: '─',
    boxes: true,
    fills: true,
};

pub static ASCII_SURFACE: Ramp = Ramp {
    name: "ascii",
    steps: &[' ', '_', '-', '#'],
    // `~`, which is the only mark left: ascii draws values with ` _ - #` and a
    // rule spelled like any of them reads as a sample.
    rule: '~',
    boxes: false,
    fills: true,
};

/// Box drawing: one level a row, and the cleanest stroke of the sets — the
/// corners are what turn a staircase of dashes into a line the eye follows.
pub static LINE_SURFACE: Ramp = Ramp {
    name: "line",
    steps: &[' ', '─'],
    // Dashed, because `─` is this surface's own stroke and a solid rule would
    // be the character the series draws.
    rule: '┄',
    boxes: true,
    fills: false,
};

/// The alphabet a one-column set uses when a cell must carry two samples.
///
/// Coarser than its bar on purpose: ` . : | #` are marks that read as a pair
/// of heights rather than as one, which is what the sparkline needs from a set
/// with no subcolumns.
pub static ASCII_PAIRED: Ramp = Ramp {
    name: "ascii",
    steps: &[' ', '.', ':', '|', '#'],
    rule: '~',
    boxes: false,
    fills: true,
};

/// One cell of an outline, given where the line enters and leaves.
pub fn box_glyph(from: f32, to: f32, row: usize, rows: usize) -> char {
    if rows == 0 {
        return ' ';
    }
    let (a, b) = (place(from, rows, 1), place(to, rows, 1));
    match (row == a, row == b) {
        // Flat: the line arrives and leaves at the same height.
        (true, true) => '─',
        // The cell's left end, where the line arrives from the previous cell,
        // so this corner always opens to the left and turns towards `b`.
        (true, false) => {
            if a > b {
                '╯'
            } else {
                '╮'
            }
        }
        // The cell's right end, where the line leaves for the next cell.
        (false, true) => {
            if a > b {
                '╰'
            } else {
                '╭'
            }
        }
        // Strictly between the two ends: the vertical part of the step.
        _ if (a.min(b)..=a.max(b)).contains(&row) => '│',
        _ => ' ',
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every glyph a surface can draw, for the width and collision checks the
    /// chrome already runs on its own marks.
    fn alphabet(s: &dyn Surface) -> Vec<char> {
        let (w, h) = s.sub();
        let mut out = Vec::new();
        for a in 0..=h {
            for b in 0..=h {
                out.push(s.fill(&[a, b][..w.min(2)]));
            }
        }
        for from in [0.0f32, 0.5, 1.0] {
            for to in [0.0f32, 0.5, 1.0] {
                for row in 0..3 {
                    out.push(s.stroke(Stroke {
                        from,
                        to,
                        row,
                        rows: 3,
                    }));
                }
            }
        }
        out.extend((1..=4).map(|k| s.rule(k)));
        out
    }

    fn surfaces() -> Vec<&'static dyn Surface> {
        vec![
            &BRAILLE_SURFACE,
            &SEXTANT_SURFACE,
            &QUADRANT_SURFACE,
            &BLOCK_SURFACE,
            &ASCII_SURFACE,
            &LINE_SURFACE,
        ]
    }

    #[test]
    fn every_glyph_a_surface_draws_is_one_column_wide() {
        // A glyph a terminal draws two columns wide runs the graph past its
        // panel. This is the chrome's own rule, applied to the alphabets.
        for s in surfaces() {
            for c in alphabet(s) {
                use unicode_width::UnicodeWidthStr as _;
                assert_eq!(
                    c.to_string().width(),
                    1,
                    "{}: {c:?} (U+{:04X}) is not one column",
                    s.name(),
                    c as u32
                );
            }
        }
    }

    #[test]
    fn a_table_has_a_glyph_for_every_pattern() {
        // A missing entry is tofu on somebody's screen, and the fill is a
        // lookup rather than a branch precisely so this can be checked.
        for (s, table) in [
            (&BRAILLE_SURFACE, &BRAILLE),
            (&SEXTANT_SURFACE, &SEXTANT),
            (&QUADRANT_SURFACE, &QUADRANT),
        ] {
            let (w, h) = s.sub();
            assert_eq!(table.len(), 1 << (w * h), "{}", s.name());
            assert!(
                table.iter().all(|c| *c != '\u{fffd}'),
                "{} has a replacement character in its table",
                s.name()
            );
        }
    }

    #[test]
    fn the_sextant_table_is_the_one_unicode_encoded() {
        // Checked against the codepoints Unicode 13 assigned, by the names it
        // gave them: `BLOCK SEXTANT-N…` numbers the subcells 1..6 from the top
        // left, reading across. The derivation here counts up from the bottom,
        // so this is the conversion as much as the table.
        let bit = |x: usize, k: usize| 1u32 << (x * 3 + k);
        // SEXTANT-1: top left alone.
        assert_eq!(SEXTANT_SURFACE.pattern(bit(0, 2)), '\u{1FB00}');
        // SEXTANT-5: bottom left alone.
        assert_eq!(SEXTANT_SURFACE.pattern(bit(0, 0)), '\u{1FB0F}');
        // SEXTANT-56: the bottom row.
        assert_eq!(SEXTANT_SURFACE.pattern(bit(0, 0) | bit(1, 0)), '\u{1FB2D}');
        // SEXTANT-1256: both ends of both columns.
        assert_eq!(
            SEXTANT_SURFACE.pattern(bit(0, 0) | bit(0, 2) | bit(1, 0) | bit(1, 2)),
            '\u{1FB30}'
        );
        // The four that were encoded before the sextants were.
        let fill = |l: usize, r: usize| SEXTANT_SURFACE.fill(&[l, r]);
        assert_eq!(fill(0, 0), ' ');
        assert_eq!(fill(3, 3), '█');
        assert_eq!(fill(3, 0), '▌');
        assert_eq!(fill(0, 3), '▐');
    }

    #[test]
    fn a_fill_climbs_from_the_bottom() {
        // The table was flipped once, and every glyph was still a legal
        // braille cell — so the sets agreed with each other and disagreed with
        // the machine. A bar of one lights the bottom subcell, in every set
        // that has more than one.
        for s in [
            &BRAILLE_SURFACE as &dyn Surface,
            &SEXTANT_SURFACE,
            &QUADRANT_SURFACE,
        ] {
            let (_, h) = s.sub();
            assert_eq!(
                s.fill(&[1, 0]),
                match s.name() {
                    "braille" => '\u{2840}',
                    "sextant" => '\u{1FB0F}',
                    _ => '▖',
                },
                "{} lights the wrong end first",
                s.name()
            );
            assert_ne!(s.fill(&[1, 0]), s.fill(&[h, 0]), "{}", s.name());
        }
    }

    #[test]
    fn a_pattern_surface_gives_each_sample_its_own_column() {
        // The property the scroll depends on: a sample owns a column, so the
        // only column that changes is the newest.
        for s in [&BRAILLE_SURFACE, &SEXTANT_SURFACE, &QUADRANT_SURFACE] {
            let (w, h) = s.sub();
            assert_eq!(w, 2, "{}", s.name());
            assert_ne!(
                s.fill(&[h, 0]),
                s.fill(&[0, h]),
                "{}: the two columns draw the same",
                s.name()
            );
        }
    }

    #[test]
    fn a_ramp_shows_the_peak_of_what_it_was_given() {
        // One subcolumn, so the cell cannot draw two samples. Showing the peak
        // is what the alphabet can say, and hiding a spike behind its
        // neighbour is what it must not do.
        assert_eq!(BLOCK_SURFACE.fill(&[8, 0]), '█');
        assert_eq!(BLOCK_SURFACE.fill(&[0, 8]), '█');
        assert_eq!(ASCII_SURFACE.fill(&[0, 3]), '#');
    }

    #[test]
    fn any_presence_in_a_row_lights_a_subcell() {
        // A running machine is never a blank cell.
        for sub in [1usize, 2, 3, 4, 8] {
            assert_eq!(levels_in_row(0.0, 2, 3, sub), 0);
            assert!(levels_in_row(0.0001, 2, 3, sub) >= 1);
            assert_eq!(levels_in_row(1.0, 0, 3, sub), sub);
        }
    }

    #[test]
    fn a_stroke_covers_the_cells_it_passes_through() {
        // A rise across one cell lights subcells at both ends, and nothing in
        // the rows it never reaches.
        let s = &BRAILLE_SURFACE;
        let rising = s.stroke(Stroke {
            from: 0.0,
            to: 1.0,
            row: 2,
            rows: 3,
        });
        assert_ne!(rising, s.blank(), "the stroke drew nothing in its own row");
        let above = s.stroke(Stroke {
            from: 0.0,
            to: 0.2,
            row: 0,
            rows: 3,
        });
        assert_eq!(
            above,
            s.blank(),
            "the stroke drew in a row it never entered"
        );
    }
}
