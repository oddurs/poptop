//! Documentation pages, and the roadmap.
//!
//! The body of every page is markdown that already exists in the repository —
//! see `content.rs` for why — so this module is layout: a sidebar, the prose,
//! the contents of the page, and the way out at the bottom.

use crate::content::{self, Doc, Group};
use crate::markdown::{self, Rendered};
use crate::site;
use crate::views::layout::{Meta, Nav};
use crate::views::ui;
use maud::{Markup, PreEscaped, html};

pub fn doc_meta(doc: &Doc) -> Meta {
    Meta::new(doc.title, doc.blurb, &doc.path(), Nav::Docs)
}

pub fn index_meta() -> Meta {
    Meta::new(
        "Documentation",
        "Everything poptop does, what each number is measured from, and the arguments \
         behind the interface.",
        "/docs",
        Nav::Docs,
    )
}

pub fn roadmap_meta() -> Meta {
    Meta::new(
        "Roadmap",
        "What is shipped, what is next, and what was deliberately left out. Rendered \
         from the same files the project tracks its work in.",
        "/roadmap",
        Nav::Docs,
    )
}

pub fn index() -> Markup {
    html! {
        div class="wrap wrap--tight" {
            div class="section" style="padding-block: var(--s-7) var(--s-6)" {
                h1 class="h1" { "Documentation" }
                p class="lede" style="margin-top: var(--s-4)" {
                    "Every page here is a section of the project's own README or design
                     notes, rendered. Nothing is written twice, so nothing can drift."
                }
            }
            @for group in Group::ORDER {
                section class="section" {
                    (ui::rule_heading(group.title(), None))
                    div style="margin-top: var(--s-4)" {
                        (group_links(group))
                    }
                }
            }
        }
    }
}

pub fn doc(doc: &Doc) -> Markup {
    let rendered = markdown::render(doc.body());
    let (prev, next) = content::neighbours(doc.slug);

    shell(
        doc.slug,
        html! {
            article class="prose" {
                h1 { (doc.title) }
                p class="lede" { (doc.blurb) }
                (PreEscaped(rendered.html.clone()))
            }
            (pager(prev, next))
            (source_note(doc))
        },
        &rendered,
    )
}

pub fn roadmap() -> Markup {
    let rendered = markdown::render_document(content::roadmap());
    shell(
        "",
        html! {
            article class="prose" {
                (PreEscaped(rendered.html.clone()))
            }
            p class="caption" style="margin-top: var(--s-7)" {
                "Rendered from " code { "ROADMAP.md" } ", which the project generates from
                 the item files it tracks work in. "
                a href=(format!("{}/tree/master/cairn/items", site::REPO)) { "The items themselves" }
                " carry the reasoning."
            }
        },
        &rendered,
    )
}

/// Sidebar, page, contents. The contents column disappears below the docs
/// breakpoint rather than stacking: a table of contents above the thing it
/// indexes is furniture.
fn shell(current: &str, body: Markup, rendered: &Rendered) -> Markup {
    html! {
        div class="wrap" {
            div class="docs" {
                aside class="docs__aside" {
                    nav class="sidenav" aria-label="Documentation" {
                        @for group in Group::ORDER {
                            div class="sidenav__group" {
                                p class="sidenav__title" { (group.title()) }
                                ul {
                                    @for d in content::in_group(group) {
                                        li {
                                            a href=(d.path())
                                              aria-current=[(d.slug == current).then_some("page")] {
                                                (d.title)
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        div class="sidenav__group" {
                            p class="sidenav__title" { "Project" }
                            ul {
                                li { a href="/roadmap" { "Roadmap" } }
                                li { a href="/community" { "Contributing" } }
                            }
                        }
                    }
                }
                div {
                    (body)
                }
                (contents(rendered))
            }
        }
    }
}

/// The on-page contents: the third column, and the first thing to go when the
/// window narrows. A table of contents stacked above the thing it indexes is
/// furniture, so below the breakpoint it is not rendered smaller — it is gone.
fn contents(rendered: &Rendered) -> Markup {
    if rendered.contents.len() < 3 {
        return html! { div {} };
    }
    html! {
        nav class="docs__toc" data-toc aria-label="On this page" {
            div class="toc" {
                p class="sidenav__title" { "On this page" }
                ul {
                    @for h in &rendered.contents {
                        li data-depth=(h.depth) {
                            a href=(format!("#{}", h.id)) { (h.text) }
                        }
                    }
                }
            }
        }
    }
}

fn pager(prev: Option<&Doc>, next: Option<&Doc>) -> Markup {
    if prev.is_none() && next.is_none() {
        return html! {};
    }
    html! {
        nav class="pager" aria-label="Nearby pages" {
            @if let Some(p) = prev {
                a href=(p.path()) { span class="pager__dir" { "previous" } (p.title) }
            } @else { span {} }
            @if let Some(n) = next {
                a href=(n.path()) style="text-align: right" {
                    span class="pager__dir" { "next" } (n.title)
                }
            }
        }
    }
}

/// Where this page actually lives, so a reader who wants to fix it knows what
/// file to open. A docs site that cannot be corrected does not get corrected.
fn source_note(doc: &Doc) -> Markup {
    let (file, what) = match doc.source {
        content::Source::File(path, _) => (path, format!("`{path}`, rendered")),
        content::Source::Section(_, heading) => ("README.md", format!("`{heading}` in the README")),
    };
    html! {
        p class="caption" style="margin-top: var(--s-5)" {
            "This page is " (PreEscaped(inline_code(&what))) ". "
            a href=(format!("{}/edit/master/{file}", site::REPO)) { "Fix it there" }
            " and the site follows."
        }
    }
}

fn inline_code(text: &str) -> String {
    // The heading names arrive wrapped in backticks; turn exactly that into
    // markup rather than reaching for a markdown pass over five words.
    let mut out = String::new();
    let mut code = false;
    for ch in text.chars() {
        match ch {
            '`' => {
                out.push_str(if code { "</code>" } else { "<code>" });
                code = !code;
            }
            '<' => out.push_str("&lt;"),
            '&' => out.push_str("&amp;"),
            c => out.push(c),
        }
    }
    if code {
        out.push_str("</code>");
    }
    out
}

fn group_links(group: Group) -> Markup {
    let paths: Vec<String> = content::in_group(group).map(|d| d.path()).collect();
    let items: Vec<(&str, &str, &str)> = content::in_group(group)
        .zip(&paths)
        .map(|(d, path)| (path.as_str(), d.title, d.blurb))
        .collect();
    ui::link_list(&items)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backticks_in_a_source_note_become_code() {
        assert_eq!(
            inline_code("`## Keys` in the README"),
            "<code>## Keys</code> in the README"
        );
    }

    #[test]
    fn a_source_note_escapes_what_it_did_not_mean_as_markup() {
        assert_eq!(inline_code("a < b"), "a &lt; b");
    }
}
