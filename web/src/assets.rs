//! The asset pipeline, which is a strong word for concatenation and a hash.
//!
//! There is no bundler, no minifier and no npm. The CSS is seven files
//! concatenated in cascade-layer order at compile time; the JS is two. Both are
//! served under a URL containing a hash of their own bytes, so they can be
//! cached forever and still change the instant they change.
//!
//! The hash is FNV-1a. It is not a security primitive and is not being used as
//! one — it decides a filename — and reaching for `sha2` to name a stylesheet
//! would put a dependency in the tree for nothing.

use std::sync::LazyLock;

/// Cascade layers, declared once, in the order they lose to each other.
/// Declaring them up front means a component rule beats a base rule no matter
/// which file happens to be read first.
const LAYERS: &str = "@layer reset, tokens, base, layout, components, prose, utilities;\n";

pub static CSS: LazyLock<String> = LazyLock::new(|| {
    [
        LAYERS,
        // Generated from the same `.range` files the subsets were fetched with,
        // so the rules and the bytes cannot disagree. See `fonts.rs`.
        crate::fonts::CSS.as_str(),
        include_str!("../assets/css/00-reset.css"),
        include_str!("../assets/css/10-tokens.css"),
        include_str!("../assets/css/20-base.css"),
        include_str!("../assets/css/30-layout.css"),
        include_str!("../assets/css/40-components.css"),
        include_str!("../assets/css/50-prose.css"),
        include_str!("../assets/css/60-demo.css"),
        include_str!("../assets/css/65-sections.css"),
        include_str!("../assets/css/70-utilities.css"),
    ]
    .join("\n")
});

pub static JS: LazyLock<String> = LazyLock::new(|| {
    [
        include_str!("../assets/js/site.js"),
        // Drawing before driving: `demo.js` calls into `Poptop`.
        include_str!("../assets/js/frame.js"),
        include_str!("../assets/js/demo.js"),
        include_str!("../assets/js/cvd.js"),
    ]
    .join("\n")
});

pub static CSS_PATH: LazyLock<String> =
    LazyLock::new(|| format!("/assets/site.{}.css", hash(&CSS)));
pub static JS_PATH: LazyLock<String> = LazyLock::new(|| format!("/assets/site.{}.js", hash(&JS)));

/// Runs before the stylesheet loads, so the page is never painted in the wrong
/// theme and then corrected. Small enough to inline; too important to defer.
pub const THEME_BOOT: &str = r#"try{var t=localStorage.getItem('poptop-theme');if(t)document.documentElement.setAttribute('data-theme',t)}catch(e){}"#;

fn hash(s: &str) -> String {
    hash_bytes(s.as_bytes())
}

pub fn hash_bytes(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")[..12].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The stylesheet is assembled from eight files by concatenation, and CSS
    /// fails silently: a rule with its selector missing is not an error, it is
    /// a declaration block the parser discards, taking the rules after it with
    /// it. That is exactly how the landing headline once shipped at body size.
    ///
    /// This is not a CSS parser. It is the three checks that would have caught
    /// it: the braces balance, no block opens without a selector, and every
    /// declaration sits inside one.
    /// Comments removed, so the scan below cannot mistake a `*::after`
    /// selector for a comment line — which is exactly what the first version of
    /// this test did.
    fn without_comments(css: &str) -> String {
        let mut out = String::with_capacity(css.len());
        let mut rest = css;
        while let Some(at) = rest.find("/*") {
            out.push_str(&rest[..at]);
            match rest[at + 2..].find("*/") {
                Some(end) => {
                    // Keep the newlines so reported line numbers still line up.
                    for _ in rest[at..at + 2 + end].matches('\n') {
                        out.push('\n');
                    }
                    rest = &rest[at + 4 + end..];
                }
                None => return out,
            }
        }
        out.push_str(rest);
        out
    }

    /// The stylesheet is assembled from eight files by concatenation, and CSS
    /// fails silently: a rule with its selector missing is not an error, it is
    /// a declaration block the parser discards, taking the rules after it with
    /// it. That is exactly how the landing headline once shipped at body size.
    ///
    /// This is not a CSS parser. It is the three checks that would have caught
    /// it: the braces balance, no block opens without a selector, and no
    /// declaration sits outside one.
    #[test]
    fn the_stylesheet_is_structurally_sound() {
        let css = without_comments(&CSS);
        let mut depth = 0i32;
        let mut selector = String::from("(top level)");

        for (n, line) in css.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            if let Some(head) = line.strip_suffix('{') {
                assert!(
                    !head.trim().is_empty(),
                    "line {}: a block opens with no selector, after `{selector}`",
                    n + 1
                );
                depth += 1;
                selector = head.trim().to_string();
                continue;
            }
            if line == "}" {
                depth -= 1;
                assert!(
                    depth >= 0,
                    "line {}: closing brace with nothing open",
                    n + 1
                );
                continue;
            }
            // A declaration outside every block is the signature of a selector
            // that was deleted out from under it.
            if line.ends_with(';') && line.contains(':') {
                assert!(
                    depth > 0,
                    "line {}: `{line}` is not inside any rule (after `{selector}`)",
                    n + 1
                );
            }
        }

        assert_eq!(depth, 0, "the stylesheet has {depth} unclosed blocks");
    }

    /// Every class the templates actually put on an element must exist in the
    /// stylesheet. A typo in either place is otherwise invisible until someone
    /// looks at the page.
    #[test]
    fn the_classes_the_views_rely_on_are_all_defined() {
        for class in [
            ".display",
            ".h1",
            ".h2",
            ".h3",
            ".lede",
            ".rule",
            ".section",
            ".btn",
            ".cmd",
            ".badge",
            ".panel",
            ".claim__head",
            ".callout",
            ".links",
            ".sidenav",
            ".toc",
            ".pager",
            ".footer",
            ".caption",
            ".stat__value",
            ".keymap",
            ".swatch",
            ".scale-row",
            ".term",
            ".sr-only",
            ".skip",
        ] {
            assert!(
                CSS.contains(&format!("{class} "))
                    || CSS.contains(&format!("{class},"))
                    || CSS.contains(&format!("{class}{{")),
                "{class} is used by a template but not defined"
            );
        }
    }

    /// Every `var(--x)` the stylesheet reads has to be defined by it. A renamed
    /// token leaves the property at its initial value, which for `font` means
    /// the element silently drops to 16px.
    #[test]
    fn every_token_referenced_is_defined() {
        let defined: std::collections::HashSet<&str> = CSS
            .match_indices("--")
            .filter_map(|(at, _)| {
                let rest = &CSS[at..];
                let end = rest.find(':')?;
                let name = &rest[..end];
                // A definition is `--name:`; a use is `var(--name)`.
                if name.contains([' ', ')', '(', ';', ',']) || CSS[..at].ends_with("var(") {
                    return None;
                }
                Some(name)
            })
            .collect();

        let mut missing = Vec::new();
        for (at, _) in CSS.match_indices("var(--") {
            let rest = &CSS[at + 4..];
            let end = rest.find([')', ',']).unwrap_or(0);
            let name = &rest[..end];
            if !defined.contains(name) {
                missing.push(name);
            }
        }
        missing.sort_unstable();
        missing.dedup();
        assert!(missing.is_empty(), "undefined tokens in use: {missing:?}");
    }
}
