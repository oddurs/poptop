//! Colour-vision simulation, for showing the reader what the measurement means.
//!
//! The landing page claims that green-and-yellow collapses into one colour for
//! roughly 8% of men and that poptop's palette does not. Quoting two numbers
//! asks a reader with normal colour vision to take that on faith, and tells a
//! reader who has a deficiency about their own experience in a footnote. So the
//! page simulates it instead, on real swatches and on a real frame.
//!
//! **The arithmetic is the tool's, not a copy of it.** Everything below the
//! include is a thin layer over `src/cvd/math.rs`: hexes instead of byte
//! triples, a `Vision` that has a monochrome state the tool's `Cvd` does not
//! need, and a simulation that returns a colour rather than a distance.
//!
//! This used to be a hand-copy of the matrices with a test pinning the
//! published figures. That test looked like a guard and was not one — it
//! computed 3.7 from the copy's own coefficients, so changing the tool would
//! have left this file simulating a poptop that no longer existed while the
//! test went on passing. It could only ever catch drift in the copy, which is
//! the direction that does not matter.
//!
//! Simulation happens here rather than in the browser for the same reason:
//! there would otherwise be a third implementation, in a language with no test
//! comparing it to the other two.

// This line is the point of the whole module. `src/cvd/math.rs` has no
// dependencies and no `crate::` references precisely so it can be compiled into
// both crates, and there is now exactly one copy of the matrices in the
// repository. A path dependency would also have worked and would have pointed
// from the site to the tool — which adds nothing to `cargo install poptop`,
// because dependencies point one way — but this is simpler and cannot drift.
#[path = "../../src/cvd/math.rs"]
#[allow(dead_code)]
mod math;

use math::Cvd;

/// What the reader is looking through. Four states, of which two are the tool's
/// deficiencies, one is unsimulated, and one — monochrome — is not a deficiency
/// at all but the proof that the thresholds and glyphs were carrying the
/// meaning the whole time.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Vision {
    Normal,
    Protan,
    Deutan,
    Mono,
}

impl Vision {
    /// The states the reader can switch between.
    pub const ALL: [Vision; 4] = [Vision::Normal, Vision::Protan, Vision::Deutan, Vision::Mono];

    /// The tool's deficiency for this state, where there is one.
    fn cvd(self) -> Option<Cvd> {
        match self {
            Vision::Protan => Some(Cvd::Protan),
            Vision::Deutan => Some(Cvd::Deutan),
            Vision::Normal | Vision::Mono => None,
        }
    }

    /// The key used in the page's JSON and in `data-vision`.
    pub fn key(self) -> &'static str {
        match self {
            Vision::Normal => "normal",
            Vision::Protan => "protan",
            Vision::Deutan => "deutan",
            Vision::Mono => "mono",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Vision::Normal => "Normal",
            Vision::Protan => "Protanopia",
            Vision::Deutan => "Deuteranopia",
            Vision::Mono => "Monochrome",
        }
    }

    /// One line on what the reader is looking at.
    pub fn note(self) -> &'static str {
        match self {
            Vision::Normal => "Unsimulated. Both palettes separate cleanly.",
            Vision::Protan => {
                "Reduced sensitivity to long wavelengths — the commonest form, and the \
                 one green-and-yellow fails hardest."
            }
            Vision::Deutan => "Reduced sensitivity to medium wavelengths. The most common form.",
            Vision::Mono => {
                "No hue at all. Not a simulation of a deficiency — the proof that the \
                 thresholds and glyphs were carrying the meaning the whole time."
            }
        }
    }
}

/// Linear light back to sRGB. The tool only ever goes the other way, because it
/// measures distances and never has to name a colour.
fn linear_to_srgb(c: f64) -> f64 {
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

/// Relative luminance, which monochrome is and contrast needs.
fn luminance(rgb: [u8; 3]) -> f64 {
    let l = math::linear(rgb);
    0.2126 * l[0] + 0.7152 * l[1] + 0.0722 * l[2]
}

/// What `hex` looks like to this reader, as `#rrggbb`.
///
/// Monochrome is relative luminance rather than a matrix: it is what a
/// greyscale display or a monochrome terminal tier actually does, and the whole
/// claim being demonstrated is that meaning survives it.
pub fn simulate(hex: &str, vision: Vision) -> String {
    let rgb = parse(hex);
    let out = if vision == Vision::Mono {
        let y = luminance(rgb);
        [y, y, y]
    } else {
        match vision.cvd() {
            Some(kind) => math::simulate(math::linear(rgb), kind),
            None => math::linear(rgb),
        }
    };
    let byte = |c: f64| (linear_to_srgb(c).clamp(0.0, 1.0) * 255.0).round() as u8;
    format!(
        "#{:02x}{:02x}{:02x}",
        byte(out[0]),
        byte(out[1]),
        byte(out[2])
    )
}

/// OKLab ΔE ×100 between two hexes, as this reader would see them.
pub fn delta_e(a: &str, b: &str, vision: Vision) -> f64 {
    if vision == Vision::Mono {
        // The tool has no monochrome tier to measure, because monochrome is a
        // rendering mode rather than a vision. Simulate to grey and compare.
        return math::delta_e(
            parse(&simulate(a, vision)),
            parse(&simulate(b, vision)),
            None,
        );
    }
    math::delta_e(parse(a), parse(b), vision.cvd())
}

/// The worst separation between two colours across normal vision and every
/// deficiency — the figure the tool calls `worst_cvd`, and the one the README
/// and this page quote.
///
/// Tritanopia is in that figure and is not one of the states the reader can
/// switch to: it is rare, and the control is already four wide. The tool owns
/// the list, which is why `Vision` no longer names it.
pub fn worst_across(a: &str, b: &str) -> f64 {
    math::worst_cvd(parse(a), parse(b)).0
}

/// The closest pair in a palette, which is the figure that decides whether it
/// is usable: a palette is only as separable as its worst two colours.
pub fn worst_pair(palette: &[&str], vision: Vision) -> f64 {
    let mut worst = f64::INFINITY;
    for (i, a) in palette.iter().enumerate() {
        for b in &palette[i + 1..] {
            worst = worst.min(delta_e(a, b, vision));
        }
    }
    worst
}

/// WCAG contrast ratio between two hexes.
pub fn contrast(a: &str, b: &str) -> f64 {
    math::contrast(parse(a), parse(b))
}

/// Black or white on `hex`, whichever actually contrasts better.
///
/// The chips carry their status name on every simulated colour, including a
/// monochrome mid-grey where neither choice is comfortable. An earlier version
/// picked by a luminance threshold and put white on saturated green at 2.16:1 —
/// a caption about legibility that could not be read. Comparing the two ratios
/// is both simpler and always right.
pub fn ink_on(hex: &str) -> &'static str {
    if contrast(hex, "#000000") >= contrast(hex, "#ffffff") {
        "#000000"
    } else {
        "#ffffff"
    }
}

