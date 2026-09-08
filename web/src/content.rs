//! What is on the site, and where it comes from.
//!
//! Every documentation page is a *section of a file that already exists in the
//! repository* — the README, `ROADMAP.md`, or one of the design notes under
//! `docs/roadmaps/`. None of it is copied here. A docs site that duplicates its
//! project's README is a docs site that is wrong six weeks later, and the
//! failure is silent: nobody diffs prose.
//!
//! So pages name a heading rather than carrying text, `section()` cuts the
//! file at that heading, and a test asserts every declared heading still
//! exists. Rename a heading in the README and the build fails, which is the
//! only kind of link check worth having.
//!
//! The files are `include_str!`d, so the finished binary is the whole site.
//! Nothing to deploy beside it, nothing to mount, no path to get wrong — the
//! same property that lets poptop run with nothing set up first.

/// Where a page's markdown comes from.
#[derive(Clone, Copy)]
pub enum Source {
    /// A whole file, used verbatim: its path in the repository, and its text.
    File(&'static str, &'static str),
    /// One section of a file: from the given heading to the next heading at
    /// the same level or higher. The heading itself is dropped — the page
    /// supplies its own title.
    Section(&'static str, &'static str),
}

/// A documentation page.
pub struct Doc {
    pub slug: &'static str,
    pub title: &'static str,
    /// One line, in the sidebar's voice: what the reader gets, not what the
    /// page is about.
    pub blurb: &'static str,
    pub group: Group,
    pub source: Source,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Group {
    Start,
    Reading,
    Configuring,
    Limits,
    Internals,
    Design,
}

impl Group {
    pub const ORDER: [Group; 6] = [
        Group::Start,
        Group::Reading,
        Group::Configuring,
        Group::Limits,
        Group::Internals,
        Group::Design,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Group::Start => "Start here",
            Group::Reading => "Reading the screen",
            Group::Configuring => "Configuring",
            Group::Limits => "What it can and cannot see",
            Group::Internals => "How it works",
            Group::Design => "Design notes",
        }
    }
}

const README: &str = include_str!("../../README.md");
const ROADMAP: &str = include_str!("../../ROADMAP.md");

