//! The community pages: the hub, and the two documents behind it.
//!
//! `CONTRIBUTING.md` and `CODE_OF_CONDUCT.md` live in the repository root,
//! where GitHub looks for them and where a contributor finds them without a
//! browser. The site renders those same files rather than restating them.

use crate::content::{self, Doc};
use crate::markdown;
use crate::site;
use crate::views::layout::{Meta, Nav};
use crate::views::ui;
use maud::{Markup, PreEscaped, html};

pub fn index_meta() -> Meta {
    Meta::new(
        "Community",
        "How poptop takes changes, where the work is tracked, and what the project \
         asks of the people working on it.",
        "/community",
        Nav::Community,
    )
}

pub fn doc_meta(doc: &Doc) -> Meta {
    Meta::new(doc.title, doc.blurb, &doc.path(), Nav::Community)
}

pub fn index() -> Markup {
    html! {
        div class="wrap wrap--tight" {
            div class="section" style="padding-block: var(--s-7) var(--s-6)" {
                h1 class="h1" { "Community" }
                p class="lede" style="margin-top: var(--s-4)" {
                    "A small project with an argumentative paper trail. Most decisions in
                     poptop were written down before they were made, which means you can
                     disagree with one on the evidence rather than on taste."
                }
            }

            section class="section" {
                (ui::rule_heading("start here", None))
                div style="margin-top: var(--s-4)" {
                    (ui::link_list(&[
                        ("/community/contributing", "Contributing",
                         "What a change is expected to carry, and the one command that checks it"),
                        ("/community/conduct", "Code of conduct",
                         "Argue about the work. Leave people their dignity"),
                        ("/roadmap", "Roadmap",
                         "What is shipped, what is next, and what was left out on purpose"),
                        ("/docs/design/index", "Design notes",
                         "The measurements and rejected alternatives behind the interface"),
                    ]))
                }
            }

            section class="section" {
                (ui::rule_heading("where things happen", None))
                div class="cols" style="margin-top: var(--s-5)" {
                    (ui::claim(
                        "Issues",
                        "Bug reports and concrete proposals. A report that names the \
                         platform, the terminal and what the number should have been is \
                         worth three that do not.",
                    ))
                    (ui::claim(
                        "Discussions",
                        "Anything that is not yet a proposal — a use poptop is bad at, a \
                         tool it should be compared against, a claim in the README you \
                         think is wrong.",
                    ))
                    (ui::claim(
                        "The roadmap",
                        "Tracked as files in the repository rather than in a web app, so \
                         the plan arrives with the clone and can be read on a plane.",
                    ))
                }
                div class="row" style="margin-top: var(--s-5)" {
                    (ui::button(site::ISSUES, "Open an issue", true))
                    (ui::button(site::DISCUSSIONS, "Start a discussion", false))
                    (ui::button(site::REPO, "Read the source", false))
                }
            }

            section class="section" {
                (ui::rule_heading("what to expect from review", None))
                div class="stack" style="margin-top: var(--s-4)" {
                    (ui::callout("One command.", false, html! {
                        code { "./check" }
                        " runs everything CI runs, in the order CI runs it. On a Mac, "
                        code { "./check --linux" }
                        " is the only thing that compiles the "
                        code { "/proc" }
                        " backend at all."
                    }))
                    (ui::callout("Claims carry tests.", false, html! {
                        "If a change makes a claim — that two colours are far enough
                         apart, that a panel degrades gracefully — the preferred form of
                         that claim is a test that fails when it stops being true."
                    }))
                    (ui::callout("Comments explain why.", false, html! {
                        "A comment earns its place by recording something a future reader
                         could not recover from the code: a measurement, a rejected
                         alternative, or the bug a line prevents from coming back."
                    }))
                    (ui::callout("No tool attribution.", true, html! {
                        "Commit messages, file contents and pull request text all end up
                         in public history, and none of them is the place for a note
                         about what wrote them. A hook, "
                        code { "./check" }
                        ", and CI all enforce it, so they cannot disagree."
                    }))
                }
            }

            section class="section" {
                (ui::rule_heading("licence", None))
                p class="measure" {
                    "poptop is " (site::LICENSE) ". Contributions ship under the same
                     terms and stay free for the next person the way they were free for
                     you."
                }
            }
        }
    }
}

pub fn doc(doc: &Doc) -> Markup {
    let rendered = markdown::render(doc.body());
    html! {
        div class="wrap wrap--tight" {
            div class="docs" style="grid-template-columns: minmax(0, 1fr)" {
                article class="prose" {
                    h1 { (doc.title) }
                    (PreEscaped(rendered.html.clone()))
                }
            }
            p class="caption" style="padding-bottom: var(--s-8)" {
                "Rendered from "
                a href=(format!("{}/blob/master/{}", site::REPO, source_path(doc))) {
                    code { (source_path(doc)) }
                }
                " in the repository."
            }
        }
    }
}

fn source_path(doc: &Doc) -> &'static str {
    match doc.source {
        content::Source::File(path, _) => path,
        content::Source::Section(_, _) => "README.md",
    }
}
