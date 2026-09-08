//! The arithmetic behind every palette claim, with no dependencies at all.
//!
//! Split out of `cvd.rs` so the website can `include!` it. The site simulates
//! colour-vision deficiency on its landing page to show what the measurement
//! means, and it used to do that with a hand-copy of these matrices. The copy
//! had a test pinning the published figures, which sounds like a guard and is
//! not one: it computed those figures from the copy's own coefficients, so a
//! change here would leave the site simulating a poptop that no longer exists
//! while its test went on passing.
//!
//! Nothing in this file may reference `crate::` or any dependency, or the
//! include stops working. That constraint is the whole point — it is what keeps
//! the arithmetic portable enough to have exactly one home.
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

/// Target separation for adjacent meaning-bearing colours, OKLab ΔE×100.
pub const CVD_TARGET: f64 = 8.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cvd {
    Protan,
    Deutan,
    Tritan,
}

impl Cvd {
    pub const ALL: [Cvd; 3] = [Cvd::Protan, Cvd::Deutan, Cvd::Tritan];

    /// Machado, Oliveira & Fernandes (2009), severity 1.0, linear RGB.
    fn matrix(self) -> [[f64; 3]; 3] {
        match self {
            Cvd::Protan => [
                [0.152286, 1.052583, -0.204868],
                [0.114503, 0.786281, 0.099216],
                [-0.003882, -0.048116, 1.051998],
            ],
            Cvd::Deutan => [
                [0.367322, 0.860646, -0.227968],
                [0.280085, 0.672501, 0.047413],
                [-0.011820, 0.042940, 0.968881],
            ],
            Cvd::Tritan => [
                [1.255528, -0.076749, -0.178779],
                [-0.078411, 0.930809, 0.147602],
                [0.004733, 0.691367, 0.303900],
            ],
        }
    }
}

pub fn srgb_to_linear(c: f64) -> f64 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

pub fn linear(rgb: [u8; 3]) -> [f64; 3] {
    [
        srgb_to_linear(rgb[0] as f64 / 255.0),
        srgb_to_linear(rgb[1] as f64 / 255.0),
        srgb_to_linear(rgb[2] as f64 / 255.0),
    ]
}

pub fn oklab([r, g, b]: [f64; 3]) -> [f64; 3] {
    let l = (0.412_221_470_8 * r + 0.536_332_536_3 * g + 0.051_445_992_9 * b).cbrt();
    let m = (0.211_903_498_2 * r + 0.680_699_545_1 * g + 0.107_396_956_6 * b).cbrt();
    let s = (0.088_302_461_9 * r + 0.281_718_837_6 * g + 0.629_978_700_5 * b).cbrt();
    [
        0.210_454_255_3 * l + 0.793_617_785_0 * m - 0.004_072_046_8 * s,
        1.977_998_495_1 * l - 2.428_592_205_0 * m + 0.450_593_709_9 * s,
        0.025_904_037_1 * l + 0.782_771_766_2 * m - 0.808_675_766_0 * s,
    ]
}

pub fn simulate(lin: [f64; 3], kind: Cvd) -> [f64; 3] {
    let m = kind.matrix();
    let mut out = [0.0; 3];
    for (i, row) in m.iter().enumerate() {
        out[i] = (row[0] * lin[0] + row[1] * lin[1] + row[2] * lin[2]).clamp(0.0, 1.0);
    }
    out
}

/// OKLab ΔE ×100. `kind` of `None` is unsimulated (normal) vision.
pub fn delta_e(a: [u8; 3], b: [u8; 3], kind: Option<Cvd>) -> f64 {
    let prep = |c: [u8; 3]| {
        let lin = linear(c);
        oklab(match kind {
            Some(k) => simulate(lin, k),
            None => lin,
        })
    };
    let (x, y) = (prep(a), prep(b));
    100.0 * ((x[0] - y[0]).powi(2) + (x[1] - y[1]).powi(2) + (x[2] - y[2]).powi(2)).sqrt()
}

/// The worst separation between `a` and `b` across normal vision and all three
/// deficiencies.
///
/// Normal vision is included deliberately. The tritan matrix has entries above
/// 1, so a simulation can *increase* separation — meaning a pair that is too
/// close for everyone could pass a CVD-only check. Folding unsimulated vision
/// in makes the guard as wide as the claim it backs.
/// Returns the vision it belongs to alongside the figure. `None` there means
/// normal vision was the worst case — not a curiosity, but exactly the case a
/// deficiency-only check would miss, and a report naming a number without its
/// subject is half a report.
pub fn worst_cvd(a: [u8; 3], b: [u8; 3]) -> (f64, Option<Cvd>) {
    Cvd::ALL
        .iter()
        .map(|&k| (delta_e(a, b, Some(k)), Some(k)))
        .chain(std::iter::once((delta_e(a, b, None), None)))
        .fold(
            (f64::INFINITY, None),
            |acc, x| {
                if x.0 < acc.0 { x } else { acc }
            },
        )
}

/// WCAG relative luminance contrast ratio. Used to check a colour is legible on
/// the backgrounds it is actually drawn over, which ΔE between hues cannot say.
pub fn contrast(a: [u8; 3], b: [u8; 3]) -> f64 {
    let rel = |c: [u8; 3]| {
        let l = linear(c);
        0.2126 * l[0] + 0.7152 * l[1] + 0.0722 * l[2]
    };
    let (x, y) = (rel(a), rel(b));
    let (hi, lo) = if x > y { (x, y) } else { (y, x) };
    (hi + 0.05) / (lo + 0.05)
}
