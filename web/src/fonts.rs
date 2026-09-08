//! The one webfont, embedded.
//!
//! Instrument Sans carries every word a person wrote. The monospace is
//! deliberately *not* a webfont — see `assets/css/10-tokens.css` for why — so
//! this is the whole of the site's font loading.
//!
//! The files are `include_bytes!`d, served under a URL containing a hash of
//! their own bytes, and cached forever. The `@font-face` rules are generated
//! here from the same `.range` files the subsets were downloaded with, rather
//! than written out by hand in the stylesheet: a `unicode-range` that disagrees
//! with the bytes it points at fails silently, as a glyph that quietly comes
//! from the fallback face.

use std::sync::LazyLock;

pub struct Face {
    /// Stable name, used to build the URL.
    pub slug: &'static str,
    pub bytes: &'static [u8],
    pub style: &'static str,
    /// Trimmed at compile time from the file the subset was fetched with.
    pub range: &'static str,
}

macro_rules! face {
    ($slug:literal, $file:literal, $style:literal) => {
        Face {
            slug: $slug,
            bytes: include_bytes!(concat!("../assets/fonts/", $file)),
            style: $style,
            range: include_str!(concat!("../assets/fonts/", $file, ".range")),
        }
    };
}

pub static FACES: LazyLock<[Face; 4]> = LazyLock::new(|| {
    [
        face!(
            "instrument-sans-latin",
            "instrument-sans-latin-normal.woff2",
            "normal"
        ),
        face!(
            "instrument-sans-latin-italic",
            "instrument-sans-latin-italic.woff2",
            "italic"
        ),
        face!(
            "instrument-sans-latin-ext",
            "instrument-sans-latin-ext-normal.woff2",
            "normal"
        ),
        face!(
            "instrument-sans-latin-ext-italic",
            "instrument-sans-latin-ext-italic.woff2",
            "italic"
        ),
    ]
});

impl Face {
    pub fn path(&self) -> String {
        format!(
            "/assets/{}.{}.woff2",
            self.slug,
            crate::assets::hash_bytes(self.bytes)
        )
    }
}

/// The subset the first paint needs: unaccented Latin, upright. It is the one
/// worth a `<link rel=preload>`; the other three would compete with it for
/// bandwidth to serve characters most pages never use.
pub fn primary() -> &'static Face {
    &FACES[0]
}

/// The `@font-face` block, prepended to the stylesheet bundle.
///
/// `font-display: swap` on purpose. The fallback is a system sans already on
/// the machine, so the first paint is readable text rather than three hundred
/// milliseconds of nothing — and the metric overrides below mean swapping to
/// the real face does not move the line, so `swap` costs no layout shift.
pub static CSS: LazyLock<String> = LazyLock::new(|| {
    let mut out = String::new();
    // Instrument Sans' own metrics, scaled so a system sans standing in for it
    // occupies the same box. Without this the swap reflows every paragraph on
    // the page, which is most of a bad Cumulative Layout Shift score and all
    // of the visible jolt.
    out.push_str(
        "@font-face {\n  \
         font-family: 'Instrument Sans Fallback';\n  \
         src: local('Helvetica Neue'), local('Arial'), local('Segoe UI'), local('Roboto');\n  \
         ascent-override: 96%;\n  \
         descent-override: 24%;\n  \
         line-gap-override: 0%;\n  \
         size-adjust: 102%;\n}\n",
    );
    for face in FACES.iter() {
        out.push_str(&format!(
            "@font-face {{\n  \
             font-family: 'Instrument Sans';\n  \
             font-style: {style};\n  \
             font-weight: 400 700;\n  \
             font-display: swap;\n  \
             src: url({path}) format('woff2');\n  \
             unicode-range: {range};\n}}\n",
            style = face.style,
            path = face.path(),
            range = face.range.trim()
        ));
    }
    out
});

pub fn find(path: &str) -> Option<&'static Face> {
    FACES.iter().find(|f| f.path() == path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_face_is_a_woff2_and_not_an_error_page() {
        for face in FACES.iter() {
            // wOF2, the woff2 magic number. A 404 page saved over a font file
            // is a real way to break this and an invisible one.
            assert_eq!(&face.bytes[..4], b"wOF2", "{} is not woff2", face.slug);
            assert!(face.bytes.len() > 8_000, "{} looks truncated", face.slug);
        }
    }

    #[test]
    fn the_generated_rules_point_at_the_faces_that_are_served() {
        let css = CSS.clone();
        for face in FACES.iter() {
            assert!(
                css.contains(&face.path()),
                "{} is not in the css",
                face.slug
            );
            assert!(find(&face.path()).is_some());
        }
        // Four real subsets plus the metric-matched fallback.
        assert_eq!(css.matches("@font-face").count(), FACES.len() + 1);
        assert!(css.contains("Instrument Sans Fallback"));
        assert!(css.contains("size-adjust"));
    }

    /// The fallback is only reached through the font stack, and the stack is
    /// written in the stylesheet rather than here. If the two ever stop naming
    /// the same family the override silently does nothing.
    #[test]
    fn the_fallback_family_is_named_in_the_font_stack() {
        let tokens = include_str!("../assets/css/10-tokens.css");
        assert!(tokens.contains("\"Instrument Sans Fallback\""));
    }

    /// The ranges come from the files the subsets were fetched with. If one
    /// were empty the browser would decide the face covers nothing and silently
    /// use the fallback for every character.
    #[test]
    fn every_face_declares_a_unicode_range() {
        for face in FACES.iter() {
            assert!(face.range.trim().starts_with("U+"), "{}", face.slug);
        }
    }
}
