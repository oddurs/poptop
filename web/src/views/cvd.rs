//! The colour-vision section: the measurement, made operable.
//!
//! Every colour on this page is simulated on the server by `crate::cvd`, which
//! is pinned against the tool's own `src/cvd.rs`. The browser only swaps
//! between palettes it was handed — there is no third implementation of the
//! Machado matrices to disagree with the other two.
//!
//! Both palettes stay on screen in every state, because the argument is a
//! comparison. Showing only poptop's would prove nothing: a reader would have
//! no idea whether *any* palette survives protanopia.

use crate::cvd::{self, Vision};
use maud::{Markup, html};

/// The convention every system monitor ships, and what poptop ships instead.
/// These are the ANSI hues — the ones poptop itself used before the palette was
/// measured, and the ones `src/cvd.rs` pins its published 3.7 figure against.
const CONVENTION: [(&str, &str); 3] = [
    ("#00cd00", "ok"),
    ("#cdcd00", "warn"),
    ("#cd0000", "critical"),
];

const POPTOP: [(&str, &str); 3] = [
    ("#5ccfe6", "ok"),
    ("#ffd580", "warn"),
    ("#ff6666", "critical"),
];

/// A glyph rides with every status, which is the whole reason the monochrome
/// state still reads.
const GLYPHS: [&str; 3] = ["✓", "△", "✖"];

pub fn demo() -> Markup {
    html! {
        div class="cvd" data-cvd data-vision="normal" {
            div class="cvd__controls" role="radiogroup" aria-label="Simulate colour vision" {
                @for vision in Vision::ALL {
                    button type="button" class="cvd__option" role="radio"
                        data-vision-set=(vision.key())
                        aria-checked=(if vision == Vision::Normal { "true" } else { "false" }) {
                        (vision.label())
                    }
                }
            }

            p class="cvd__note" data-cvd-note aria-live="polite" { (Vision::Normal.note()) }

            div class="cvd__rows" {
                (row("The convention", "green · yellow · red", &CONVENTION, "convention"))
                (row("poptop's default", "cyan · amber · red", &POPTOP, "poptop"))
            }

            // The same simulation, on the thing it actually matters to. In the
            // monochrome state this is the whole argument: every hue is gone
            // and the plot still reads, because the thresholds are dashed rules
            // and the statuses carry glyphs.
            div class="cvd__frame" data-cvd-frame {
                (crate::views::ui::still(crate::views::ui::Still {
                    cursor: 232,
                    rows: 4,
                    head: true,
                    ..Default::default()
                }))
            }

            p class="caption" {
                "Worst pair, OKLab ΔE×100 — the closest two colours in each set, which is
                 what decides whether a palette is usable. Below 8 they are one colour.
                 In monochrome the figure is withheld: there are no hues left to
                 separate, and what that state shows is the glyphs and the threshold
                 rules doing the work instead."
            }
        }

        // Every palette, in every state, computed on the server. The browser
        // swaps between them; it never simulates anything itself.
        script type="application/json" id="cvd-palettes" { (data()) }
    }
}

fn row(title: &str, tag: &str, palette: &[(&str, &str); 3], key: &str) -> Markup {
    let worst = cvd::worst_pair(
        &palette.iter().map(|(hex, _)| *hex).collect::<Vec<_>>(),
        Vision::Normal,
    );
    html! {
        div class="cvd__row" data-cvd-row=(key) {
            div class="cvd__meta" {
                p class="cvd__title" { (title) }
                p class="cvd__tag" { (tag) }
            }
            div class="cvd__chips" {
                @for (i, (hex, name)) in palette.iter().enumerate() {
                    div class="cvd__chip" data-swatch=(format!("{key}-{i}"))
                        style=(format!("--chip: {hex}; --chip-ink: {}", cvd::ink_on(hex))) {
                        span class="cvd__glyph" aria-hidden="true" { (GLYPHS[i]) }
                        span class="cvd__name" { (name) }
                    }
                }
            }
            p class="cvd__delta" {
                span class="cvd__delta-value" data-delta=(key) { (format!("{worst:.1}")) }
                span class="cvd__delta-label" { "ΔE" }
            }
        }
    }
}

/// `{ vision: { row: { chips: [hex,…], delta: n } } }`, plus the frame's own
/// hues, so the browser can restyle everything from one lookup.
fn data() -> Markup {
    let mut out = String::from("{");
    for (v, vision) in Vision::ALL.iter().enumerate() {
        if v > 0 {
            out.push(',');
        }
        out.push_str(&format!("\"{}\":{{", vision.key()));
        out.push_str(&format!("\"note\":\"{}\",", vision.note()));

        for (name, palette) in [("convention", &CONVENTION), ("poptop", &POPTOP)] {
            let hexes: Vec<String> = palette
                .iter()
                .map(|(hex, _)| cvd::simulate(hex, *vision))
                .collect();
            let inks: Vec<String> = hexes.iter().map(|h| cvd::ink_on(h).to_string()).collect();
            // ΔE between hues is meaningless once there are no hues. In the
            // monochrome state the figure is withheld rather than reported as a
            // failure — what that state demonstrates is the glyphs and the
            // threshold rules, not a separation the palette never claimed.
            let delta = if *vision == Vision::Mono {
                "—".to_string()
            } else {
                let worst = cvd::worst_pair(
                    &palette.iter().map(|(hex, _)| *hex).collect::<Vec<_>>(),
                    *vision,
                );
                format!("{worst:.1}")
            };
            let quote = |v: &Vec<String>| {
                v.iter()
                    .map(|h| format!("\"{h}\""))
                    .collect::<Vec<_>>()
                    .join(",")
            };
            out.push_str(&format!(
                "\"{name}\":{{\"chips\":[{}],\"inks\":[{}],\"delta\":\"{delta}\"}},",
                quote(&hexes),
                quote(&inks)
            ));
        }

        // The frame's own series and status hues, so the plot beside the
        // swatches is simulated too rather than staying honest-looking while
        // the swatches change.
        let frame: Vec<String> = ["#5ccfe6", "#ffd580", "#ff6666", "#7a7ae6", "#b48ead"]
            .iter()
            .map(|hex| format!("\"{}\"", cvd::simulate(hex, *vision)))
            .collect();
        out.push_str(&format!("\"frame\":[{}]", frame.join(",")));
        out.push('}');
    }
    out.push('}');
    maud::PreEscaped(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The section's whole claim, asserted where it is rendered rather than
    /// only where it is computed.
    #[test]
    fn the_json_carries_every_state() {
        let json = data().0;
        for vision in Vision::ALL {
            assert!(
                json.contains(&format!("\"{}\"", vision.key())),
                "{}",
                vision.key()
            );
        }
        assert!(json.contains("\"convention\""));
        assert!(json.contains("\"poptop\""));
        assert!(!json.contains('\n'), "the JSON island must be one line");
    }

    #[test]
    fn the_convention_falls_below_the_floor_and_poptop_does_not() {
        let convention: Vec<&str> = CONVENTION.iter().map(|(h, _)| *h).collect();
        let poptop: Vec<&str> = POPTOP.iter().map(|(h, _)| *h).collect();
        assert!(cvd::worst_pair(&convention, Vision::Protan) < 8.0);
        assert!(cvd::worst_pair(&poptop, Vision::Protan) >= 8.0);
    }
}
