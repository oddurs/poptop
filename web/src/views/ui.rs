//! The component library, in Rust.
//!
//! Every function here is rendered live on `/design`. That page is generated
//! from these same functions rather than from a copy of their markup, so it
//! cannot describe a component the site does not actually have — which is the
//! usual failure of a style guide and the reason most of them stop being read.

use maud::{Markup, html};

/// A titled hairline, the way each panel opens in the terminal:
///
/// ```text
/// ── timeline — 4m59s of 9m59s buffered ──────────────────────────
/// ```
///
/// This is the plain form, for a rule that titles a list rather than a
/// section. When the rule titles a section, use [`section`], which renders the
/// same thing as a real `<h2>` — the rule *looks* like the section's heading,
/// so it had better be one. Before this split the document outline went h1
/// straight to h3 and a screen-reader user got no section structure at all.
pub fn rule(label: &str, note: Option<&str>) -> Markup {
    rule_inner("div", None, label, note)
}

/// The same rule, as a heading. For a `<section>` built by hand rather than
/// through [`section`].
pub fn rule_heading(label: &str, note: Option<&str>) -> Markup {
    rule_inner("h2", None, label, note)
}

fn rule_inner(tag: &str, id: Option<&str>, label: &str, note: Option<&str>) -> Markup {
    let body = html! {
        span { (label) }
        @if let Some(note) = note {
            span class="rule__note" { (note) }
        }
    };
    html! {
        @if tag == "h2" {
            h2 class="rule" id=[id] { (body) }
        } @else {
            div class="rule" { (body) }
        }
    }
}

/// A section with a gutter label, which is the site's only structural device.
///
/// Four of these fields are strings and two of them appear side by side in the
/// header, which is exactly the shape that should not be positional — a reader
/// was having to count commas to tell the gutter from the heading.
pub struct Section<'a> {
    /// Anchor for the section, and the stem of its heading's id.
    pub id: &'a str,
    /// The word in the left margin. Names the section the way the graph gutter
    /// names a series, and marks position rather than restating the heading.
    pub gutter: &'a str,
    /// The rule that opens the section, and its `<h2>`.
    pub title: &'a str,
    /// The quieter half of the rule, after the title.
    pub note: Option<&'a str>,
    pub body: Markup,
}

pub fn section(s: Section<'_>) -> Markup {
    let heading_id = format!("{}-title", s.id);
    html! {
        section class="section" id=(s.id) aria-labelledby=(heading_id) {
            div class="section__grid" {
                // Hidden from the outline on purpose: the heading beside it
                // already carries the meaning, and this carries the position.
                p class="section__label" aria-hidden="true" { (s.gutter) }
                div class="section__body" {
                    (rule_inner("h2", Some(&heading_id), s.title, s.note))
                    (s.body)
                }
            }
        }
    }
}

/// The install line, with a copy button that says what it did.
pub fn command(text: &str) -> Markup {
    html! {
        div class="cmd" {
            code class="cmd__text" { (text) }
            button class="cmd__copy" type="button" data-copy=(text)
                aria-label=(format!("Copy “{text}” to the clipboard")) { "copy" }
        }
    }
}

/// A verdict, a state, a classification — reversed rather than coloured.
///
/// Swapping foreground and background is poptop's own convention for the
/// cursor: it reads on any palette, in either theme, and on a monochrome
/// display, and it leaves the status hues free to go on meaning a status.
///
/// Use [`badge`] instead for anything that means *good* or *bad*. A stamp
/// classifies; a badge judges, and carries a glyph so the judgement survives
/// without hue.
pub fn stamp(label: &str) -> Markup {
    html! { span class="stamp" { (label) } }
}

/// The same classification, for the row that is not the point.
pub fn stamp_quiet(label: &str) -> Markup {
    html! { span class="stamp stamp--quiet" { (label) } }
}

/// A passage with a label in the margin naming whose it is.
///
/// A run of these scans as a column of names, which is the reason to reach for
/// it over [`callout`], whose label is part of the sentence.
pub fn attributed(label: &str, body: Markup) -> Markup {
    html! {
        div class="attributed" {
            p class="attributed__label" { (label) }
            div class="attributed__body" { (body) }
        }
    }
}

/// The same, given the weight of a rule rather than an aside.
pub fn attributed_strong(label: &str, body: Markup) -> Markup {
    html! {
        div class="attributed attributed--strong" {
            p class="attributed__label" { (label) }
            div class="attributed__body" { (body) }
        }
    }
}

/// A dense index: many rows, few words each, read by scanning one column.
///
/// `caption` is for a screen reader — the visible framing is the section around
/// it. Rows are written by the caller so a cell can hold whatever it needs,
/// usually a [`stamp`] or a [`badge`].
pub fn ledger(caption: &str, columns: &[&str], rows: Markup) -> Markup {
    html! {
        div class="table-scroll" {
            table class="ledger" {
                caption class="sr-only" { (caption) }
                thead {
                    tr {
                        @for column in columns {
                            th scope="col" { (column) }
                        }
                    }
                }
                tbody { (rows) }
            }
        }
    }
}

