//! Markdown, rendered.
//!
//! Two things happen here that a plain `markdown_to_html` would not do, and
//! both exist because the sources are repository documentation rather than
//! pages written for a website.
//!
//! **Headings are promoted.** A page cut from `### Themes` starts at depth
//! three. On its own page it is the top-level thing, so every heading in the
//! body shifts up until the shallowest is an `<h2>` beneath the page's `<h1>`.
//! Skipping levels is not a styling detail: it is what a screen reader reads
//! out as the shape of the document.
//!
//! **Links are rewritten.** `docs/roadmaps/05-data-fidelity.md` is a real path
//! in the repository and a dead link on the web. Repository-relative links
//! become site routes where the site has that page, and GitHub URLs where it
//! does not — so such a link is never simply broken.
//!
//! No syntax highlighting, deliberately. Most fenced blocks in this project
//! hold terminal output — braille plots, `poptop.conf`, sample tables — which
//! no lexer knows, and a highlighter that colours half a page and gives up on
//! the rest is worse than one that colours none of it.

use comrak::nodes::NodeValue;
use comrak::{Anchorizer, Arena, Options, format_html, parse_document};

const REPO: &str = "https://github.com/oddurs/poptop";

/// One entry in a page's contents.
pub struct Heading {
    pub id: String,
    pub text: String,
    pub depth: u8,
}

pub struct Rendered {
    pub html: String,
    pub contents: Vec<Heading>,
}

/// A page body: its headings normalised so the shallowest sits at `<h2>`,
/// beneath the `<h1>` the page itself supplies.
pub fn render(source: &str) -> Rendered {
    render_with(source, true)
}

/// A whole document, headings left exactly as written. Used for the roadmap,
/// which is generated with its own `<h1>` and owns the page.
pub fn render_document(source: &str) -> Rendered {
    render_with(source, false)
}

fn render_with(source: &str, normalise: bool) -> Rendered {
    let arena = Arena::new();
    let options = options();
    let root = parse_document(&arena, source, &options);

    let shallowest = root
        .descendants()
        .filter_map(|n| match n.data.borrow().value {
            NodeValue::Heading(h) => Some(h.level),
            _ => None,
        })
        .min()
        .unwrap_or(2);
    // Shift so the shallowest heading lands on h2 — up for a section cut from
    // `###`, down for a whole file that starts at `#`. A page with two h1s, or
    // one that jumps from h1 to h3, is not a styling problem: it is what a
    // screen reader reads out as the shape of the document.
    let shift: i16 = if normalise {
        i16::from(shallowest) - 2
    } else {
        0
    };

    // The same anchorizer comrak will use when it formats, walked in the same
    // document order, so the contents link to ids that actually exist.
    let mut anchorizer = Anchorizer::new();
    let mut contents = Vec::new();

    for node in root.descendants() {
        let mut data = node.data.borrow_mut();
        if let NodeValue::Heading(heading) = &mut data.value {
            heading.level = (i16::from(heading.level) - shift).clamp(1, 6) as u8;
            let level = heading.level;
            drop(data);
            let text = node.collect_text();
            let id = anchorizer.anchorize(&text);
            if level <= 3 && !text.trim().is_empty() {
                contents.push(Heading {
                    id,
                    text,
                    depth: level,
                });
            }
        }
    }

    let mut html = String::new();
    format_html(root, &options, &mut html).expect("writing to a String cannot fail");

    Rendered { html, contents }
}

fn options() -> Options<'static> {
    let mut o = Options::default();
    o.extension.strikethrough = true;
    o.extension.table = true;
    o.extension.autolink = true;
    o.extension.tasklist = true;
    o.extension.footnotes = true;
    o.extension.superscript = true;
    // An empty prefix means "give every heading an id", which is what the
    // contents and every deep link off the README depend on.
    o.extension.header_id_prefix = Some(String::new());
    o.extension.link_url_rewriter = Some(std::sync::Arc::new(rewrite));
    // The sources are this repository's own files. Their inline HTML — `<sup>`
    // tags in the roadmap, mostly — is intentional, and is not user input from
    // anywhere.
    o.render.r#unsafe = true;
    o
}