pub const DOCS: &[Doc] = &[
    Doc {
        slug: "install",
        title: "Install",
        blurb: "Build it, or install it from crates.io.",
        group: Group::Start,
        source: Source::Section(README, "## Build"),
    },
    Doc {
        slug: "keys",
        title: "Keys",
        blurb: "Every binding, and what it does while paused.",
        group: Group::Start,
        source: Source::Section(README, "## Keys"),
    },
    Doc {
        slug: "prior-art",
        title: "Compared to atop and zenith",
        blurb: "Where poptop is the weaker tool, and where it is not.",
        group: Group::Start,
        source: Source::Section(README, "## Prior art, and where poptop actually differs"),
    },
    Doc {
        slug: "header",
        title: "The header",
        blurb: "CPU, wait, run queue, blocked, memory — and what each is measured from.",
        group: Group::Reading,
        source: Source::Section(README, "## Reading the header"),
    },
    Doc {
        slug: "timeline",
        title: "The timeline",
        blurb: "What the graphs plot, and what the gutter is for.",
        group: Group::Reading,
        source: Source::Section(README, "### What the timeline graphs"),
    },
    Doc {
        slug: "processes",
        title: "The process table",
        blurb: "Columns, sorting, and the per-process history sparkline.",
        group: Group::Reading,
        source: Source::Section(README, "## Reading the table"),
    },
    Doc {
        slug: "identifying-a-row",
        title: "Identifying a row",
        blurb: "Why a pid is not a name, and what the table shows instead.",
        group: Group::Reading,
        source: Source::Section(README, "### Identifying a row"),
    },
    Doc {
        slug: "configuration",
        title: "poptop.conf",
        blurb: "Every setting, with its default.",
        group: Group::Configuring,
        source: Source::Section(README, "## Configuration"),
    },
    Doc {
        slug: "sampling",
        title: "Sample rate and window",
        blurb: "How long the buffer holds, and what it costs to hold it.",
        group: Group::Configuring,
        source: Source::Section(README, "### Sample rate and window"),
    },
    Doc {
        slug: "history",
        title: "History across restarts",
        blurb: "Keeping the buffer when the process goes away.",
        group: Group::Configuring,
        source: Source::Section(README, "### Keeping history across restarts"),
    },
    Doc {
        slug: "themes",
        title: "Themes",
        blurb: "The palette, the tiers, and writing your own.",
        group: Group::Configuring,
        source: Source::Section(README, "### Themes"),
    },
    Doc {
        slug: "measuring-a-theme",
        title: "Measuring a theme",
        blurb: "Contrast and colour-vision checks you can run on your own palette.",
        group: Group::Configuring,
        source: Source::Section(README, "### Measuring a theme"),
    },
    Doc {
        slug: "storage",
        title: "Storage",
        blurb: "Disk throughput, and where the numbers come from.",
        group: Group::Limits,
        source: Source::Section(README, "### Storage"),
    },
    Doc {
        slug: "filesystem-capacity",
        title: "Filesystem capacity",
        blurb: "What counts as full, and which mounts are counted.",
        group: Group::Limits,
        source: Source::Section(README, "### Filesystem capacity"),
    },
    Doc {
        slug: "network",
        title: "The network",
        blurb: "Interface counters, and what they do not tell you.",
        group: Group::Limits,
        source: Source::Section(README, "### The network"),
    },
    Doc {
        slug: "stall-pressure",
        title: "Stall pressure",
        blurb: "Reading pressure stall information when the kernel offers it.",
        group: Group::Limits,
        source: Source::Section(README, "### Stall pressure"),
    },
    Doc {
        slug: "blind-spots",
        title: "What the table cannot show",
        blurb: "Short-lived processes, and the honest account of what is missed.",
        group: Group::Limits,
        source: Source::Section(README, "### What the table cannot show"),
    },
    Doc {
        slug: "how-it-works",
        title: "How it works",
        blurb: "The collector, the ring buffer, and the draw loop.",
        group: Group::Internals,
        source: Source::Section(README, "## How it works"),
    },
    Doc {
        slug: "reading-the-others",
        title: "Notes from reading the others",
        blurb: "What htop, btop, bottom, atop and zenith each got right.",
        group: Group::Internals,
        source: Source::Section(README, "## Notes from reading the others"),
    },
    Doc {
        slug: "tests",
        title: "Tests",
        blurb: "What is covered, and what a passing suite does not prove.",
        group: Group::Internals,
        source: Source::Section(README, "## Tests"),
    },
    Doc {
        slug: "status",
        title: "Status",
        blurb: "Which platforms are real, and how finished each one is.",
        group: Group::Internals,
        source: Source::Section(README, "## Status"),
    },
    // The design notes are the reasoning behind the interface: measurements,
    // prior-art comparisons, and the arguments that produced each decision.
    // They are the most interesting writing in the repository and they were
    // buried two directories deep, so the site gives them a section.
    Doc {
        slug: "design/index",
        title: "Why these notes exist",
        blurb: "How the interface decisions were argued, and against what evidence.",
        group: Group::Design,
        source: Source::File(
            "docs/roadmaps/README.md",
            include_str!("../../docs/roadmaps/README.md"),
        ),
    },
    Doc {
        slug: "design/positioning",
        title: "Positioning",
        blurb: "Honest claims about what poptop is.",
        group: Group::Design,
        source: Source::File(
            "docs/roadmaps/00-positioning.md",
            include_str!("../../docs/roadmaps/00-positioning.md"),
        ),
    },
    Doc {
        slug: "design/colour",
        title: "Colour and accessibility",
        blurb: "Palette, theming, and colour-vision safety.",
        group: Group::Design,
        source: Source::File(
            "docs/roadmaps/01-color-and-accessibility.md",
            include_str!("../../docs/roadmaps/01-color-and-accessibility.md"),
        ),
    },
    Doc {
        slug: "design/charts",
        title: "Chart legibility",
        blurb: "Scale, thresholds, labels, and the cursor readout.",
        group: Group::Design,
        source: Source::File(
            "docs/roadmaps/02-chart-legibility.md",
            include_str!("../../docs/roadmaps/02-chart-legibility.md"),
        ),
    },
    Doc {
        slug: "design/layout",
        title: "Layout and density",
        blurb: "Reclaiming vertical space.",
        group: Group::Design,
        source: Source::File(
            "docs/roadmaps/03-layout-and-density.md",
            include_str!("../../docs/roadmaps/03-layout-and-density.md"),
        ),
    },
    Doc {
        slug: "design/process-table",
        title: "The process table",
        blurb: "Scanning the table, and per-process history.",
        group: Group::Design,
        source: Source::File(
            "docs/roadmaps/04-process-table.md",
            include_str!("../../docs/roadmaps/04-process-table.md"),
        ),
    },
    Doc {
        slug: "design/data-fidelity",
        title: "Data fidelity",
        blurb: "The gaps that limit what poptop can answer.",
        group: Group::Design,
        source: Source::File(
            "docs/roadmaps/05-data-fidelity.md",
            include_str!("../../docs/roadmaps/05-data-fidelity.md"),
        ),
    },
    Doc {
        slug: "design/collection",
        title: "Collection efficiency",
        blurb: "Making the collector cheap enough to sample faster.",
        group: Group::Design,
        source: Source::File(
            "docs/roadmaps/06-collection-efficiency.md",
            include_str!("../../docs/roadmaps/06-collection-efficiency.md"),
        ),
    },
];