pub enum Status {
    Ok,
    Warn,
    Critical,
}

/// A state, shown as hue *and* glyph. Never hue alone — the badge has to keep
/// working for a reader who cannot tell the hues apart, which is the same rule
/// the terminal's monochrome tier enforces.
pub fn badge(status: Status, label: &str) -> Markup {
    let class = match status {
        Status::Ok => "badge badge--ok",
        Status::Warn => "badge badge--warn",
        Status::Critical => "badge badge--critical",
    };
    html! { span class=(class) { (label) } }
}

/// A keycap.
pub fn key(label: &str) -> Markup {
    html! { kbd { (label) } }
}

/// One row of the keymap: the keys, then what they do.
pub fn binding(keys: &[&str], what: &str) -> Markup {
    html! {
        div class="keymap__row" {
            span class="keymap__keys" {
                @for k in keys { (key(k)) }
            }
            span class="keymap__what" { (what) }
        }
    }
}

/// An aside. `label` carries the kind, so the callout does not rely on its
/// left border being a particular colour.
pub fn callout(label: &str, warn: bool, body: Markup) -> Markup {
    html! {
        p class=(if warn { "callout callout--warn" } else { "callout" }) {
            b class="callout__label" { (label) }
            (body)
        }
    }
}

/// A claim and its evidence. No box: a heading and a paragraph are enough
/// structure for three things side by side, and boxing them would make the
/// page look like a pricing table.
pub fn claim(head: &str, body: &str) -> Markup {
    html! {
        div {
            h3 class="claim__head" { (head) }
            p class="claim__body" { (body) }
        }
    }
}

/// A list of links where each has a name and a line about it.
pub fn link_list(items: &[(&str, &str, &str)]) -> Markup {
    html! {
        ul class="links" {
            @for (href, name, what) in items {
                li {
                    a href=(href) {
                        span class="links__name" { (name) }
                        span class="links__what" { (what) }
                    }
                }
            }
        }
    }
}

/// One measured figure, what it measures, and how to read it. The verdict is a
/// status badge rather than a colour on the number itself: the figure is data,
/// and whether it passes is a judgement about the figure.
pub fn stat(label: &str, value: &str, verdict: Markup) -> Markup {
    html! {
        div class="panel" {
            p class="panel__title" { (label) }
            p class="stat__value" { (value) }
            (verdict)
        }
    }
}

/// A frame that does not respond to anyone.
///
/// The sections below the hero argue with pictures of the tool, and those
/// pictures have to be drawn by the same code that draws the interactive one —
/// see `assets/js/frame.js`. A still declares what it wants in `data-frame`
/// and `frame.js` fills it in.
///
/// It is `aria-hidden`, and its caption carries the meaning instead. A screen
/// reader has no use for two thousand braille cells, and a caption that says
/// what the picture shows is better writing anyway.
pub struct Still {
    /// Which sample the frame is drawn at.
    pub cursor: usize,
    /// How many braille rows each plot is. One field, rendered three ways —
    /// into the JSON the browser reads, into the CSS custom property that
    /// reserves the height, and into nothing else. It used to be written out
    /// three times with a comment asking the reader to keep them in step.
    pub rows: u8,
    /// Samples per half-cell. One is the finest the buffer holds.
    pub step: u8,
    /// How a slot reduces the samples in it: peak, or the mistake.
    pub aggregate: Aggregate,
    /// Where the buffer begins. Everything before it is time poptop was not
    /// running for, and is drawn as absent rather than as zero.
    pub from: usize,
    /// Fixed plot width in cells. `None` measures the frame and fills it.
    pub cols: Option<u16>,
    /// Shown in the window chrome. `None` draws no chrome bar.
    pub chrome: Option<String>,
    /// Which plots to draw, in order. `cpu`, `mem`, `wait` or `spike`.
    pub series: &'static [&'static str],
    pub head: bool,
    pub table: bool,
    /// Drop THR and HIST. A still is narrow and is making a point about which
    /// process, so it keeps the columns that answer that and loses the rest.
    pub compact: bool,
    /// This frame is meant to draw nothing — the closing one, which is what
    /// poptop looks like a second after you start it. The audit asserts it is
    /// blank rather than asserting it drew.
    pub empty: bool,
    /// Extra class on the frame, for the sections that dim or shrink one.
    pub class: &'static str,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Aggregate {
    /// What poptop does: a slot reports the highest sample in it.
    Peak,
    /// What poptop refuses to do, drawn once so the reader can see why.
    Mean,
}

impl Default for Still {
    fn default() -> Self {
        Self {
            cursor: 0,
            rows: 5,
            step: 1,
            aggregate: Aggregate::Peak,
            from: 0,
            cols: None,
            chrome: None,
            series: &["cpu"],
            head: false,
            table: false,
            compact: true,
            empty: false,
            class: "",
        }
    }
}

