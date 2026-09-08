//! The landing page.
//!
//! The hero is the product, working: a real poptop frame with a real buffer
//! behind it that a visitor can drag. Everything a screenshot could say about
//! this tool has already been said by every other system monitor's screenshot;
//! the thing poptop does that they do not is an *interaction*, and there is no
//! honest way to show an interaction with a picture.

use crate::demo;
use crate::site;
use crate::views::layout::{Meta, Nav};
use crate::views::ui::{self, Status, Still};
use maud::{Markup, PreEscaped, html};
use std::sync::LazyLock;

pub fn meta() -> Meta {
    Meta::new(site::NAME, site::DESCRIPTION, "/", Nav::Home)
}

/// Where the incident peaks, and a stretch before it where nothing is wrong.
/// Both index the same buffer, so every picture on this page is the same
/// machine at a different second.
const PEAK: usize = 232;
const QUIET: usize = 40;

/// The hero drives itself for one moment on load: it opens on the quiet stretch
/// and travels back to the incident.
static HERO: LazyLock<ui::Frame> = LazyLock::new(|| ui::Frame {
    id: "hero-frame",
    rows: 5,
    quiet: QUIET,
    intro: true,
    caption: Some("[data-hero-caption]"),
    label: "A poptop session you can scrub through, from a generated buffer",
    ..Default::default()
});

/// The second one, under the claim that the process table rewinds. No opening:
/// the page has one moment and it is spent above.
static TABLE_FRAME: LazyLock<ui::Frame> = LazyLock::new(|| ui::Frame {
    id: "table-frame",
    rows: 3,
    quiet: QUIET,
    say: Some("[data-table-say]"),
    label: "The same session, with the process table under the cursor",
    class: "term--compact",
    ..Default::default()
});

pub fn render() -> Markup {
    html! {
        div class="wrap" {
            (hero())
            (problem())
            (nothing_first())
            (table_rewinds())
            (peak_not_mean())
            (colour())
            (versus())
            (keys())
            (close())
        }
    }
}

/* ── 1 · the hero ─────────────────────────────────────────────────────────
The page's one motion moment, spent on the argument rather than on movement:
what you see when you arrive, then what was true before you got there. */

fn hero() -> Markup {
    html! {
        div class="section section--hero" {
            div class="stack-lg" {
                div class="stack" {
                    h1 class="display" { "Scrub back to the moment it went wrong." }
                    p class="lede" {
                        "poptop keeps every sample it takes — including the full process
                         table — so you can rewind forty seconds and name the process that
                         did it. No daemon, no config, nothing that had to be running
                         before you noticed."
                    }
                }
                div class="row" {
                    (ui::command(site::INSTALL))
                    (ui::button("/docs/install", "Other ways to install", false))
                }
                (frame())
            }
        }
    }
}