/// The community pages. Kept out of `DOCS` because they belong to the project
/// rather than to the tool, and putting them in the documentation sidebar
/// would bury them under twenty pages about reading a graph.
pub const COMMUNITY: &[Doc] = &[
    Doc {
        slug: "contributing",
        title: "Contributing",
        blurb: "What a change is expected to carry, and the one command that checks it.",
        group: Group::Start,
        source: Source::File("CONTRIBUTING.md", include_str!("../../CONTRIBUTING.md")),
    },
    Doc {
        slug: "conduct",
        title: "Code of conduct",
        blurb: "Argue about the work. Leave people their dignity.",
        group: Group::Start,
        source: Source::File(
            "CODE_OF_CONDUCT.md",
            include_str!("../../CODE_OF_CONDUCT.md"),
        ),
    },
];

pub fn community(slug: &str) -> Option<&'static Doc> {
    COMMUNITY.iter().find(|d| d.slug == slug)
}

/// The roadmap, rendered from the same file `cairn render` writes.
pub fn roadmap() -> &'static str {
    ROADMAP
}

pub fn find(slug: &str) -> Option<&'static Doc> {
    DOCS.iter().find(|d| d.slug == slug)
}

pub fn in_group(group: Group) -> impl Iterator<Item = &'static Doc> {
    DOCS.iter().filter(move |d| d.group == group)
}

/// The page before and after this one, in sidebar order.
pub fn neighbours(slug: &str) -> (Option<&'static Doc>, Option<&'static Doc>) {
    let ordered: Vec<&Doc> = Group::ORDER.iter().flat_map(|g| in_group(*g)).collect();
    let Some(at) = ordered.iter().position(|d| d.slug == slug) else {
        return (None, None);
    };
    let prev = at.checked_sub(1).and_then(|i| ordered.get(i)).copied();
    (prev, ordered.get(at + 1).copied())
}

impl Doc {
    /// Community pages live under `/community`, everything else under `/docs`.
    /// Slugs are unique across both lists, which a test asserts.
    pub fn path(&self) -> String {
        if COMMUNITY.iter().any(|d| d.slug == self.slug) {
            format!("/community/{}", self.slug)
        } else {
            format!("/docs/{}", self.slug)
        }
    }

    /// The markdown for this page, with its own heading removed.
    pub fn body(&self) -> &'static str {
        match self.source {
            Source::File(_, text) => strip_leading_title(text),
            Source::Section(text, heading) => {
                section(text, heading).expect("declared heading is missing from its source file")
            }
        }
    }
}

