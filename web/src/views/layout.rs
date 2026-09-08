//! The page shell: head, masthead, footer.
//!
//! Every route renders through `page`, so there is exactly one place where the
//! document language, the canonical URL, the theme boot script and the skip
//! link are decided, and no route can quietly forget one.

use crate::assets;
use crate::site;
use maud::{DOCTYPE, Markup, PreEscaped, html};

/// What the shell needs to know about the page inside it.
pub struct Meta {
    pub title: String,
    pub description: String,
    /// Absolute path, leading slash, no trailing slash except at the root.
    pub path: String,
    /// Which masthead link is the current one.
    pub nav: Nav,
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum Nav {
    Home,
    Docs,
    Design,
    Community,
    None,
}

impl Meta {
    pub fn new(title: &str, description: &str, path: &str, nav: Nav) -> Self {
        Self {
            title: title.to_string(),
            description: description.to_string(),
            path: path.to_string(),
            nav,
        }
    }
}

pub fn page(base_url: &str, meta: &Meta, body: Markup) -> Markup {
    // The landing page is the one page whose title is not "thing — poptop":
    // "poptop — A system monitor you can rewind" reads better in a tab and in
    // a search result than "poptop — poptop".
    let title = if meta.path == "/" {
        format!("{} — {}", site::NAME, site::TAGLINE)
    } else {
        format!("{} — {}", meta.title, site::NAME)
    };
    let canonical = format!("{base_url}{}", meta.path);

    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (title) }
                meta name="description" content=(meta.description);
                link rel="canonical" href=(canonical);

                // Before the stylesheet, so a stored theme is applied before
                // the first paint rather than corrected after it.
                script { (PreEscaped(assets::THEME_BOOT)) }

                // The upright Latin subset is on the critical path for every
                // page. Preloading it starts the fetch alongside the
                // stylesheet instead of after the browser has parsed it and
                // found the @font-face.
                link rel="preload" href=(crate::fonts::primary().path()) as="font"
                    type="font/woff2" crossorigin;
                link rel="stylesheet" href=(*assets::CSS_PATH);

                meta property="og:type" content="website";
                meta property="og:title" content=(title);
                meta property="og:description" content=(meta.description);
                meta property="og:url" content=(canonical);
                meta property="og:image" content=(format!("{base_url}/og.svg"));
                meta property="og:image:width" content="1200";
                meta property="og:image:height" content="630";
                meta property="og:image:alt"
                    content="poptop — a system monitor you can rewind";
                meta property="og:site_name" content="poptop";
                meta name="twitter:card" content="summary_large_image";
                meta name="color-scheme" content="dark light";

                link rel="icon" href="/favicon.svg" type="image/svg+xml";
                link rel="alternate" type="application/rss+xml" title="poptop releases"
                    href=(format!("{}/releases.atom", site::REPO));

                // Structured data, on the landing page only. Repeating it on
                // every documentation page would claim each of them is the
                // application, which is not what any of them is.
                @if meta.path == "/" {
                    script type="application/ld+json" {
                        (PreEscaped(structured_data(base_url)))
                    }
                }
            }
            body {
                a class="skip" href="#main" { "Skip to content" }
                (masthead(meta.nav))
                main id="main" { (body) }
                (footer())
                script src=(*assets::JS_PATH) defer {}
            }
        }
    }
}

/// A description of the software, for anything that reads schema.org. Written
/// out by hand rather than through a serialiser: it is one flat object, and a
/// dependency for four fields would be a poor trade.
fn structured_data(base_url: &str) -> String {
    format!(
        r#"{{"@context":"https://schema.org","@type":"SoftwareApplication","name":"poptop","applicationCategory":"DeveloperApplication","operatingSystem":"Linux, macOS","url":"{base_url}/","description":"{desc}","license":"https://www.gnu.org/licenses/gpl-3.0.html","codeRepository":"{repo}","programmingLanguage":"Rust","offers":{{"@type":"Offer","price":"0","priceCurrency":"USD"}}}}"#,
        desc = site::DESCRIPTION.replace('"', "'").replace('\n', " "),
        repo = site::REPO,
    )
}

fn masthead(current: Nav) -> Markup {
    let link = |href: &str, label: &str, nav: Nav, overflow: bool| {
        html! {
            a href=(href)
              class=[overflow.then_some("nav--overflow")]
              aria-current=[(current == nav).then_some("page")] { (label) }
        }
    };

    html! {
        header class="masthead" {
            div class="wrap masthead__inner" {
                a class="masthead__mark" href="/" { "poptop" }
                nav class="nav" aria-label="Site" {
                    (link("/docs/install", "Docs", Nav::Docs, false))
                    (link("/design", "Design", Nav::Design, true))
                    (link("/community", "Community", Nav::Community, false))
                    a href=(site::REPO) rel="noopener" { "GitHub" }
                    button class="theme-toggle" type="button" data-theme-toggle
                        aria-label="Change colour theme" { "◐" }
                }
            }
        }
    }
}

fn footer() -> Markup {
    html! {
        footer class="footer" {
            div class="wrap" {
                div class="footer__grid" {
                    div {
                        h2 { "Read" }
                        ul {
                            li { a href="/docs/install" { "Install" } }
                            li { a href="/docs/keys" { "Keys" } }
                            li { a href="/docs/configuration" { "Configuration" } }
                            li { a href="/docs/how-it-works" { "How it works" } }
                        }
                    }
                    div {
                        h2 { "Decide" }
                        ul {
                            li { a href="/docs/prior-art" { "Compared to atop" } }
                            li { a href="/docs/blind-spots" { "What it cannot see" } }
                            li { a href="/docs/design/index" { "Design notes" } }
                            li { a href="/roadmap" { "Roadmap" } }
                        }
                    }
                    div {
                        h2 { "Join in" }
                        ul {
                            li { a href="/community" { "Contributing" } }
                            li { a href=(site::ISSUES) rel="noopener" { "Issues" } }
                            li { a href=(site::DISCUSSIONS) rel="noopener" { "Discussions" } }
                            li { a href="/design" { "Design system" } }
                        }
                    }
                    div {
                        h2 { "Source" }
                        ul {
                            li { a href=(site::REPO) rel="noopener" { "GitHub" } }
                            li { a href=(site::CRATE) rel="noopener" { "crates.io" } }
                        }
                    }
                }
                div class="footer__legal" {
                    span { (site::LICENSE) ". Free software, and meant to stay that way." }
                    span { "Built with ratatui and crossterm." }
                }
            }
        }
    }
}