fn frame() -> Markup {
    html! {
        figure class="stack figure" {
            (HERO.wrap(html! {
                div class="term__chrome" {
                    span class="term__title" { "poptop" }
                    span class="term__state" data-state data-live="false" { "PAUSED" }
                    span data-offset { "-0s" }
                    span { "warn 50 · crit 80" }
                }

                div class="term__screen" data-screen {
                    div {
                        div class="term__head" data-head {}
                        div class="term__rule" { span { "timeline — peak per slot, never mean" } }
                        div class="term__graph" {
                            pre class="term__gutter" data-gutter-cpu {}
                            pre class="term__plot term__plot--cpu" data-plot-cpu {}
                        }
                        div class="term__graph" {
                            pre class="term__gutter" data-gutter-mem {}
                            pre class="term__plot term__plot--mem" data-plot-mem {}
                        }
                        p class="term__gutter" style="text-align: right; margin-top: .25rem" {
                            span data-readout {}
                            span class="term__cursor" { "▐" }
                        }
                        p class="term__gutter" style="text-align: left" data-span {}

                        div class="term__rule" {
                            span { "processes (" span data-count { "5" } ") — sort: CPU" }
                        }
                        table class="term__table" {
                            thead {
                                tr {
                                    th class="term__num" scope="col" { "CPU%" }
                                    th scope="col" { span class="sr-only" { "load" } }
                                    th class="term__num" scope="col" { "RSS" }
                                    th scope="col" { "S" }
                                    th class="term__num" scope="col" { "THR" }
                                    th scope="col" { "HIST" }
                                    th class="term__num" scope="col" { "PID" }
                                    th scope="col" { "COMMAND" }
                                }
                            }
                            tbody data-procs {}
                        }
                    }
                }

                div class="term__footer" {
                    button class="btn btn--quiet" type="button" data-act="back"
                        aria-label="Scrub ten samples back" { "←" }
                    button class="btn btn--quiet" type="button" data-act="live" { "live" }
                    button class="btn btn--quiet" type="button" data-act="forward"
                        aria-label="Scrub ten samples forward" { "→" }
                    button class="btn btn--quiet" type="button" data-act="zoom-out"
                        aria-label="Zoom the timeline out" { "-" }
                    button class="btn btn--quiet" type="button" data-act="zoom-in"
                        aria-label="Zoom the timeline in" { "+" }
                    span class="term__hint" {
                        "drag the plot, or " kbd { "←" } " " kbd { "→" }
                        " to scrub · " kbd { "Space" } " for live"
                    }
                }
            }))

            p class="sr-only" aria-live="polite" data-say {}

            // Two states, swapped by the intro. Both are in the markup, so the
            // sentence is there for a reader whose script never runs.
            figcaption class="caption caption--moment" data-hero-caption data-moment="now" {
                span class="caption__now" { "This is what you see when you arrive." }
                span class="caption__then" {
                    "This is what was happening forty seconds earlier. "
                    "Now you drive: drag it, or use " kbd { "←" } " " kbd { "→" } "."
                }
                // Said plainly and not in a footnote. The buffer is generated
                // from a fixed seed, not captured from a machine — and a page
                // that spends a section on where poptop loses does not get to
                // be vague about where its own picture came from.
                span class="caption__source" {
                    " The buffer is generated from a fixed seed, not recorded from a real
                     machine, so the same incident is here for everyone. The braille encoding and
                     the scrubbing are poptop's, drawn again in the browser — the terminal
                     does it in Rust."
                }
            }

            noscript {
                p class="callout" { (demo::peak_summary()) " "
                    a href=(site::REPO) { "The README has the full frame as text." } }
            }
        }

        // The buffer. Rendered on the server from a fixed seed, so it is the
        // same incident for every visitor and identical between builds.
        script type="application/json" id="demo-buffer" { (PreEscaped(demo::buffer_json())) }
    }
}

/* ── 2 · the problem ─────────────────────────────────────────────────────── */

fn problem() -> Markup {
    ui::section(ui::Section {
        id: "problem",
        gutter: "the problem",
        title: "you are always late",
        note: Some("same machine, forty seconds apart"),
        body: html! {
            p class="measure" {
                "The box spiked, the alert fired, you connected — and by the time the
                 monitor painted, everything was calm. Every system monitor shows you
                 the present, and the present is never when the problem happened."
            }

            div class="twin" {
                (ui::figure(
                    ui::still(Still {
                        cursor: QUIET,
                        rows: 4,
                        chrome: Some("what you see when you arrive".into()),
                        table: true,
                        class: "term--quiet",
                        ..Default::default()
                    }),
                    html! { "Nothing above four percent. Correct, and useless." },
                ))
                div class="twin__gap" aria-hidden="true" { span { "40s" } }
                (ui::figure(
                    ui::still(Still {
                        cursor: PEAK,
                        rows: 4,
                        chrome: Some("what happened before you got there".into()),
                        table: true,
                        ..Default::default()
                    }),
                    html! { "The same buffer, forty seconds earlier. " code { "postgres" } " at 74%." },
                ))
            }

            p class="measure" {
                "Both are the same generated buffer at two cursors — the one in the frame
                 above, which you can scrub to either of them yourself."
            }
        },
    })
}

