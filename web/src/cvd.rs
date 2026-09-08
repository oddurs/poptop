//! Colour-vision simulation, for showing the reader what the measurement means.
//!
//! The landing page claims that green-and-yellow collapses into one colour for
//! roughly 8% of men and that poptop's palette does not. Quoting two numbers
//! asks a reader with normal colour vision to take that on faith, and tells a
//! reader who has a deficiency about their own experience in a footnote. So the
//! page simulates it instead, on real swatches and on a real frame.
//!
//! **This is a second copy of `src/cvd.rs` and that is a hazard**, because the
//! whole point of the tool's version is that CI enforces what it measures. The
//! matrices, the sRGB linearisation and the OKLab conversion below are copied
//! verbatim from it, and `the_simulation_agrees_with_the_tool` pins the outputs
//! against the figures that module's own tests assert — 3.7 for the old
//! green↔yellow pair under protanopia, and a worst pair above 8 for the shipped
//! palette. If the tool's palette or its arithmetic moves, that test fails here
//! too, rather than the site quietly showing a picture of something that is no
//! longer true.
//!
//! Simulation happens here rather than in the browser for the same reason:
//! there would otherwise be a third implementation, in a language with no test
//! comparing it to the first two.

/// Machado, Oliveira & Fernandes (2009), severity 1.0, linear RGB. Copied from
/// `src/cvd.rs`; see the module note.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Vision {
    Normal,
    Protan,
    Deutan,
    /// Not offered as a state on the page — tritanopia is rare and the section
    /// is already four controls wide — but folded into [`worst_across`],
    /// because the tool's own guard includes it and the figures quoted on the
    /// page come from that guard.
    Tritan,
    Mono,
}

impl Vision {
    /// The states the reader can switch between.
    pub const ALL: [Vision; 4] = [Vision::Normal, Vision::Protan, Vision::Deutan, Vision::Mono];
    /// Everything [`worst_across`] folds in. Normal vision is included for the
    /// reason `src/cvd.rs` gives: the tritan matrix has entries above 1, so a
    /// simulation can *increase* separation, and a pair too close for everyone
    /// could pass a deficiency-only check.
    pub const DEFICIENCIES: [Vision; 4] = [
        Vision::Protan,
        Vision::Deutan,
        Vision::Tritan,
        Vision::Normal,
    ];

    /// The key used in the page's JSON and in `data-vision`.
    pub fn key(self) -> &'static str {
        match self {
            Vision::Normal => "normal",
            Vision::Protan => "protan",
            Vision::Deutan => "deutan",
            Vision::Tritan => "tritan",
            Vision::Mono => "mono",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Vision::Normal => "Normal",
            Vision::Protan => "Protanopia",
            Vision::Deutan => "Deuteranopia",
            Vision::Tritan => "Tritanopia",
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
            Vision::Tritan => "Reduced sensitivity to short wavelengths. Rare.",
            Vision::Mono => {
                "No hue at all. Not a simulation of a deficiency — the proof that the \
                 thresholds and glyphs were carrying the meaning the whole time."
            }
        }
    }

    fn matrix(self) -> Option<[[f64; 3]; 3]> {
        match self {
            Vision::Normal | Vision::Mono => None,
            Vision::Tritan => Some([
                [1.255528, -0.076749, -0.178779],
                [-0.078411, 0.930809, 0.147602],
                [0.004733, 0.691367, 0.303900],
            ]),
            Vision::Protan => Some([
                [0.152286, 1.052583, -0.204868],
                [0.114503, 0.786281, 0.099216],
                [-0.003882, -0.048116, 1.051998],
            ]),
            Vision::Deutan => Some([
                [0.367322, 0.860646, -0.227968],
                [0.280085, 0.672501, 0.047413],
                [-0.011820, 0.042940, 0.968881],
            ]),
        }
    }
}

