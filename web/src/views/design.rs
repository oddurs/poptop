//! The design system, rendered from itself.
//!
//! Every component below comes from `views::ui`, the same functions the rest of
//! the site calls. Nothing here is a copy of a component's markup, so this page
//! cannot document a version of a component that no longer exists — which is
//! how style guides normally die.

use crate::views::layout::{Meta, Nav};
use crate::views::ui::{self, Status};
use maud::{Markup, html};

/// A colour token: the variable, the job it does, and where it is allowed to
/// appear. The last column is the useful one — a palette that says only what a
/// colour *is* cannot tell you when you are misusing it.
struct Token {
    var: &'static str,
    dark: &'static str,
    job: &'static str,
    allowed: &'static str,
}

const STATUS: &[Token] = &[
    Token {
        var: "--ok",
        dark: "#5ccfe6",
        job: "Status — this is fine",
        allowed: "Header figures below the warn threshold, the LIVE badge, confirmations",
    },
    Token {
        var: "--warn",
        dark: "#ffd580",
        job: "Status — this is getting bad",
        allowed: "Values between the warn and critical thresholds",
    },
    Token {
        var: "--critical",
        dark: "#ff6666",
        job: "Status — this is bad now",
        allowed: "Values above the critical threshold",
    },
];

const IDENTITY: &[Token] = &[
    Token {
        var: "--series-cpu",
        dark: "#7a7ae6",
        job: "Identity — which series this is",
        allowed: "The CPU plot, and its gutter label",
    },
    Token {
        var: "--series-mem",
        dark: "#b48ead",
        job: "Identity — which series this is",
        allowed: "The memory plot, and its gutter label",
    },
];

const CHROME: &[Token] = &[
    Token {
        var: "--canvas",
        dark: "#101317",
        job: "Chrome — the page",
        allowed: "The body, and nothing else",
    },
    Token {
        var: "--surface",
        dark: "#161a20",
        job: "Chrome — a raised plane",
        allowed: "Panels, keycaps, the masthead",
    },
    Token {
        var: "--surface-sunk",
        dark: "#0c0e12",
        job: "Chrome — a recessed plane",
        allowed: "Code blocks and the terminal frame",
    },
    Token {
        var: "--line",
        dark: "#242a33",
        job: "Chrome — structure",
        allowed: "Every hairline. Borders are 1px and never coloured by status",
    },
    Token {
        var: "--ink",
        dark: "#ccd2d9",
        job: "Chrome — text",
        allowed: "Body text and headings",
    },
    Token {
        var: "--ink-dim",
        dark: "#8b939d",
        job: "Chrome — secondary text",
        allowed: "Supporting prose, table cells, inactive navigation",
    },
    Token {
        var: "--ink-faint",
        dark: "#5e6873",
        job: "Chrome — labels",
        allowed: "Gutter labels, rule titles, captions",
    },
];

pub fn meta() -> Meta {
    Meta::new(
        "Design system",
        "The tokens, type scale and components this site is built from — and the four \
         jobs a colour is allowed to do.",
        "/design",
        Nav::Design,
    )
}

pub fn render() -> Markup {
    html! {
        div class="wrap" {
            div class="section" style="padding-block: var(--s-7) var(--s-6)" {
                h1 class="h1" { "Design system" }
                p class="lede" style="margin-top: var(--s-4)" {
                    "The same rules the terminal interface follows, applied to a website.
                     Every component on this page is rendered by the function the rest of
                     the site calls, so nothing here can describe a component that no
                     longer exists."
                }
            }

            (rules())
            (colour())
            (typography())
            (space())
            (components())
        }
    }
}