/* ── 3 · nothing had to be running first ─────────────────────────────────── */

fn nothing_first() -> Markup {
    ui::section(ui::Section {
        id: "start",
        gutter: "the trade",
        title: "nothing had to be running first",
        note: Some("and nothing before that is recoverable"),
        body: html! {
            p class="measure" {
                "poptop starts with an empty buffer and fills it as it runs. There is no
                 daemon that should have been enabled last month, no logfiles, and no
                 configuration. You notice the problem, then you start poptop — and from
                 that second, you can rewind."
            }

            (ui::figure(
                ui::still(Still {
                    cursor: PEAK,
                    rows: 4,
                    // Everything before `from` is time poptop was not running
                    // for, drawn as absent rather than as zero.
                    from: 96,
                    class: "term--band",
                    ..Default::default()
                }),
                html! {
                    "Left of the mark is time poptop was not running for: no data, and no "
                    "daemon that should have been collecting it. atop would have this "
                    "third — and would have needed enabling last month to get it."
                },
            ))

            div class="band-key" aria-hidden="true" {
                span class="band-key__before" { "before you started poptop" }
                span class="band-key__mark" { "you noticed the problem" }
                span class="band-key__after" { "buffered, and scrubbable" }
            }
        },
    })
}

/* ── 4 · the table rewinds too ───────────────────────────────────────────── */

fn table_rewinds() -> Markup {
    ui::section(ui::Section {
        id: "table",
        gutter: "the proof",
        title: "the table rewinds too",
        note: Some("scrub it yourself"),
        body: html! {
            p class="measure" {
                "zenith has zoomable scroll-back, but its history holds aggregate series
                 only: its process table renders from a live map that drops pids as they
                 exit, so scrolling back moves the charts and not the rows. The table is
                 what names the process."
            }

            figure class="figure" {
                (TABLE_FRAME.wrap(html! {
                    div class="term__screen" data-screen {
                        div {
                            div class="term__graph" {
                                pre class="term__gutter" data-gutter-cpu {}
                                pre class="term__plot term__plot--cpu" data-plot-cpu {}
                            }
                            div class="term__rule" { span { "processes under the cursor" } }
                            table class="term__table" {
                                thead {
                                    tr {
                                        th class="term__num" scope="col" { "CPU%" }
                                        th scope="col" { span class="sr-only" { "load" } }
                                        th class="term__num" scope="col" { "RSS" }
                                        th scope="col" { "S" }
                                        th class="term__num" scope="col" { "THR" }
                                        th scope="col" { "HIST" }
                                        th class="term__num" scope="col" { "PID" }
                                        th scope="col" { "COMMAND" }
                                    }
                                }
                                tbody data-procs {}
                            }
                        }
                    }
                    div class="term__footer" {
                        button class="btn btn--quiet" type="button" data-act="back"
                            aria-label="Scrub ten samples back" { "←" }
                        button class="btn btn--quiet" type="button" data-act="forward"
                            aria-label="Scrub ten samples forward" { "→" }
                        span class="term__hint" {
                            "drag it — " code { "rustc" } " appears during its own burst, "
                            "and is gone afterwards"
                        }
                    }
                }))
                p class="sr-only" aria-live="polite" data-table-say {}
                figcaption class="caption" {
                    "The rows are the ones that were running at that second — not an
                     interpolation, and not today's processes drawn against yesterday's
                     numbers. A row that changed position carries a mark."
                }
            }
        },
    })
}

/* ── 5 · peak, never mean ────────────────────────────────────────────────── */