/// Drop a file's own top-level title, which the page renders itself from the
/// declared `title`. Without this every file-sourced page carries its heading
/// twice, once from the site and once from the file.
pub fn strip_leading_title(text: &'static str) -> &'static str {
    let mut offset = 0usize;
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            offset += line.len();
            continue;
        }
        return match trimmed.strip_prefix("# ") {
            Some(_) => &text[offset + line.len()..],
            None => text,
        };
    }
    text
}

/// Cut a markdown file at `heading`, ending at the next heading of the same
/// level or higher.
///
/// Fenced code blocks are skipped, because poptop's README contains sample
/// configuration files whose comment lines start with `#` and would otherwise
/// read as headings and truncate the page mid-example.
pub fn section(text: &'static str, heading: &str) -> Option<&'static str> {
    let level = heading.chars().take_while(|c| *c == '#').count();
    let mut start = None;
    let mut fenced = false;
    let mut end = text.len();
    let mut offset = 0usize;

    for line in text.split_inclusive('\n') {
        let here = offset;
        offset += line.len();
        let trimmed = line.trim_end();

        if trimmed.starts_with("```") {
            fenced = !fenced;
            continue;
        }
        if fenced || !trimmed.starts_with('#') {
            continue;
        }

        if start.is_none() {
            if trimmed == heading {
                start = Some(offset); // the heading line itself is dropped
            }
            continue;
        }

        if trimmed.chars().take_while(|c| *c == '#').count() <= level {
            end = here;
            break;
        }
    }

    start.map(|s| &text[s..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The link check that matters: every page still points at a heading that
    /// exists. Rename a README heading and this fails, rather than the site
    /// quietly serving an empty page.
    #[test]
    fn every_declared_section_resolves() {
        for doc in DOCS {
            let body = doc.body();
            assert!(
                body.trim().len() > 40,
                "{} resolved to almost nothing — has its source heading moved?",
                doc.slug
            );
        }
    }

    /// Uniqueness across both lists, because `path()` decides which prefix a
    /// page gets by looking its slug up in the other one.
    #[test]
    fn slugs_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for doc in DOCS.iter().chain(COMMUNITY) {
            assert!(seen.insert(doc.slug), "duplicate slug {}", doc.slug);
        }
    }

    #[test]
    fn community_pages_are_not_under_docs() {
        assert_eq!(
            community("contributing").unwrap().path(),
            "/community/contributing"
        );
        assert_eq!(find("keys").unwrap().path(), "/docs/keys");
    }

    #[test]
    fn every_community_page_resolves() {
        for doc in COMMUNITY {
            assert!(doc.body().trim().len() > 200, "{} is empty", doc.slug);
        }
    }

    #[test]
    fn a_file_loses_its_own_title_but_keeps_everything_else() {
        assert_eq!(strip_leading_title("# Title\n\nbody\n"), "\nbody\n");
        assert_eq!(
            strip_leading_title("## Title\n\nbody\n"),
            "## Title\n\nbody\n"
        );
        assert_eq!(strip_leading_title("body only\n"), "body only\n");
    }

    #[test]
    fn a_section_stops_at_the_next_heading_of_its_level() {
        let text = "# t\n\n## one\n\nbody\n\n### deep\n\nmore\n\n## two\n\nafter\n";
        let cut = section(text, "## one").unwrap();
        assert!(cut.contains("body") && cut.contains("more"));
        assert!(!cut.contains("after"));
    }

    /// The README's configuration section is a fenced file full of `#`
    /// comments. They are not headings and must not end the page.
    #[test]
    fn hashes_inside_a_fence_are_not_headings() {
        let text =
            "## conf\n\n```\n# ~/.config/poptop/poptop.conf\nrate = 1\n```\n\ntail\n\n## next\n";
        let cut = section(text, "## conf").unwrap();
        assert!(cut.contains("rate = 1"));
        assert!(cut.contains("tail"));
        assert!(!cut.contains("## next"));
    }

    #[test]
    fn neighbours_walk_the_sidebar_in_order() {
        let (prev, next) = neighbours("keys");
        assert_eq!(prev.map(|d| d.slug), Some("install"));
        assert_eq!(next.map(|d| d.slug), Some("prior-art"));
    }
}
