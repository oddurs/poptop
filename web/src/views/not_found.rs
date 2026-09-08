//! The page for a URL the site does not serve.
//!
//! Here rather than in `app.rs` because two callers need it: the router, which
//! wraps it in a 404 status, and the static export, which writes it to
//! `404.html` for the host to serve. It used to exist twice, copied because the
//! async handler was inconvenient to call from a synchronous exporter — thirty
//! lines below a module comment in `export.rs` promising there was no second
//! implementation to keep in step.

use crate::site;
use crate::views::layout::{Meta, Nav};
use crate::views::ui;
use maud::{Markup, html};

pub fn meta() -> Meta {
    Meta::new("Not found", "That page is not here.", "/404", Nav::None)
}

/// A 404 that helps. A wrong URL on a documentation site is usually a page that
/// moved, so this offers the list rather than an apology.
pub fn render() -> Markup {
    html! {
        div class="wrap wrap--tight" {
            div class="section" style="padding-block: var(--s-8)" {
                p class="mono dim" { "404" }
                h1 class="h1" style="margin-top: var(--s-3)" { "That page is not here." }
                p class="lede" style="margin-top: var(--s-4)" {
                    "It may have moved. Everything the site has is one of these."
                }
                div style="margin-top: var(--s-6)" {
                    (ui::link_list(&[
                        ("/docs", "Documentation", "Every page, grouped"),
                        ("/roadmap", "Roadmap", "What is shipped and what is next"),
                        ("/community", "Community", "How to contribute"),
                        (site::REPO, "The repository", "Source, issues, and discussions"),
                    ]))
                }
            }
        }
    }
}