fn peak_not_mean() -> Markup {
    ui::section(ui::Section {
        id: "aggregation",
        gutter: "the detail",
        title: "peak, never mean",
        note: Some("the same samples, twice"),
        body: html! {
            p class="measure" {
                "Zooming out has to put several samples in one slot. Averaging a
                 saturated second with three idle ones renders 25% — and hides the exact
                 event the tool exists to catch."
            }

            div class="twin twin--pair" {
                (ui::figure(
                    ui::still(Still {
                        // 40 cells at four samples a slot is 320 samples, which
                        // is the whole buffer: both plots show all of it, and
                        // differ only in how each slot was reduced.
                        cursor: 319,
                        rows: 4,
                        step: 4,
                        cols: Some(40),
                        aggregate: ui::Aggregate::Mean,
                        chrome: Some("aggregated by mean".into()),
                        series: &["spike"],
                        class: "term--wrong",
                        ..Default::default()
                    }),
                    html! { "The spike is not there. Nothing tells you it ever was." },
                ))
                (ui::figure(
                    ui::still(Still {
                        cursor: 319,
                        rows: 4,
                        step: 4,
                        cols: Some(40),
                        aggregate: ui::Aggregate::Peak,
                        chrome: Some("aggregated by peak — what poptop does".into()),
                        series: &["spike"],
                        ..Default::default()
                    }),
                    html! { "Same samples, same slots. The event survives the zoom." },
                ))
            }
        },
    })
}

/* ── 6 · colour ──────────────────────────────────────────────────────────── */

fn colour() -> Markup {
    ui::section(ui::Section {
        id: "colour",
        gutter: "the craft",
        title: "legible without colour",
        note: Some("measured, and enforced in CI"),
        body: html! {
            p class="measure" {
                "Green-and-yellow is the worst available pair for red-green colour vision
                 deficiency, which affects roughly 8% of men, and every system monitor
                 ships it. poptop replaces green with cyan and proves the difference
                 rather than claiming it."
            }

            (crate::views::cvd::demo())

            p class="measure" {
                "Separation is measured in OKLab ΔE×100 under Machado 2009 simulation,
                 and a test fails the build if any pair among the five meaning-bearing
                 hues drops below 8. "
                a href="/docs/themes" { "Themes and tiers" } "."
            }

            // Both figures are computed, not typed. If the palette moves, the
            // page moves with it — and `crate::cvd` is pinned against the
            // tool's own `src/cvd.rs`, so neither can drift alone.
            @let convention = crate::cvd::delta_e("#00cd00", "#cdcd00", crate::cvd::Vision::Protan);
            @let poptop = crate::cvd::worst_across("#ff6666", "#b48ead");
            div class="cols" {
                (ui::stat(
                    "green ↔ yellow, under protanopia",
                    &format!("{convention:.1}"),
                    ui::badge(Status::Critical, "below the floor of 8"),
                ))
                (ui::stat(
                    "poptop's worst pair, every vision",
                    &format!("{poptop:.1}"),
                    ui::badge(Status::Ok, "enforced by a test"),
                ))
            }
        },
    })
}

/* ── 7 · where poptop loses ──────────────────────────────────────────────── */

fn versus() -> Markup {
    ui::section(ui::Section {
        id: "versus",
        gutter: "the honest bit",
        title: "atop is the better tool",
        note: Some("and here is exactly when"),
        body: html! {
            p class="lede" style="max-width: min(46ch, 100%)" {
                "atop captures processes that started " em { "and finished" }
                " between two samples. poptop can only tell you they happened."
            }
            p class="measure" {
                "If a burst of short-lived processes spiked your machine, atop can name
                 them and poptop cannot — it reports a count. That is a real gap, it is
                 written down in "
                a href="/docs/design/data-fidelity" { "the design notes" }
                ", and no amount of the rest of this page closes it."
            }
            p class="measure" {
                "What poptop has instead is a process table that rewinds, with nothing to
                 install first."
            }

            details class="disclose" {
                summary { "The full comparison" }
                div class="table-scroll" {
                    table class="table" {
                        caption class="sr-only" {
                            "How poptop compares with atop, zenith and htop on history"
                        }
                        thead {
                            tr {
                                th scope="col" { "" }
                                th scope="col" { "History of the process table" }
                                th scope="col" { "Needs setting up first" }
                                th scope="col" { "Short-lived processes" }
                            }
                        }
                        tbody {
                            tr {
                                th scope="row" { "poptop" }
                                td { "Yes, back to when you started it" }
                                td { "No" }
                                td { "Counted, not named" }
                            }
                            tr {
                                th scope="row" { "atop" }
                                td { "Yes, 28 days by default" }
                                td { "Yes — a daemon writing daily logs" }
                                td { "Named" }
                            }
                            tr {
                                th scope="row" { "zenith" }
                                td { "Aggregate series only" }
                                td { "No" }
                                td { "No" }
                            }
                            tr {
                                th scope="row" { "htop, btop, bottom" }
                                td { "None" }
                                td { "No" }
                                td { "No" }
                            }
                        }
                    }
                }
                p { a href="/docs/prior-art" { "The long version" } }
            }
        },
    })
}