fn parse(hex: &str) -> [u8; 3] {
    let h = hex.trim_start_matches('#');
    let byte = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).unwrap_or(0);
    [byte(0), byte(2), byte(4)]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The three figures `src/cvd.rs` pins in `reproduces_the_published_figures`,
    /// reproduced here from the same inputs *through the same code*. It can no
    /// longer drift, so this is now a check that the include still reaches the
    /// arithmetic rather than a check on a copy of it — which is worth keeping,
    /// because the include is a path and paths break quietly.
    #[test]
    fn the_simulation_reaches_the_tool_arithmetic() {
        let near = |a: f64, b: f64| (a - b).abs() < 0.15;

        let d = delta_e("#00cd00", "#cdcd00", Vision::Protan);
        assert!(
            near(d, 3.7),
            "old green<->yellow under protanopia: {d:.2}, expected 3.7"
        );

        let d = delta_e("#5ccfe6", "#ffd580", Vision::Protan);
        assert!(
            near(d, 16.2),
            "safe ok<->warn under protanopia: {d:.2}, expected 16.2"
        );

        let d = worst_across("#ff6666", "#b48ead");
        assert!(
            near(d, 10.3),
            "safe critical<->mem, worst case: {d:.2}, expected 10.3"
        );

        let shipped = ["#5ccfe6", "#ffd580", "#ff6666", "#7a7ae6", "#b48ead"];
        for vision in [Vision::Protan, Vision::Deutan] {
            let worst = worst_pair(&shipped, vision);
            assert!(
                worst >= math::CVD_TARGET,
                "{} worst pair is {worst:.2}, under the floor",
                vision.label()
            );
        }
    }

    /// The floor the page quotes is the tool's constant, not a literal typed
    /// into the copy that used to live here.
    #[test]
    fn the_floor_comes_from_the_tool() {
        assert_eq!(math::CVD_TARGET, 8.0);
    }

    #[test]
    fn normal_vision_changes_nothing() {
        for hex in ["#000000", "#ffffff", "#101317", "#b48ead", "#5ccfe6"] {
            assert_eq!(simulate(hex, Vision::Normal), hex);
        }
    }

    /// The point of the monochrome state: every hue becomes the same kind of
    /// thing, so anything still readable is not being carried by colour.
    #[test]
    fn monochrome_removes_hue_entirely() {
        for hex in ["#5ccfe6", "#ffd580", "#ff6666", "#7a7ae6"] {
            let grey = simulate(hex, Vision::Mono);
            let rgb = parse(&grey);
            assert_eq!(rgb[0], rgb[1], "{hex} kept a hue: {grey}");
            assert_eq!(rgb[1], rgb[2], "{hex} kept a hue: {grey}");
        }
    }

    /// The claim the section is built on: the convention collapses and poptop's
    /// palette does not.
    #[test]
    fn the_convention_collapses_and_the_default_does_not() {
        let convention = ["#00cd00", "#cdcd00", "#cd0000"];
        let default = ["#5ccfe6", "#ffd580", "#ff6666"];
        let a = worst_pair(&convention, Vision::Protan);
        let b = worst_pair(&default, Vision::Protan);
        assert!(a < 8.0, "the convention should fail, got {a:.2}");
        assert!(
            b > a * 2.0,
            "the default should be far clearer: {b:.2} vs {a:.2}"
        );
    }

    /// Every chip label clears 4.5:1 on its own chip, in every state. This
    /// section is an argument about legibility; a caption on it that cannot be
    /// read would be an own goal.
    #[test]
    fn every_chip_label_is_legible_on_its_chip() {
        let palettes = [
            ["#00cd00", "#cdcd00", "#cd0000"],
            ["#5ccfe6", "#ffd580", "#ff6666"],
        ];
        for vision in Vision::ALL {
            for palette in palettes {
                for hex in palette {
                    let chip = simulate(hex, vision);
                    let ink = ink_on(&chip);
                    let ratio = contrast(&chip, ink);
                    assert!(
                        ratio >= 4.5,
                        "{ink} on {chip} ({}) is {ratio:.2}:1",
                        vision.label()
                    );
                }
            }
        }
    }
}