fn rules() -> Markup {
    ui::section(ui::Section {
        id: "rules",
        gutter: "rules",
        title: "five rules",
        note: Some("in the order they cost to break"),
        body: html! {
            div {
                (ui::attributed_strong("status", html! {
                    p {
                        strong { "Status hues mean status." }
                        " A status hue reused as an identity hue destroys the meaning of
                         that hue everywhere else, because the reader can no longer tell
                         whether red means “this is bad” or merely “this is the third
                         thing”. There are no status hues on this site behind a button, a
                         link or a border."
                    }
                }))
                (ui::attributed_strong("channels", html! {
                    p {
                        strong { "No meaning rests on hue." }
                        " Every status carries a glyph as well as a colour, every
                         threshold is drawn as a rule as well as a change of shade, and
                         the current page in a navigation list is marked by weight as
                         well as colour. Turn the page monochrome and nothing stops
                         working."
                    }
                }))
                (ui::attributed_strong("surfaces", html! {
                    p {
                        strong { "Panels do not nest." }
                        " One border, one radius of 3px, no shadows. A terminal has none
                         of them, and a soft grey shadow under every panel is the fastest
                         way to make a tool for people who read htop look like a pricing
                         page."
                    }
                }))
                (ui::attributed_strong("motion", html! {
                    p {
                        strong { "One thing moves." }
                        " The scrubbable frame on the landing page, and nothing else. No
                         entrance animations, no hover lifts. Motion answers an action or
                         it does not happen."
                    }
                }))
                (ui::attributed_strong("provenance", html! {
                    p {
                        strong { "A picture of poptop says where it came from." }
                        " The landing page shows a simulation — generated data, drawn by
                         a second renderer — and says so in the same breath as the claim
                         it supports. This rule is the newest and was learnt: a rewrite
                         once deleted that sentence and the page spent a working session
                         calling seeded data a recording. "
                        code { "the_landing_page_says_where_its_data_came_from" }
                        " fails the build if it goes missing again."
                    }
                }))
            }
        },
    })
}

fn colour() -> Markup {
    ui::section(ui::Section {
        id: "colour",
        gutter: "colour",
        title: "tokens",
        note: Some("dark values shown; the light palette re-chooses them"),
        body: html! {
            p class="measure" {
                "The hues are the terminal interface's own, from "
                code { "themes/safe.theme" }
                ". They are not re-tinted for the light theme — they are re-chosen,
                 because #5ccfe6 on white is 1.9:1 and unreadable, and a palette that
                 fails in one of its two modes is a palette that fails."
            }

            h3 class="h3" style="margin-top: var(--s-6)" { "Status" }
            (swatches(STATUS))
            h3 class="h3" style="margin-top: var(--s-6)" { "Identity" }
            (swatches(IDENTITY))
            h3 class="h3" style="margin-top: var(--s-6)" { "Chrome" }
            (swatches(CHROME))

            h3 class="h3" style="margin-top: var(--s-6)" { "Cursor" }
            p class="measure dim" {
                "Position has no hue at all. It is drawn reversed — foreground and
                 background swapped — which is legible on any palette, in any theme, and
                 on a monochrome display."
            }
            p { span class="term__cursor" style="font-family: var(--font-mono); padding: 0 .5em" { "PAUSED" } }
        },
    })
}

fn swatches(tokens: &[Token]) -> Markup {
    html! {
        div class="swatches" style="margin-top: var(--s-4)" {
            @for t in tokens {
                div class="swatch" {
                    div class="swatch__chip" style=(format!("background: var({})", t.var)) {}
                    div class="swatch__meta" {
                        // The token name and its value are both machine text.
                        div class="mono" style="color: var(--ink)" { (t.var) }
                        div class="mono" { (t.dark) }
                        div class="swatch__job" style="margin-top: var(--s-3)" { (t.job) }
                        div class="swatch__job" { (t.allowed) }
                    }
                }
            }
        }
    }
}

/// A type role: the token, what it is set in, and where it is allowed. The last
/// column is the useful one — a scale that lists sizes tells you nothing about
/// when to reach for one.
struct Role {
    token: &'static str,
    spec: &'static str,
    used: &'static str,
}

