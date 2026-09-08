//! The favicon and the social card, drawn rather than stored.
//!
//! Both are SVG, and both are built from the same tokens as the rest of the
//! site, so there is no binary asset to regenerate when a colour changes.
//!
//! A caveat worth writing down: not every link scraper rasterises SVG. The
//! ones that do not fall back to showing no image, which is the failure this
//! is willing to accept in exchange for having no build step and no stored
//! bitmap that can silently go stale. If a raster card becomes necessary,
//! render this file to `assets/og.png` and point `og:image` at it.

use maud::{Markup, PreEscaped};

const CANVAS: &str = "#101317";
const INK: &str = "#ccd2d9";
const FAINT: &str = "#5e6873";
const CPU: &str = "#7a7ae6";
const OK: &str = "#5ccfe6";

/// The mark: a rising plot in a cell, which is what the tool is.
pub fn favicon() -> Markup {
    let mut bars = String::new();
    for (i, h) in [4u32, 7, 5, 11, 16, 13, 22, 19].into_iter().enumerate() {
        let x = 4 + i as u32 * 3;
        let colour = if h > 15 { OK } else { CPU };
        bars.push_str(&format!(
            r#"<rect x="{x}" y="{}" width="2" height="{h}" fill="{colour}"/>"#,
            28 - h
        ));
    }
    PreEscaped(format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32">
<rect width="32" height="32" rx="4" fill="{CANVAS}"/>{bars}</svg>"#
    ))
}

/// The social card. 1200×630, the size every scraper expects.
pub fn card(tagline: &str) -> Markup {
    // A plot across the lower half, the same shape the landing page's buffer
    // makes: quiet, a climb, saturation, a slower recovery.
    let mut bars = String::new();
    for i in 0..96u32 {
        let t = f64::from(i) / 95.0;
        let surge = if t < 0.42 {
            0.10 + (t * 9.0).sin().abs() * 0.06
        } else if t < 0.68 {
            0.10 + ((t - 0.42) / 0.26).powf(2.2) * 0.86
        } else {
            0.96 - ((t - 0.68) / 0.32).powf(0.8) * 0.80
        };
        let h = (surge * 150.0).max(4.0);
        let x = 96 + i * 11;
        let colour = if surge > 0.8 { OK } else { CPU };
        bars.push_str(&format!(
            r#"<rect x="{x}" y="{y:.0}" width="7" height="{h:.0}" fill="{colour}" opacity="{o}"/>"#,
            y = 470.0 - h,
            o = if surge > 0.8 { "1" } else { "0.85" }
        ));
    }

    PreEscaped(format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="1200" height="630" viewBox="0 0 1200 630" role="img" aria-label="poptop — {tagline}">
<rect width="1200" height="630" fill="{CANVAS}"/>
<g font-family="ui-monospace, SFMono-Regular, Menlo, monospace">
  <text x="96" y="150" fill="{FAINT}" font-size="26">$ poptop</text>
  <text x="96" y="252" fill="{INK}" font-size="76" font-weight="600" letter-spacing="-3">{tagline}</text>
  <text x="96" y="322" fill="{FAINT}" font-size="28">Every sample kept, process table and all.</text>
  <text x="96" y="530" fill="{FAINT}" font-size="24">100</text>
  <text x="96" y="562" fill="{FAINT}" font-size="24">CPU</text>
  <text x="1104" y="562" fill="{FAINT}" font-size="24" text-anchor="end">&#8592;/&#8594; scrub</text>
</g>
<line x1="96" y1="482" x2="1104" y2="482" stroke="#242a33" stroke-width="2"/>
<line x1="96" y1="396" x2="1104" y2="396" stroke="#242a33" stroke-width="2" stroke-dasharray="6 10"/>
{bars}
</svg>"##
    ))
}