fn srgb_to_linear(c: f64) -> f64 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(c: f64) -> f64 {
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

fn linear(rgb: [u8; 3]) -> [f64; 3] {
    [
        srgb_to_linear(f64::from(rgb[0]) / 255.0),
        srgb_to_linear(f64::from(rgb[1]) / 255.0),
        srgb_to_linear(f64::from(rgb[2]) / 255.0),
    ]
}

fn oklab([r, g, b]: [f64; 3]) -> [f64; 3] {
    let l = (0.412_221_470_8 * r + 0.536_332_536_3 * g + 0.051_445_992_9 * b).cbrt();
    let m = (0.211_903_498_2 * r + 0.680_699_545_1 * g + 0.107_396_956_6 * b).cbrt();
    let s = (0.088_302_461_9 * r + 0.281_718_837_6 * g + 0.629_978_700_5 * b).cbrt();
    [
        0.210_454_255_3 * l + 0.793_617_785_0 * m - 0.004_072_046_8 * s,
        1.977_998_495_1 * l - 2.428_592_205_0 * m + 0.450_593_709_9 * s,
        0.025_904_037_1 * l + 0.782_771_766_2 * m - 0.808_675_766_0 * s,
    ]
}

fn simulate_linear(lin: [f64; 3], vision: Vision) -> [f64; 3] {
    match vision.matrix() {
        None => lin,
        Some(m) => {
            let mut out = [0.0; 3];
            for (i, row) in m.iter().enumerate() {
                out[i] = (row[0] * lin[0] + row[1] * lin[1] + row[2] * lin[2]).clamp(0.0, 1.0);
            }
            out
        }
    }
}

/// What `hex` looks like to this reader, as `#rrggbb`.
///
/// Monochrome is relative luminance rather than a matrix: it is what a
/// greyscale display or a monochrome terminal tier actually does, and the whole
/// claim being demonstrated is that meaning survives it.
pub fn simulate(hex: &str, vision: Vision) -> String {
    let rgb = parse(hex);
    let out = if vision == Vision::Mono {
        let lin = linear(rgb);
        let y = 0.2126 * lin[0] + 0.7152 * lin[1] + 0.0722 * lin[2];
        [y, y, y]
    } else {
        simulate_linear(linear(rgb), vision)
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
    let prep = |hex: &str| {
        let lin = linear(parse(hex));
        oklab(if vision == Vision::Mono {
            let y = 0.2126 * lin[0] + 0.7152 * lin[1] + 0.0722 * lin[2];
            [y, y, y]
        } else {
            simulate_linear(lin, vision)
        })
    };
    let (x, y) = (prep(a), prep(b));
    100.0 * ((x[0] - y[0]).powi(2) + (x[1] - y[1]).powi(2) + (x[2] - y[2]).powi(2)).sqrt()
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

/// The worst separation between two colours across normal vision and every
/// deficiency — the figure `src/cvd.rs` calls `worst_cvd`, and the one the
/// README and this page quote.
pub fn worst_across(a: &str, b: &str) -> f64 {
    Vision::DEFICIENCIES
        .iter()
        .map(|&v| delta_e(a, b, v))
        .fold(f64::INFINITY, f64::min)
}

/// Relative luminance, the WCAG definition.
fn luminance(hex: &str) -> f64 {
    let l = linear(parse(hex));
    0.2126 * l[0] + 0.7152 * l[1] + 0.0722 * l[2]
}

/// WCAG contrast ratio between two hexes.
pub fn contrast(a: &str, b: &str) -> f64 {
    let (x, y) = (luminance(a), luminance(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
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

    /// The figures `src/cvd.rs` asserts in its own tests, reproduced here. This
    /// is the check that the copy has not drifted from the original — see the
    /// module note for why a copy exists at all.
    /// The three figures `src/cvd.rs` pins in `reproduces_the_published_figures`,
    /// reproduced here from the same inputs. This is the check that the copy has
    /// not drifted from the original — see the module note for why a copy exists
    /// at all. Same tolerance the tool uses.
    #[test]
    fn the_simulation_agrees_with_the_tool() {
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

        // And the shipped palette clears the floor the tool enforces.
        let shipped = ["#5ccfe6", "#ffd580", "#ff6666", "#7a7ae6", "#b48ead"];
        for vision in [Vision::Protan, Vision::Deutan] {
            let worst = worst_pair(&shipped, vision);
            assert!(
                worst >= 8.0,
                "{} worst pair is {worst:.2}, under the floor of 8",
                vision.label()
            );
        }
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
                        "{} on {chip} ({}) is {ratio:.2}:1",
                        ink,
                        vision.label()
                    );
                }
            }
        }
    }

    #[test]
    fn normal_vision_changes_nothing() {
        assert_eq!(simulate("#5ccfe6", Vision::Normal), "#5ccfe6");
        assert_eq!(simulate("#ff6666", Vision::Normal), "#ff6666");
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

    #[test]
    fn a_hex_survives_a_round_trip_through_normal_vision() {
        for hex in ["#000000", "#ffffff", "#101317", "#b48ead"] {
            assert_eq!(simulate(hex, Vision::Normal), hex);
        }
    }
}