impl Still {
    /// The configuration `frame.js` reads, built from the fields above. Written
    /// by hand because this crate has no serialiser and does not want one for
    /// six keys — but built from one place, which was the whole complaint.
    fn config(&self) -> String {
        let mut out = format!(
            "{{\"cursor\":{},\"rows\":{},\"step\":{},\"from\":{},\"aggregate\":\"{}\"",
            self.cursor,
            self.rows,
            self.step,
            self.from,
            match self.aggregate {
                Aggregate::Peak => "peak",
                Aggregate::Mean => "mean",
            }
        );
        if let Some(cols) = self.cols {
            out.push_str(&format!(",\"cols\":{cols}"));
        }
        if self.table {
            out.push_str(",\"table\":true");
        }
        if self.compact {
            out.push_str(",\"compact\":true");
        }
        if self.head {
            out.push_str(",\"head\":true");
        }
        if self.empty {
            out.push_str(",\"empty\":true");
        }
        out.push('}');
        out
    }
}

/// A frame that answers to a person: the landing hero, and the one under the
/// process-table claim.
///
/// Separate from [`Still`] because the fields are different — a still has an
/// aggregate and a start index, a driven frame has an opening and a caption —
/// but for the same reason: the row count has to reach the JSON and the CSS
/// custom property from one field, or the height reserved before the script
/// runs stops matching the height the script draws.
pub struct Frame {
    pub id: &'static str,
    pub rows: u8,
    /// A quiet stretch of the buffer: the sample the frame opens on, and the
    /// one live playback wraps back to.
    pub quiet: usize,
    /// Play the opening: hold on `quiet`, then travel back to the incident.
    pub intro: bool,
    /// Selector for the caption whose two states the opening swaps.
    pub caption: Option<&'static str>,
    /// Selector for the live region this frame speaks its readout into.
    pub say: Option<&'static str>,
    pub label: &'static str,
    pub class: &'static str,
}

impl Default for Frame {
    fn default() -> Self {
        Self {
            id: "frame",
            rows: 5,
            quiet: 40,
            intro: false,
            caption: None,
            say: None,
            label: "A poptop session you can scrub through",
            class: "",
        }
    }
}

impl Frame {
    pub fn config(&self) -> String {
        let mut out = format!("{{\"rows\":{},\"quiet\":{}", self.rows, self.quiet);
        if self.intro {
            out.push_str(",\"intro\":true");
        }
        if let Some(caption) = self.caption {
            out.push_str(&format!(",\"caption\":\"{caption}\""));
        }
        out.push('}');
        out
    }

    /// Wraps the frame's contents in the element that carries its attributes.
    /// Kept here so the row count reaches `data-demo` and `--rows` from the
    /// same field: a mismatch reserves the wrong height before the script runs,
    /// which is the layout shift the reservation exists to prevent.
    pub fn wrap(&self, body: Markup) -> Markup {
        html! {
            div class=(format!("term {}", self.class)) id=(self.id) tabindex="0" role="group"
                style=(format!("--rows: {}", self.rows))
                data-demo=(self.config())
                data-say=[self.say]
                aria-label=(self.label) {
                (body)
            }
        }
    }
}

pub fn still(s: Still) -> Markup {
    html! {
        div class=(format!("term term--still {}", s.class)) data-frame=(s.config())
            style=(format!("--rows: {}", s.rows)) aria-hidden="true" {
            @if let Some(chrome) = &s.chrome {
                div class="term__chrome" { span class="term__title" { (chrome) } }
            }
            div class="term__screen" data-screen {
                div {
                    @if s.head {
                        div class="term__head" data-head {}
                    }
                    @for key in s.series {
                        div class="term__graph" {
                            pre class="term__gutter"
                                data-gutter-cpu[*key == "cpu"]
                                data-gutter-mem[*key == "mem"]
                                data-gutter-wait[*key == "wait"]
                                data-gutter-spike[*key == "spike"] {}
                            pre class=(format!("term__plot term__plot--{key}"))
                                data-plot-cpu[*key == "cpu"]
                                data-plot-mem[*key == "mem"]
                                data-plot-wait[*key == "wait"]
                                data-plot-spike[*key == "spike"] {}
                        }
                    }
                    @if s.table {
                        // The header names exactly the columns `frame.js`
                        // draws for this configuration. A header that does not
                        // match its own rows is the kind of thing nobody
                        // notices until they try to read it.
                        table class="term__table" {
                            thead {
                                tr {
                                    th class="term__num" scope="col" { "CPU%" }
                                    th scope="col" {}
                                    th class="term__num" scope="col" { "RSS" }
                                    th scope="col" { "S" }
                                    @if !s.compact {
                                        th class="term__num" scope="col" { "THR" }
                                        th scope="col" { "HIST" }
                                    }
                                    th class="term__num" scope="col" { "PID" }
                                    th scope="col" { "COMMAND" }
                                }
                            }
                            tbody data-procs {}
                        }
                    }
                }
            }
        }
    }
}

/// A still with the sentence it is making underneath it.
pub fn figure(still_markup: Markup, caption: Markup) -> Markup {
    html! {
        figure class="figure" {
            (still_markup)
            figcaption class="caption" { (caption) }
        }
    }
}

pub fn button(href: &str, label: &str, primary: bool) -> Markup {
    html! {
        a class=(if primary { "btn btn--primary" } else { "btn" }) href=(href) { (label) }
    }
}