const ROLES: &[Role] = &[
    Role {
        token: "--type-display",
        spec: "600 · 40–68px · 1.0 · −0.035em",
        used: "The landing headline. One per site.",
    },
    Role {
        token: "--type-title",
        spec: "600 · 30–40px · 1.1 · −0.025em",
        used: "The title of a page, and an h1 inside prose.",
    },
    Role {
        token: "--type-heading",
        spec: "600 · 23px · 1.25 · −0.015em",
        used: "A section within a page. Carries the hairline above it.",
    },
    Role {
        token: "--type-subhead",
        spec: "600 · 18px · 1.4 · −0.008em",
        used: "A heading inside a section; a claim; a link name.",
    },
    Role {
        token: "--type-lede",
        spec: "400 · 18–21px · 1.5",
        used: "The paragraph under a title. One, never two.",
    },
    Role {
        token: "--type-body",
        spec: "400 · 17px · 1.65",
        used: "Running text. Prose sets a step larger, at 18px on 1.62.",
    },
    Role {
        token: "--type-ui",
        spec: "400 · 15px · 1.5",
        used: "Navigation, buttons, tables, the sidebar — text you act on.",
    },
    Role {
        token: "--type-label",
        spec: "400 · 13px · 1.5 · +0.005em",
        used: "Rule titles, panel titles, badges, captions.",
    },
    Role {
        token: "--type-micro",
        spec: "400 · 12px · 1.45 · +0.015em",
        used: "Gutter labels, column headings in the footer. The floor.",
    },
    Role {
        token: "--type-code",
        spec: "400 · 14px · 1.55 · mono",
        used: "The machine voice: commands, code, token names, the frame.",
    },
];

fn typography() -> Markup {
    ui::section(ui::Section {
        id: "type",
        gutter: "type",
        title: "one rule decides the face",
        note: Some("and it is not about size"),
        body: html! {

            p class="lede" style="margin-bottom: var(--s-5)" {
                "Sans is the human voice. Mono is the machine voice."
            }

            p class="measure" {
                "If a person wrote it, it is Instrument Sans — headings, prose, buttons,
                 navigation, captions, table headers. If the machine printed it, or you
                 would type it, it is monospace: the terminal frame, code, commands, file
                 paths, keycaps, token names. That is why "
                code { "$ poptop" }
                " in the masthead is still mono. It is not a wordmark, it is a shell
                 prompt, and poptop is a thing you type."
            }

            div class="cols" style="margin-top: var(--s-6)" {
                div class="panel" {
                    p class="panel__title" { "--font-sans · Instrument Sans" }
                    p style="font: var(--type-title); letter-spacing: var(--track-title)" {
                        "Scrub back 40s"
                    }
                    p class="keymap__what" style="margin-top: var(--s-3)" {
                        "One variable file, 400–700, self-hosted and embedded in the
                         binary. 84 KB for the family, under the SIL Open Font Licence.
                         Nothing is fetched from anyone else's server to read a page about
                         a system monitor."
                    }
                }
                div class="panel" {
                    p class="panel__title" { "--font-mono · the reader's own" }
                    p style="font-family: var(--font-mono); font-size: 1.5rem; letter-spacing: -0.02em" {
                        "⣿⣇⣿⣿ 88.4"
                    }
                    p class="keymap__what" style="margin-top: var(--s-3)" {
                        "A system stack, deliberately not a webfont. The frame draws
                         braille, which almost no webfont covers, and a plot whose glyphs
                         come from a different face than the text around them stops lining
                         up with its own gutter."
                    }
                }
            }

            h3 class="h3" style="margin-top: var(--s-7)" { "Roles" }
            p class="keymap__what" style="max-width: var(--measure); margin-bottom: var(--s-4)" {
                "Each token is a "
                code { "font" }
                " shorthand, so a size cannot be used without the line height it was drawn
                 for — the commonest way a type scale comes apart. Tracking rides
                 alongside, because the shorthand cannot carry it."
            }

            div class="table-scroll" {
                table class="table" {
                    thead {
                        tr {
                            th scope="col" { "Token" }
                            th scope="col" { "Specimen" }
                            th scope="col" { "Weight · size · leading · tracking" }
                            th scope="col" { "Where" }
                        }
                    }
                    tbody {
                        @for role in ROLES {
                            tr {
                                th scope="row" { code { (role.token) } }
                                td {
                                    span style=(format!(
                                        "font: var({}); letter-spacing: var({})",
                                        role.token,
                                        tracking(role.token),
                                    )) { "Ag" }
                                }
                                td class="nowrap" { (role.spec) }
                                td { (role.used) }
                            }
                        }
                    }
                }
            }

            h3 class="h3" style="margin-top: var(--s-7)" { "Measure" }
            p class="measure" {
                "Prose is capped at 66 characters and a lede at 54, both also capped
                 against the container — a bare "
                code { "ch" }
                " maximum does not shrink below the viewport, and on a phone it is the
                 thing that pushes the whole document sideways."
            }
        },
    })
}

