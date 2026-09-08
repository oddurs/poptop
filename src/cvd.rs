//! Colour-vision arithmetic, so palette claims are checked rather than asserted.
//!
//! A palette regresses the moment somebody nudges a hex by eye. The separation
//! between two colours is arithmetic, so it belongs in the build rather than in
//! a reviewer's judgement — this module is what lets `theme.rs`'s tests fail a
//! palette instead of merely describing one.
//!
//! Method, chosen to match the tooling the shipped palette was measured with so
//! the figures in `theme.rs` are reproducible from here:
//!
//! - sRGB → linear → OKLab (Ottosson's coefficients).
//! - Colour-vision deficiency via Machado, Oliveira & Fernandes (2009) at
//!   severity 1.0, applied in linear RGB.
//! - ΔE is Euclidean distance in OKLab, ×100.
//!
//! Naming the model matters: an independent check of an earlier candidate under
//! Viénot 1999 gave 7.95 where Machado gave 9.1 — one side of the threshold
//! each. A ΔE quoted without its model is not a reproducible number.

use ratatui::style::Color;

mod math;

// Re-exported: what the rest of the tool actually calls. `delta_e` and the
// conversions stay behind `math::`, reached by name where they are needed.
pub use math::{CVD_TARGET, Cvd, contrast, worst_cvd};

/// The sRGB behind a `Color`, where one exists.
///
/// `None` for the ANSI-16 slots on purpose: their appearance is chosen by the
/// user's terminal theme, so there is no value here to measure. That is a real
/// limit of that tier rather than a gap in this module, and the tests state it
/// rather than quietly skipping.
pub fn to_rgb(c: Color) -> Option<[u8; 3]> {
    match c {
        Color::Rgb(r, g, b) => Some([r, g, b]),
        // Indices 0-15 are the same sixteen theme-defined slots as the named
        // variants, just spelled differently. Returning a fabricated value for
        // them would let a palette expressed as `Indexed(9)` slip past the
        // unmeasurability check and be "validated" against a colour the
        // terminal actually chooses.
        Color::Indexed(i) if i >= 16 => Some(xterm256(i)),
        _ => None,
    }
}

/// The measurable part of the xterm 256 palette: the 6×6×6 cube and the greys.
///
/// Indices below 16 are deliberately absent rather than approximated — they are
/// theme-defined, so any value here would be fiction. [`to_rgb`] filters them
/// out before this is called.
fn xterm256(i: u8) -> [u8; 3] {
    const CUBE: [u8; 6] = [0, 95, 135, 175, 215, 255];
    debug_assert!(i >= 16, "index {i} is a theme-defined slot, not measurable");
    match i {
        0..=15 => [0, 0, 0],
        16..=231 => {
            let n = i - 16;
            [
                CUBE[(n / 36) as usize],
                CUBE[((n % 36) / 6) as usize],
                CUBE[(n % 6) as usize],
            ]
        }
        232..=255 => {
            let v = 8 + 10 * (i as u16 - 232);
            [v as u8, v as u8, v as u8]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::math::delta_e;
    use super::*;

    /// Within a tenth of a ΔE unit is close enough to call the port faithful.
    fn near(a: f64, b: f64) -> bool {
        (a - b).abs() < 0.15
    }

    #[test]
    fn reproduces_the_published_figures() {
        // These are the numbers quoted in theme.rs and the README, produced by
        // the reference validator. If this port drifts, every claim in the
        // docs silently stops being true — so pin them here.
        let green = [0x00, 0xcd, 0x00];
        let yellow = [0xcd, 0xcd, 0x00];
        let d = delta_e(green, yellow, Some(Cvd::Protan));
        assert!(
            near(d, 3.7),
            "poptop's old green<->yellow under protanopia: got {d:.2}, expected 3.7"
        );

        let cyan = [0x5c, 0xcf, 0xe6];
        let amber = [0xff, 0xd5, 0x80];
        let d = delta_e(cyan, amber, Some(Cvd::Protan));
        assert!(near(d, 16.2), "safe ok<->warn: got {d:.2}, expected 16.2");

        let crit = [0xff, 0x66, 0x66];
        let mem = [0xb4, 0x8e, 0xad];
        let d = worst_cvd(crit, mem).0;
        assert!(near(d, 10.3), "safe crit<->mem: got {d:.2}, expected 10.3");
    }

    #[test]
    fn identical_colours_are_zero_apart() {
        let c = [0x5c, 0xcf, 0xe6];
        assert_eq!(delta_e(c, c, None), 0.0);
        for k in Cvd::ALL {
            assert_eq!(delta_e(c, c, Some(k)), 0.0);
        }
    }

    #[test]
    fn xterm_cube_and_greys_resolve() {
        // Cube corners and the grey ramp ends, checked against the xterm spec.
        assert_eq!(xterm256(16), [0, 0, 0]);
        assert_eq!(xterm256(231), [255, 255, 255]);
        assert_eq!(xterm256(232), [8, 8, 8]);
        assert_eq!(xterm256(255), [238, 238, 238]);
        // Index 80 is the safe palette's `ok`. It quantises to #5fd7d7 — red is
        // the low channel, which is what makes it read as cyan.
        let [r, g, b] = xterm256(80);
        assert_eq!([r, g, b], [95, 215, 215]);
        assert!(
            r < g && r < b,
            "index 80 should read as a cyan, got {r},{g},{b}"
        );
    }

    #[test]
    fn indexed_and_rgb_colours_both_resolve() {
        assert_eq!(to_rgb(Color::Rgb(1, 2, 3)), Some([1, 2, 3]));
        assert_eq!(to_rgb(Color::Indexed(16)), Some([0, 0, 0]));
    }

    #[test]
    fn the_low_indices_are_refused_like_their_named_twins() {
        // Indexed(9) is Color::Red spelled differently; both are chosen by the
        // terminal theme, so neither has a value to measure.
        for i in 0..16u8 {
            assert_eq!(
                to_rgb(Color::Indexed(i)),
                None,
                "Indexed({i}) is theme-defined and must not claim a value"
            );
        }
        assert!(to_rgb(Color::Indexed(16)).is_some());
    }

    #[test]
    fn worst_cvd_also_accounts_for_normal_vision() {
        // Tritan simulation can widen a pair, so a CVD-only fold could pass a
        // pair that nobody can tell apart.
        let a = [0x80, 0x80, 0x80];
        let b = [0x82, 0x82, 0x82];
        assert!(worst_cvd(a, b).0 <= delta_e(a, b, None) + 1e-9);
    }

    #[test]
    fn contrast_matches_known_ratios() {
        assert!((contrast([255, 255, 255], [0, 0, 0]) - 21.0).abs() < 0.01);
        assert!((contrast([0, 0, 0], [0, 0, 0]) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn ansi_slots_have_no_measurable_value() {
        // Not an oversight: the terminal theme picks these, so there is nothing
        // here to measure. The tier's own docs say it can promise nothing.
        for c in [
            Color::Green,
            Color::Yellow,
            Color::Red,
            Color::Cyan,
            Color::Reset,
        ] {
            assert_eq!(to_rgb(c), None, "{c:?} should not claim a measurable value");
        }
    }
}