/* ── 8 · keys ────────────────────────────────────────────────────────────── */

fn keys() -> Markup {
    // Keys the browser frame implements drive it; the rest are documentation
    // and say so, because a key that claims to work and does nothing is worse
    // than a table.
    let live = |keys: &[&str], act: &str, what: &str| {
        html! {
            div class="keymap__row" {
                span class="keymap__keys" {
                    @for k in keys {
                        button class="kbd kbd--live" type="button"
                            data-act=(act) data-act-for="hero-frame"
                            aria-label=(format!("{what} — runs on the frame above")) { (k) }
                    }
                }
                span class="keymap__what" { (what) }
            }
        }
    };

    ui::section(ui::Section {
        id: "keys",
        gutter: "the whole of it",
        title: "ten keys",
        note: Some("it fits on one screen"),
        body: html! {
            div class="keymap" {
                (live(&["←"], "back", "scrub back — Shift for ten at a time"))
                (live(&["→"], "forward", "scrub forward"))
                (live(&["+"], "zoom-in", "zoom the timeline in"))
                (live(&["-"], "zoom-out", "zoom the timeline out"))
                (live(&["Space"], "live", "pause on this sample, or resume live"))
                (live(&["Home"], "oldest", "jump to the oldest sample"))
                (live(&["End"], "newest", "jump to live"))
                (ui::binding(&["↑", "↓"], "select a process"))
                (ui::binding(&["s"], "cycle the sort column"))
                (ui::binding(&["t"], "toggle the process tree"))
                (ui::binding(&["i"], "toggle per-process disk IO columns"))
                (ui::binding(&["/"], "filter by name or pid"))
                (ui::binding(&["q"], "quit"))
            }
            p class="caption" {
                "The first seven run on the frame at the top of this page. The rest need
                 a terminal."
            }
        },
    })
}

/* ── 9 · the close ───────────────────────────────────────────────────────── */

fn close() -> Markup {
    ui::section(ui::Section {
        id: "install",
        gutter: "get it",
        title: "one line",
        note: None,
        body: html! {
            (ui::command(site::INSTALL))
            p class="measure dim" {
                "Linux reads " code { "/proc" } " directly. macOS is supported through
                 sysinfo and is the platform poptop is developed on. One binary, no
                 runtime dependencies, and nothing written to disk unless you ask for it."
            }

            (ui::figure(
                ui::still(Still {
                    // A buffer with nothing in it yet: the honest picture of
                    // the first second after you start. `empty` says so, so the
                    // audit asserts it is blank rather than asserting it drew.
                    rows: 3,
                    from: 400,
                    empty: true,
                    class: "term--empty",
                    ..Default::default()
                }),
                html! { "It starts empty. Then it starts remembering." },
            ))

            (ui::link_list(&[
                ("/docs/install", "Install", "Build from source, or take it from crates.io"),
                ("/docs/keys", "Keys", "Every binding, and what it does while paused"),
                ("/docs/configuration", "Configuration", "Every setting, with its default"),
                ("/community", "Contributing", "How the project takes changes, and what it asks for"),
            ]))
        },
    })
}