/// The tracking token that goes with a type token. Two tokens rather than one
/// because the `font` shorthand cannot carry letter-spacing, so this is the
/// single place the pairing is written down.
fn tracking(type_token: &str) -> &'static str {
    match type_token {
        "--type-display" => "--track-display",
        "--type-title" => "--track-title",
        "--type-heading" => "--track-heading",
        "--type-subhead" => "--track-subhead",
        "--type-label" => "--track-label",
        "--type-micro" => "--track-micro",
        _ => "--track-body",
    }
}

fn space() -> Markup {
    let steps: &[(&str, &str)] = &[
        ("--s-1", "0.25rem"),
        ("--s-2", "0.5rem"),
        ("--s-3", "0.75rem"),
        ("--s-4", "1rem"),
        ("--s-5", "1.5rem"),
        ("--s-6", "2rem"),
        ("--s-7", "3rem"),
        ("--s-8", "4.5rem"),
        ("--s-9", "7rem"),
    ];

    ui::section(ui::Section {
        id: "space",
        gutter: "space",
        title: "space",
        note: Some("nine steps, and no value outside them"),
        body: html! {
            div style="margin-top: var(--s-4)" {
                @for (name, size) in steps {
                    div class="scale-row" {
                        span class="scale-row__name" { (name) }
                        span class="scale-row__box" style=(format!("width: var({name})")) {}
                        span class="keymap__what" style="margin-inline-start: auto" { (size) }
                    }
                }
            }
        },
    })
}