/// Repository-relative links become site routes where the site has the page,
/// and GitHub URLs where it does not.
fn rewrite(url: &str) -> String {
    let bare = url.trim_start_matches("./");
    if bare.starts_with("http")
        || bare.starts_with('#')
        || bare.starts_with('/')
        || bare.starts_with("mailto:")
    {
        return url.to_string();
    }

    let mut path = bare;
    while let Some(rest) = path.strip_prefix("../") {
        path = rest;
    }

    let mapped = match path {
        "ROADMAP.md" => Some("/roadmap".to_string()),
        "README.md" => Some("/docs/install".to_string()),
        "docs/roadmaps/" | "docs/roadmaps/README.md" => Some("/docs/design/index".to_string()),
        _ => path
            .strip_prefix("docs/roadmaps/")
            .unwrap_or(path)
            .strip_suffix(".md")
            .and_then(design_route),
    };

    mapped.unwrap_or_else(|| format!("{REPO}/blob/master/{path}"))
}

/// The design notes are numbered on disk and named in the sidebar. One table,
/// so the two cannot disagree.
fn design_route(file: &str) -> Option<String> {
    let slug = match file {
        "00-positioning" => "positioning",
        "01-color-and-accessibility" => "colour",
        "02-chart-legibility" => "charts",
        "03-layout-and-density" => "layout",
        "04-process-table" => "process-table",
        "05-data-fidelity" => "data-fidelity",
        "06-collection-efficiency" => "collection",
        _ => return None,
    };
    Some(format!("/docs/design/{slug}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_section_starting_at_h3_is_lifted_to_h2() {
        let out = render("### Themes\n\nbody\n\n#### deeper\n\nmore\n");
        assert!(out.html.contains("<h2"), "{}", out.html);
        assert!(out.html.contains("<h3"), "{}", out.html);
        assert!(!out.html.contains("<h4"), "{}", out.html);
    }

    #[test]
    fn a_whole_file_keeps_its_own_h1() {
        let out = render_document("# Roadmap\n\n## v0.1\n\nbody\n");
        assert!(out.html.contains("<h1"));
        assert!(out.html.contains("<h2"));
    }

    /// A page supplies its own title, so a body that still starts at h1 is
    /// pushed down rather than competing with it.
    #[test]
    fn a_body_starting_at_h1_is_demoted_under_the_page_title() {
        let out = render("# Roadmaps\n\n## Suggested order\n\nbody\n");
        assert!(!out.html.contains("<h1"), "{}", out.html);
        assert!(out.html.contains("<h2"));
        assert!(out.html.contains("<h3"));
    }

    #[test]
    fn repository_paths_become_site_routes() {
        assert_eq!(
            rewrite("docs/roadmaps/05-data-fidelity.md"),
            "/docs/design/data-fidelity"
        );
        assert_eq!(rewrite("../../ROADMAP.md"), "/roadmap");
        assert_eq!(
            rewrite("01-color-and-accessibility.md"),
            "/docs/design/colour"
        );
    }

    #[test]
    fn paths_the_site_does_not_serve_fall_back_to_github() {
        assert_eq!(
            rewrite("LICENSE"),
            "https://github.com/oddurs/poptop/blob/master/LICENSE"
        );
        assert_eq!(
            rewrite("themes/"),
            "https://github.com/oddurs/poptop/blob/master/themes/"
        );
        assert_eq!(
            rewrite("../../cairn/items/"),
            "https://github.com/oddurs/poptop/blob/master/cairn/items/"
        );
    }

    #[test]
    fn external_and_absolute_links_are_left_alone() {
        assert_eq!(
            rewrite("https://www.atoptool.nl/"),
            "https://www.atoptool.nl/"
        );
        assert_eq!(rewrite("#keys"), "#keys");
        assert_eq!(rewrite("/docs/keys"), "/docs/keys");
    }

    /// The contents and the rendered ids come from the same anchorizer. If a
    /// comrak upgrade changes the anchoring algorithm this fails here rather
    /// than shipping a page of links that go nowhere.
    #[test]
    fn contents_ids_match_the_ids_in_the_html() {
        let out = render("## Reading the table\n\nx\n\n## Reading the table\n\ny\n");
        assert_eq!(out.contents.len(), 2);
        assert_eq!(out.contents[0].id, "reading-the-table");
        assert_eq!(out.contents[1].id, "reading-the-table-1");
        for heading in &out.contents {
            assert!(
                out.html.contains(&format!("id=\"{}\"", heading.id)),
                "no id {} in {}",
                heading.id,
                out.html
            );
        }
    }

    #[test]
    fn tables_and_footnotes_survive() {
        let out = render("| a | b |\n|---|---|\n| 1 | 2 |\n\ntext[^1]\n\n[^1]: note\n");
        assert!(out.html.contains("<table"));
        assert!(out.html.contains("footnotes"));
    }
}