fn components() -> Markup {
    ui::section(ui::Section {
        id: "components",
        gutter: "parts",
        title: "components",
        note: Some("rendered by the same functions the site calls"),
        body: html! {

            (specimen("Titled rule", "The only structural device on the site. It is the \
                terminal's own panel header: a hairline carrying a name.",
                ui::rule("timeline", Some("4m59s of 9m59s buffered"))))

            (specimen("Command", "An install line that can be copied. The button says what \
                happened, then goes back to saying what it does.",
                ui::command("cargo install poptop")))

            (specimen("Stamp", "A verdict, a state, a classification — reversed rather \
                than coloured. That is the frame's own convention for the cursor: it \
                reads on any palette, in either theme, and on a monochrome display, and \
                it leaves the status hues free to go on meaning a status. A stamp \
                classifies; the badge below judges.", html! {
                div class="row" {
                    (ui::stamp("NAMED"))
                    (ui::stamp("COUNTED"))
                    (ui::stamp_quiet("NO"))
                    (ui::stamp_quiet("AGGREGATE ONLY"))
                }
            }))

            (specimen("Attributed block", "A passage with a label in the margin naming \
                whose it is. A run of them scans as a column of names, which is the \
                reason to reach for this over a callout, whose label is part of the \
                sentence.", html! {
                div {
                    (ui::attributed("peak", html! {
                        p { "A slot reports the highest sample in it, so an event cannot
                             be lost to a zoom level." }
                    }))
                    (ui::attributed("mean", html! {
                        p { "What poptop refuses to do, drawn once on the landing page so
                             the reader can see why." }
                    }))
                }
            }))

            (specimen("Ledger", "A dense index: many rows, few words each, read by \
                scanning one column rather than across. Column heads take the label \
                role, row heads take the weight, and figures are tabular.",
                ui::ledger(
                    "An example ledger",
                    &["Tier", "Colours", "Meaning survives"],
                    html! {
                        tr {
                            th scope="row" { "true colour" }
                            td { "16.7M" }
                            td { (ui::stamp("FULL")) }
                        }
                        tr {
                            th scope="row" { "256" }
                            td { "216 cube, 24 greys" }
                            td { (ui::stamp("FULL")) }
                        }
                        tr {
                            th scope="row" { "ansi 16" }
                            td { "slots, not colours" }
                            td { (ui::stamp_quiet("READABLE")) }
                        }
                        tr {
                            th scope="row" { "mono" }
                            td { "none" }
                            td { (ui::stamp("PROOF")) }
                        }
                    },
                )))

            (specimen("Status badge", "Hue and glyph together. Never hue alone.", html! {
                div class="row" {
                    (ui::badge(Status::Ok, "live"))
                    (ui::badge(Status::Warn, "above 50%"))
                    (ui::badge(Status::Critical, "above 80%"))
                }
            }))

            (specimen("Keys", "Keycaps, and a keymap row.", html! {
                div class="keymap" style="max-width: 26rem" {
                    (ui::binding(&["←", "→"], "scrub — Shift for ten at a time"))
                    (ui::binding(&["Space"], "pause here, or resume live"))
                }
            }))

            (specimen("Callout", "An aside. The label carries the kind, so the callout \
                does not depend on its border being a particular colour.", html! {
                div class="stack" {
                    (ui::callout("Note.", false, html! { "Zooming aggregates by peak, never mean." }))
                    (ui::callout("Careful.", true, html! {
                        "Writing history to disk is off by default, and stays off until
                         you name a path."
                    }))
                }
            }))

            (specimen("Claim", "A heading and its evidence, unboxed. Three of these across \
                a row is a section; boxing them would make it a pricing table.", html! {
                div class="cols" {
                    (ui::claim("The table rewinds too",
                        "The rows under the cursor are the ones that were running at that \
                         second, not an interpolation."))
                    (ui::claim("Zoom keeps the spike",
                        "Aggregation is by peak. Averaging a saturated second with three \
                         idle ones renders 25%."))
                }
            }))

            (specimen("Link list", "A name, and one line about it. Used wherever a page \
                hands the reader onward.",
                ui::link_list(&[
                    ("/docs/keys", "Keys", "Every binding, and what it does while paused"),
                    ("/docs/themes", "Themes", "The palette, the tiers, and writing your own"),
                ])))

            (specimen("Buttons", "Two weights. There is no third, and no ghost variant \
                waiting to be invented.", html! {
                div class="row" {
                    (ui::button("/docs/install", "Install poptop", true))
                    (ui::button("/roadmap", "Read the roadmap", false))
                }
            }))

            (specimen("Panel", "One border, one radius, no shadow. Panels do not nest.", html! {
                div class="panel" {
                    p class="panel__title" { "worst pair, OKLab ΔE×100" }
                    p class="stat__value" { "10.3" }
                }
            }))
        },
    })
}

fn specimen(name: &str, what: &str, body: Markup) -> Markup {
    html! {
        div style="margin-top: var(--s-7)" {
            h3 class="h3" { (name) }
            p class="keymap__what" style="max-width: var(--measure); margin-bottom: var(--s-4)" { (what) }
            div class="panel panel--sunk" { (body) }
        }
    }
}
