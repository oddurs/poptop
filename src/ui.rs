//! Rendering.
//!
//! Layout, top to bottom: a summary header, per-core meters, the timeline
//! (the reason this tool exists), the process table, and a help line.

use crate::app::{self, App};
use crate::glyphs::{self, GlyphSet};
use crate::history;
use crate::sample::{IoRates, NetStat, Sample};
use crate::theme::Theme;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Cell, Clear, Paragraph, Row, Table};
use std::time::Duration;

/// Eighth-block glyphs, used to draw the timeline one cell per sample.
const BARS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// Panel heights. Named so tests can locate a panel from the same numbers the
/// renderer lays out with, rather than re-deriving them from a comment that
/// silently goes stale.
// One divider row per section instead of two border rows per panel, and no
// side borders at all, which returned five rows and two columns to the data.
// Boxes also compete with their own contents for attention; a hairline rule
// separates without doing that. L2 then folded the cores section in here,
// trading its divider and its data row for one header line.
/// Two rows: the figures, with the live/paused marker in front of them and the
/// heat legend behind, and the per-core meters.
///
/// It was three. The extra one carried the marker, the legend and the word
/// `poptop` — and on a thirty-row terminal every chrome row is a process the
/// table cannot show.
pub const HEADER_H: u16 = 2;

/// The header's height on this machine.
///
/// [`HEADER_H`] plus a row for the NUMA nodes, on a machine that has more than
/// one. A box with a single node spends nothing here: its per-node figures are
/// the figures on the two rows above, and a permanent row restating them is a
/// row the process table does not get.
///
/// Taken from the machine and not from the sample under the cursor. Per-sample
/// it made the header grow and shrink as history was scrubbed across a
/// boundary where the field appeared — restored history from a build before
/// this one, or a moment when `/sys` could not be read — which moves the
/// timeline and the whole table by a row on every keypress.
pub fn header_height(app: &App) -> u16 {
    HEADER_H + u16::from(app.numa)
}
/// The height the timeline had when it was fixed.
///
/// It is the floor, not a minimum for legibility: growing a panel must never
/// shrink it. A proportional height alone gave an 80x24 terminal five rows
/// where nine were fixed before — a quarter of the CPU resolution, on the most
/// common terminal size, from a change justified by *more* resolution.
pub const TIMELINE_MIN_H: u16 = 9;
/// Beyond this, rows stop buying resolution and start starving the table.
pub const TIMELINE_MAX_H: u16 = 16;
/// Rows reserved for the process table when deciding the timeline's height.
///
/// A reservation, not a layout constraint: the table is laid out with whatever
/// remains, so this number and the layout cannot drift apart.
pub const PROCS_RESERVE_H: u16 = 6;
/// Process rows the table keeps however cramped the terminal.
///
/// Measured in processes, which is the unit the panel exists to show. It was
/// two *panel* rows, and a table spends two on chrome before any data — so a
/// floor of two rows was a floor of zero processes. At 120x14 a filter matching
/// eleven processes drew none of them; in tree mode at sixteen rows it drew two
/// idle daemons, because tree order is structural rather than by CPU.
pub const PROCS_FLOOR_ROWS: u16 = 3;
/// The divider and the column header, spent before a single process is drawn.
pub const PROCS_CHROME_H: u16 = 2;
/// What the panel therefore needs to honour that floor.
pub const PROCS_FLOOR_H: u16 = PROCS_FLOOR_ROWS + PROCS_CHROME_H;

/// Timeline height for a terminal of `total` rows.
///
/// Proportional above the floor, because rows are resolution here: each braille
/// row carries four levels, so the eleven-row panel a 38-row terminal gives has
/// twenty distinct CPU heights against the twelve of the old fixed nine.
///
/// Never below the height it used to have, and never so tall the process table
/// cannot be read.
pub fn timeline_height(total: u16, header: u16) -> u16 {
    // `total` is the whole frame; the bar and the tab strip have taken theirs.
    let total = total.saturating_sub(chrome_height(total));
    let spare = total.saturating_sub(header + PROCS_RESERVE_H + 1);
    let want = (spare * 2 / 5).clamp(TIMELINE_MIN_H, TIMELINE_MAX_H);
    // On a terminal too small for the floor, take what is left over — but never
    // everything, or the table disappears.
    want.min(total.saturating_sub(header + 1 + PROCS_FLOOR_H).max(1))
}

/// Rows occupied by the timeline panel.
///
/// Test-only: it exists so a test can locate the panel from the same function
/// the renderer lays out with. Accurate because the timeline is a `Length` and
/// the table takes what remains — with a `Min` on the table, ratatui would
/// outrank the timeline and this would report a panel that is not there.
#[cfg(test)]
pub fn timeline_rows_range(total_height: u16) -> std::ops::Range<u16> {
    // The menu bar sits above the header, so every panel is one row lower than
    // the constants alone would say.
    // Anchored to the bottom, where the graphs are: the help line below them,
    // and whatever the gap and the table took above.
    let h = timeline_height(total_height, HEADER_H);
    let top = total_height.saturating_sub(1 + h);
    top..top + h
}

/// Where each panel sits in the frame.
///
/// Derived once and used by both the drawing and the mouse, which is the only
/// way the two can agree about what a click landed on. A second copy of this
/// arithmetic would put a hit box a row away from the thing drawn in it, and
/// nothing would say so — the click would just do the wrong thing sometimes.
pub struct Panels {
    pub menu: Rect,
    pub tabs: Rect,
    pub header: Rect,
    pub timeline: Rect,
    pub table: Rect,
    pub help: Rect,
}

/// The screen, top to bottom: the bar, the machine, the table's own strip, the
/// table, the graphs, the keys.
///
/// The order is an argument about what each band is about, and who it belongs
/// to.
///
/// The strip is above the table because it is the table's: the tabs choose
/// which resource the *columns* describe, the settings say how the *rows* are
/// ordered and folded, and the scope says which rows are there at all. Above
/// the header it was navigation for the screen; here it is a toolbar for the
/// panel underneath it, and a reader looking at a row can find every control
/// that shaped it on the line directly above.
///
/// The graphs are at the bottom because they are the past and the table is the
/// present. Reading down the screen now goes: this machine, this table, how it
/// got here — and the timeline's caption, which says how much time is on
/// screen and which sample the cursor is on, lands beside the key hints that
/// scrub it rather than eight rows above them.
///
/// It cost no rows. The strip and the panel rule were already two lines above
/// the table; they have swapped contents rather than multiplied — the rule
/// gave up the settings it was naming and kept what only it says, which is
/// what the table cannot show.
pub fn panels(app: &App, area: Rect) -> Panels {
    // Measured rather than assumed, so the node row is a row the layout knows
    // about instead of one drawn over the timeline.
    let header = header_height(app);
    let gap = app.density.panel_gap(area.height);
    let c = Layout::vertical([
        Constraint::Length(MENU_H),
        Constraint::Length(header),
        Constraint::Length(tabs_height(area.height)),
        // Whatever remains. `timeline_height` has already reserved the table's
        // share, and a `Min` here would outrank the timeline's `Length` and
        // silently shrink it below the height that function reports.
        Constraint::Min(1),
        // A blank row between the table and the graph, where the terminal is
        // tall enough to give one up. Vertical space is the scarcest thing
        // here, which is why this is the last comfort granted and the first
        // withdrawn.
        Constraint::Length(gap),
        Constraint::Length(timeline_height(area.height.saturating_sub(gap), header)),
        Constraint::Length(1), // help
    ])
    .split(area);
    Panels {
        menu: c[0],
        header: c[1],
        tabs: c[2],
        table: c[3],
        timeline: c[5],
        help: c[6],
    }
}

pub fn draw(f: &mut Frame, app: &App) {
    let p = panels(app, f.area());

    // The ground first, under everything. Two things follow from painting it
    // rather than leaving it to the terminal: the interface reads as one
    // surface instead of as text that happens to be arranged, and every
    // contrast figure `--check-theme` reports becomes a measurement rather
    // than an assumption about somebody else's configuration.
    f.render_widget(Block::default().style(app.theme.surface_style()), f.area());
    // Panels one step up, so the bands of the screen are visible without a
    // border spending a row and a column on saying where they are.
    for panel in [p.timeline, p.table] {
        f.render_widget(Block::default().style(app.theme.panel_style()), panel);
    }
    f.render_widget(Block::default().style(app.theme.raised_style()), p.menu);

    let Some(sample) = app.history.current() else {
        f.render_widget(
            Paragraph::new("collecting first sample…").style(app.theme.dim_style()),
            f.area(),
        );
        return;
    };

    draw_menu_bar(f, p.menu, app);
    draw_tabs(f, p.tabs, app);
    draw_header(f, p.header, app, sample);
    draw_timeline(f, p.timeline, app);
    if app.show_cgroups {
        draw_cgroups(f, p.table, app);
    } else {
        draw_procs(f, p.table, app, p.timeline, p.tabs.height > 0);
    }
    draw_help(f, p.help, app);
    // Centred on the frame, not on the table: it is a modal about one row, and
    // sizing it to the table clipped the measurements off the bottom — which
    // are the point, since they are the part Activity Monitor cannot do.
    draw_inspector(f, f.area(), app);
    // The whole list of keys, which `?` and the Help menu both ask for. Over
    // the inspector, because it is the more recent request and covering it is
    // the only honest way to answer one modal asked for from another.
    if app.show_help {
        draw_key_list(f, app);
    }
    // Last, over everything: a dropdown that the table drew on top of would be
    // a menu you can open and cannot read.
    draw_dropdown(f, f.area(), app);
}

/// The rows above the header: the menu bar, and the tab strip if it fits.
///
/// A function rather than a constant, because the strip is the first row given
/// up on a short terminal — and named once, because the last time a row was
/// added at the top, twenty tests that had written `MENU_H` to mean "the offset
/// to the header" all had to be found and changed.
///
/// The two rows are no longer adjacent: the bar is above the header and the
/// strip below it, on the table it belongs to. This is still the number the
/// timeline has to subtract, which is what it is for — where they sit is a
/// question for [`panels`].
pub fn chrome_height(total: u16) -> u16 {
    MENU_H + tabs_height(total)
}

/// The row the tab strip is drawn on, for a frame this app would fill.
///
/// Derived from [`panels`] so a test cannot count it out by hand and be one
/// row wrong — which is how this file lost an afternoon the last time a band
/// moved.
#[cfg(test)]
pub fn tabs_y(app: &App, area: Rect) -> u16 {
    panels(app, area).tabs.y
}

/// Height of the tab strip, which is one row or none.
///
/// Drawn wherever there is room, because navigation nobody can see is
/// navigation nobody uses. Given up before the timeline loses a row, because a
/// graph too short to read is a worse loss than a strip whose contents the
/// panel title still names.
pub fn tabs_height(total: u16) -> u16 {
    let without = MENU_H + HEADER_H + 1 + PROCS_FLOOR_H + TIMELINE_MIN_H;
    u16::from(total > without)
}

/// Height of the tab strip when there is room for it.
#[cfg(test)]
pub const TABS_H: u16 = 1;

/// The table's own strip: which resource, how the rows are arranged, and which
/// rows they are.
///
/// One row, three jobs, left to right in the order a reader asks them. The
/// tabs say what the columns are about. The settings say what was done to the
/// rows — the ordering, the folding, the averaging — which used to be clauses
/// in the panel rule underneath and are a line closer to the table here, on
/// the row that also offers the tabs that change them. The scope says which
/// rows are in the list at all, and never disappears.
///
/// The tabs are marked with an underline rather than colour alone. Five
/// meaning-bearing hues are already spent, and a navigation strip that is
/// invisible at the mono tier fails on exactly the terminals a monitor is most
/// likely to be opened in.
fn draw_tabs(f: &mut Frame, area: Rect, app: &App) {
    let area = content(app, area);
    // The cgroup table is a different list with an ordering of its own, and it
    // is drawn in the panel this strip sits on. So no tab is marked while it
    // is up — marking one would claim these columns are what is below — and
    // the settings, which are the process table's, say nothing. What the list
    // *is* is still said, at the end of the row where that always goes.
    let cgroups = app.show_cgroups;
    let mut spans = Vec::new();
    for v in crate::app::View::ALL {
        let on = app.view == v && !cgroups;
        let style = if on {
            app.theme
                .title_style()
                .add_modifier(Modifier::BOLD)
                .add_modifier(Modifier::UNDERLINED)
        } else {
            app.theme.dim_style()
        };
        spans.push(Span::styled(format!(" {}  ", v.label()), style));
    }
    // The scope, right-aligned on the same row. "Which resource" and "which
    // processes" are the same question — what am I looking at — and putting the
    // second one here costs no row of its own.
    let used: usize = spans.iter().map(|s| s.content.chars().count()).sum();
    // While the filter is being typed it *is* the scope, so the field is here
    // rather than in a box of its own at the other end of the screen. Two
    // places saying the same thing is the objection the key hints already
    // answer to; a filter is no different.
    let tail: Vec<Span> = if app.editing_filter {
        let text = format!("filter: {}", app.filter);
        vec![
            Span::styled(text, app.theme.cursor_style()),
            Span::styled("█", app.theme.cursor_style()),
        ]
    } else if cgroups {
        // Named rather than counted: the count and the depth are in the
        // panel's own rule under this, and what this row has to say is which
        // list that is.
        vec![Span::styled("cgroups (C)", app.theme.dim_style())]
    } else {
        let scope = scope_text(app, (area.width as usize).saturating_sub(used + 2));
        vec![Span::styled(scope, app.theme.dim_style())]
    };
    let tail_w: usize = tail.iter().map(|s| s.content.chars().count()).sum();
    // The settings take what is left between the two, and only what is left:
    // the tabs are the navigation and the scope is the one line that may never
    // vanish, so this is the part of the row that gives way.
    let settings = if cgroups {
        String::new()
    } else {
        settings_text(app, (area.width as usize).saturating_sub(used + tail_w + 4))
    };
    let settings_w = settings.chars().count();
    if settings_w > 0 {
        spans.push(Span::styled(settings.clone(), app.theme.dim_style()));
    }
    let pad = (area.width as usize).saturating_sub(used + settings_w + tail_w + 1);
    spans.push(Span::raw(" ".repeat(pad)));
    spans.extend(tail);
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

/// What has been done to the rows: the ordering, the folding, the averaging.
///
/// These were clauses in the panel rule under this one, ranked among the
/// omissions and the events. They are a different kind of statement: an
/// omission is something the reader needs to be told, and a setting is
/// something they did — so it belongs beside the keys that undo it, and the
/// rule below is left to say only what the table cannot show.
///
/// Returned in pieces so the two places that say them can each give way in
/// their own units: the strip drops from the end of the list, and the rule —
/// which says them only when there is no strip — hands them to `fit_title`
/// with ranks of their own.
///
/// Empty where there is nothing to say. The sort is always something, so the
/// first is never empty; the fold and the average are off by default.
pub fn settings_parts(app: &App) -> [String; 3] {
    // What the rows stand for, when a row is not one process. Named, not just
    // "grouped": the rows say what they fold only if you already know which
    // key is in force, and `g` has four states rather than two.
    let fold = match (app.tree, app.group) {
        (true, _) => "tree".to_string(),
        (_, g) if g != crate::app::Grouping::Off => g.label().replace("grouped by ", "by "),
        _ => String::new(),
    };
    // Said, because otherwise the table and the timeline disagree in silence.
    // A row reading 12.3% under a graph showing a spike to 40 is two panels
    // contradicting each other, and nothing else says one of them is an
    // average.
    let avg = if app.smoothing().is_on() {
        format!("avg {}", fmt_smooth(app.smooth, app.interval))
    } else {
        String::new()
    };
    [format!("sort {}", app.sort.label()), fold, avg]
}

/// The settings as one clause for the strip, in whatever room is left.
///
/// A ladder, given up from the least important end, and it may reach nothing:
/// the sorted column wears a caret in its own header (0206), so the ordering
/// is named on screen whether or not this is. That is what makes it safe for
/// this to be the part of the strip that gives way.
///
/// The kernel toggle is not here. That one hides rows, and how many were
/// hidden is a fact the panel rule states as an omission — which is where a
/// reader who has forgotten the setting will find it.
pub fn settings_text(app: &App, width: usize) -> String {
    let [sort, fold, avg] = settings_parts(app);
    let join = |parts: &[&str]| -> String {
        let kept: Vec<&str> = parts.iter().copied().filter(|p| !p.is_empty()).collect();
        if kept.is_empty() {
            String::new()
        } else {
            format!("  {}", kept.join(" · "))
        }
    };
    [
        join(&[&sort, &fold, &avg]),
        join(&[&sort, &fold]),
        join(&[&sort]),
        String::new(),
    ]
    .into_iter()
    .find(|r| r.chars().count() <= width)
    .unwrap_or_default()
}

/// What is being listed, and what has narrowed it.
///
/// Present when nothing is filtered, which is what makes it trustworthy when
/// something is: the line never disappears, so its absence can never be
/// mistaken for "no filter". poptop used to state this in a clause of the
/// process panel's title — and that title is a ladder whose clauses are dropped
/// from the least important end, so on the terminals where the table is hardest
/// to read, the sentence saying *which* processes these are went first.
///
/// A table that does not say it is filtered is a table that lies about the
/// machine, and it does it silently. So this has its own ladder, and the bottom
/// rung is still a pair of numbers rather than nothing.
pub fn scope_text(app: &App, width: usize) -> String {
    let rows = app.visible_rows();
    let shown: usize = rows
        .iter()
        .filter(|r| !r.is_thread())
        .map(|r| r.count())
        .sum();
    // Every process in the sample, including the ones a filter or the kernel
    // toggle is hiding — the denominator has to be the machine, or "4 of 4"
    // would be true of a filtered list and say nothing.
    let total = app.history.current().map_or(0, |s| s.procs.len());
    let user = app.one_user();
    let filter = app.filter.trim();

    let mut rungs = Vec::new();
    if filter.is_empty() {
        if let Some(u) = user.as_deref() {
            rungs.push(format!("All processes · {total} · {u}"));
        }
        rungs.push(format!("All processes · {total}"));
        rungs.push(format!("{total} processes"));
        rungs.push(format!("{total}"));
    } else {
        if let Some(u) = user.as_deref() {
            rungs.push(format!("{filter} · {shown} of {total} · {u}"));
        }
        rungs.push(format!("{filter} · {shown} of {total}"));
        rungs.push(format!("{shown} of {total}"));
        rungs.push(format!("{shown}/{total}"));
    }
    rungs
        .into_iter()
        .find(|r| r.chars().count() <= width)
        // Never nothing. A scope line that can vanish is one whose absence
        // means "unfiltered", and that is the claim this exists to stop.
        .unwrap_or_else(|| format!("{shown}/{total}"))
}

/// Where a tab's name starts, in columns. Shared with the mouse.
pub fn tab_column(index: usize) -> usize {
    crate::app::View::ALL
        .iter()
        .take(index)
        .fold(1, |at, v| at + v.label().chars().count() + 4)
}

/// Width of a tab's clickable region.
pub fn tab_width(v: crate::app::View) -> usize {
    v.label().chars().count() + 4
}

/// Height of the menu bar. One row, always drawn — a bar that appeared only
/// when opened would be a bar nobody discovers, which is the whole reason it
/// exists.
pub const MENU_H: u16 = 1;

/// The bar: `File  Edit  View  Go  Process`, with the open one highlighted.
fn draw_menu_bar(f: &mut Frame, area: Rect, app: &App) {
    let area = content(app, area);
    let titles = crate::menu::bar();
    let mut spans = Vec::new();
    for (i, t) in titles.iter().enumerate() {
        let open = app.menu.open == Some(i);
        let style = if open {
            app.theme.selection_style()
        } else {
            app.theme.title_style()
        };
        spans.push(Span::styled(format!(" {} ", t.name), style));
    }
    // The key that opens it, stated on the bar itself. A menu bar with no way
    // in is decoration.
    //
    // At the far end, not two spaces after `Process`, where it read as a sixth
    // menu — and where the eye going down the left-hand column hits it before
    // it hits anything on the row below. The scope sits at that end of the tab
    // strip for the same reason: this column of the screen is for what the bar
    // *is*, not for what is on it. Dropped rather than crowded when the titles
    // leave no room, like every other hint here.
    let hint = if app.menu.is_open() {
        "↑↓ move · ⏎ choose · esc close"
    } else {
        "F10 menu"
    };
    let used: usize = spans.iter().map(|s| cols(&s.content)).sum();
    let room = (area.width as usize).saturating_sub(used + 1);
    if cols(hint) <= room {
        spans.push(Span::raw(" ".repeat(room - cols(hint))));
        spans.push(Span::styled(hint.to_string(), app.theme.dim_style()));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

/// The open dropdown, drawn over whatever is beneath it.
/// One process, everything poptop holds about it, over the table.
///
/// `d` replaces the timeline with the selected process's history, which is a
/// different and better thing than this and is not a substitute for it: the
/// full command is truncated in the table and available nowhere, the parent is
/// collected and shown only in the tree, and `--export` has every field and is
/// not a thing you read while looking at a row.
///
/// The peaks are the part Activity Monitor cannot do. They come from the
/// buffer, and they are the answer to "is this normal for it".
fn draw_inspector(f: &mut Frame, area: Rect, app: &App) {
    if !app.inspecting {
        return;
    }
    let Some(sample) = app.history.current() else {
        return;
    };
    let Some(watched) = app.selected.as_ref() else {
        return;
    };
    let Some(p) = sample.procs.iter().find(|p| watched.matches(p)) else {
        return;
    };

    // Over the whole buffer, not the visible window: "is this normal for it"
    // is a question about everything that was recorded, and the window is a
    // scroll position.
    let (mut peak_cpu, mut peak_rss, mut seen) = (0.0f32, 0u64, 0usize);
    for s in app.history.iter() {
        if let Some(q) = s.procs.iter().find(|q| watched.matches(q)) {
            peak_cpu = peak_cpu.max(q.cpu);
            peak_rss = peak_rss.max(q.rss);
            seen += 1;
        }
    }

    let dim = app.theme.dim_style();
    let val = app.theme.title_style();
    let pair = |k: &str, v: String| {
        Line::from(vec![
            Span::styled(format!(" {k:<9}"), dim),
            Span::styled(v, val),
        ])
    };
    let mut lines = vec![
        Line::from(Span::styled(format!(" {}", p.command()), val)),
        Line::from(""),
        pair("user", p.user.to_string()),
        pair("pid", format!("{}  parent {}", p.pid, p.ppid)),
        pair(
            "state",
            match p.state {
                'R' => "R · running".into(),
                'S' => "S · sleeping".into(),
                'D' => "D · uninterruptible".into(),
                'Z' => "Z · zombie".into(),
                'T' => "T · stopped".into(),
                c => c.to_string(),
            },
        ),
        pair(
            "threads",
            p.threads.map_or_else(|| "—".into(), |n| n.to_string()),
        ),
    ];
    if let Some(n) = p.nice {
        lines.push(pair("nice", n.to_string()));
    }
    if let Some(c) = p.container.as_deref() {
        lines.push(pair("container", c.to_string()));
    }
    lines.push(Line::from(""));
    // Both time bases are named, because they are different: the figure is the
    // moment under the cursor and the peak is everything recorded. A panel
    // showing two clocks without saying so is one whose numbers cannot be
    // compared with each other.
    lines.push(pair(
        "cpu",
        format!("{:.1}%   peak {peak_cpu:.1}% over {seen} samples", p.cpu),
    ));
    lines.push(pair(
        "memory",
        format!("{}   peak {}", fmt_bytes(p.rss), fmt_bytes(peak_rss)),
    ));
    if let Some(io) = p.io.as_ref() {
        lines.push(pair(
            "disk",
            format!(
                "{}/s read · {}/s written",
                fmt_bytes(io.read),
                fmt_bytes(io.write)
            ),
        ));
    }

    let w = lines
        .iter()
        .map(|l| l.spans.iter().map(|s| cols(&s.content)).sum::<usize>())
        .max()
        .unwrap_or(20)
        .clamp(24, area.width.saturating_sub(4) as usize);
    let h = (lines.len() + 2).min(area.height.saturating_sub(2) as usize);
    // Centred in the panel it is over, which means the panel's own origin: a
    // box positioned in frame coordinates lands on whatever is at the top of
    // the screen instead.
    let x = area.x + (area.width.saturating_sub(w as u16 + 2)) / 2;
    let y = area.y + (area.height.saturating_sub(h as u16)) / 2;
    let box_area = Rect::new(x, y, w as u16 + 2, h as u16);

    f.render_widget(Clear, box_area);
    f.render_widget(Block::default().style(app.theme.raised_style()), box_area);

    // Both ends of the box are drawn by this function, so both have to be
    // measured by it. A line wider than `w` used to be laid down whole and
    // clipped by the terminal, which ate the right border and left the command
    // running into whatever was behind the box — on a full command line, which
    // is most of them, the panel simply had no right-hand side.
    let title = elide_middle(&format!(" {} · {} ", p.name, p.pid), w);
    let bar = "─".repeat(w.saturating_sub(cols(&title)));
    let mut framed = vec![Line::from(Span::styled(
        format!("╭{title}{bar}╮"),
        app.theme.chrome_style(),
    ))];
    for l in lines.into_iter().take(h.saturating_sub(2)) {
        let mut spans = vec![Span::styled("│", app.theme.chrome_style())];
        let mut used = 0usize;
        for span in l.spans {
            let room = w - used;
            if room == 0 {
                break;
            }
            let text = elide_middle(&span.content, room);
            used += cols(&text);
            spans.push(Span::styled(text, span.style));
        }
        spans.push(Span::raw(" ".repeat(w - used)));
        spans.push(Span::styled("│", app.theme.chrome_style()));
        framed.push(Line::from(spans));
    }
    framed.push(Line::from(Span::styled(
        format!("╰{}╯", "─".repeat(w)),
        app.theme.chrome_style(),
    )));
    f.render_widget(Paragraph::new(framed), box_area);
}

/// Where the open dropdown sits, if one is open.
///
/// Shared with the mouse for the reason `panels` is: a hit box computed
/// separately from the box it is drawn in agrees until it does not.
pub fn dropdown_rect(app: &App, area: Rect) -> Option<Rect> {
    let titles = crate::menu::bar();
    let open = app.menu.open?;
    let title = titles.get(open)?;
    let w = crate::menu::width(title).min(area.width.saturating_sub(2) as usize);
    let y = MENU_H;
    if area.height <= y + 1 || w == 0 {
        return None;
    }
    let x = crate::menu::title_column(open, &titles)
        .min(area.width.saturating_sub(w as u16 + 1) as usize) as u16;
    let h = ((title.items.len() + 2) as u16).min(area.height - y);
    Some(Rect::new(x, y, w as u16, h))
}

/// The first item drawn, when the dropdown is taller than the screen.
///
/// Shared with the mouse for the reason [`dropdown_rect`] is: an offset worked
/// out twice puts the highlight on one item and the click on another.
///
/// The list used to be cut off at the bottom instead, which is worse than it
/// sounds — `move_item` still walked onto the items nobody could see, so the
/// highlight left the screen and the menu read as having stopped responding.
pub fn dropdown_offset(app: &App, area: Rect) -> usize {
    let Some(rect) = dropdown_rect(app, area) else {
        return 0;
    };
    let titles = crate::menu::bar();
    let Some(title) = app.menu.open.and_then(|i| titles.get(i)) else {
        return 0;
    };
    let shown = rect.height.saturating_sub(2) as usize;
    if shown == 0 || title.items.len() <= shown {
        return 0;
    }
    // Scrolled no further than the highlight demands, so the list sits at its
    // top until something below the fold is reached and returns there when it
    // wraps round.
    app.menu
        .item
        .saturating_sub(shown - 1)
        .min(title.items.len() - shown)
}

fn draw_dropdown(f: &mut Frame, area: Rect, app: &App) {
    let titles = crate::menu::bar();
    let Some(open) = app.menu.open else { return };
    let Some(title) = titles.get(open) else {
        return;
    };
    let Some(box_area) = dropdown_rect(app, area) else {
        return;
    };
    let (w, h) = (box_area.width as usize, box_area.height);

    // Cleared first: a dropdown is opaque, and ratatui draws over rather than
    // through. Then the raised ground, which is what makes it read as being
    // *over* the table rather than cut into it.
    f.render_widget(Clear, box_area);
    f.render_widget(Block::default().style(app.theme.raised_style()), box_area);

    let inner = w.saturating_sub(2);
    // A rule, with a mark on it when there is more list in that direction. In
    // the border rather than on a row of its own: the reason the list is being
    // scrolled is that rows are scarce.
    let rule = |left: char, right: char, more: bool| {
        let mut mid = "─".repeat(inner);
        if more && inner >= 3 {
            mid = format!(
                "{}{}─",
                "─".repeat(inner - 2),
                if left == '╭' { '↑' } else { '↓' }
            );
        }
        Line::from(Span::styled(
            format!("{left}{mid}{right}"),
            app.theme.chrome_style(),
        ))
    };
    let shown = (h as usize).saturating_sub(2);
    let offset = dropdown_offset(app, area);
    let mut lines = vec![rule('╭', '╮', offset > 0)];
    for (i, item) in title.items.iter().enumerate().skip(offset).take(shown) {
        lines.push(match item {
            crate::menu::Item::Rule => Line::from(Span::styled(
                format!("├{}┤", "─".repeat(inner)),
                app.theme.chrome_style(),
            )),
            crate::menu::Item::Do(label, key, _) => {
                let tick = match crate::menu::checked(item, app) {
                    Some(true) => "• ",
                    Some(false) => "  ",
                    None => "  ",
                };
                let key_w = key.chars().count();
                let gap =
                    inner.saturating_sub(2 + tick.chars().count() + label.chars().count() + key_w);
                let text = format!(
                    " {tick}{label}{}{key} ",
                    " ".repeat(gap.max(1).saturating_sub(1))
                );
                let style = if i == app.menu.item {
                    app.theme.selection_style()
                } else {
                    app.theme.dim_style()
                };
                Line::from(vec![
                    Span::styled("│", app.theme.chrome_style()),
                    Span::styled(cut(&text, inner), style),
                    Span::styled("│", app.theme.chrome_style()),
                ])
            }
        });
    }
    if lines.len() < h as usize {
        lines.push(rule('╰', '╯', offset + shown < title.items.len()));
    }
    f.render_widget(Paragraph::new(lines), box_area);
}

/// Pad or truncate to exactly `n` columns.
fn cut(s: &str, n: usize) -> String {
    let have = s.chars().count();
    if have >= n {
        s.chars().take(n).collect()
    } else {
        format!("{s}{}", " ".repeat(n - have))
    }
}

/// A section rule with its name on it, replacing a panel border.
///
/// The rule takes the most recessive token and the name a readable but still
/// recessive one, so neither competes with the figures beneath.
/// Join what fits, dropping whole clauses from the least important end.
///
/// The same ladder the header figures and the key hints use. A title is not
/// truncated: a clipped one reads as a message that does not exist, and the
/// pieces here are each a separate claim rather than one sentence, so losing a
/// whole claim is honest where losing the end of one is not.
///
/// Clauses are given in display order and carry the rank at which they are
/// given up, highest first, and the style they are drawn in. An empty clause
/// costs nothing and is skipped.
fn fit_title(parts: &[(u8, String, Style)], width: usize) -> Vec<Span<'static>> {
    let mut keep: Vec<bool> = parts.iter().map(|(_, s, _)| !s.is_empty()).collect();
    let len = |keep: &[bool]| -> usize {
        parts
            .iter()
            .zip(keep)
            .filter(|(_, k)| **k)
            .map(|((_, s, _), _)| cols(s))
            .sum()
    };
    while len(&keep) > width {
        // The least important thing still present.
        let Some(i) = parts
            .iter()
            .enumerate()
            .filter(|(i, _)| keep[*i])
            .max_by_key(|(_, (rank, _, _))| *rank)
            .map(|(i, _)| i)
        else {
            break;
        };
        // Never drop the subject itself; a title with no name is worse than a
        // long one.
        if parts[i].0 == 0 {
            break;
        }
        keep[i] = false;
    }
    parts
        .iter()
        .zip(&keep)
        .filter(|(_, k)| **k)
        .map(|((_, s, style), _)| Span::styled(s.clone(), *style))
        .collect()
}

fn divider(title: &str, width: u16, theme: &Theme) -> Line<'static> {
    divider_of(
        // The space the clauses carry for themselves in `divider_of`.
        vec![Span::styled(
            format!(" {}", title.trim()),
            theme.title_style(),
        )],
        width,
        theme,
    )
}

/// A section rule around a title made of separately styled clauses.
///
/// Split from [`divider`] so the process panel can draw a warning as a warning
/// while the timeline keeps its one-sentence title.
fn divider_of(parts: Vec<Span<'static>>, width: u16, theme: &Theme) -> Line<'static> {
    let lead = "─".repeat(2.min(width as usize));
    let name: usize = parts.iter().map(|s| cols(&s.content)).sum();
    let used = cols(&lead) + name + 1;
    let tail = "─".repeat((width as usize).saturating_sub(used));
    // The clauses carry their own leading space, so the rule does not add one.
    let mut out = vec![Span::styled(lead, theme.chrome_style())];
    out.extend(parts);
    out.push(Span::raw(" "));
    out.push(Span::styled(tail, theme.chrome_style()));
    Line::from(out)
}

/// A byte rate in a fixed number of columns.
///
/// The width of a figure must not depend on its value. `4.1M/s` is six columns
/// and `635.7K/s` is eight, so a network figure that switched between them
/// moved every figure to its right — measured at forty-six columns shifting a
/// second, which is most of what made this row look unstable.
///
/// Three significant figures, which is more than anybody reads off a header,
/// and right-aligned so the unit lands in the same place every time.
pub fn fmt_rate(b: u64) -> String {
    const UNITS: [&str; 5] = ["B", "K", "M", "G", "T"];
    let mut v = b as f64;
    let mut i = 0;
    while v >= 1024.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    let n = if i == 0 || v >= 100.0 {
        format!("{v:.0}")
    } else if v >= 10.0 {
        format!("{v:.1}")
    } else {
        format!("{v:.2}")
    };
    let text = format!("{n}{}/s", UNITS[i]);
    // A counter that wrapped, or a clock that jumped. No interface carries
    // sixteen exabytes a second, and the honest rendering of "this is not a
    // rate" is not to print it — but the fixed width is the whole point of this
    // function, so it says there was one rather than going blank.
    let text = if text.chars().count() > RATE_W {
        "≫1T/s".to_string()
    } else {
        text
    };
    format!("{text:>RATE_W$}")
}

/// Columns an interface name occupies. Most are three or four — `en0`, `lo0`,
/// `eth0`, `wlan0` — and a longer one is elided rather than allowed to shift
/// the row it sits in.
pub const IFACE_W: usize = 5;

/// Columns a rate occupies, whatever it is. `1023K/s` is the widest.
pub const RATE_W: usize = 7;

pub fn fmt_bytes(b: u64) -> String {
    const UNITS: [&str; 5] = ["B", "K", "M", "G", "T"];
    let mut v = b as f64;
    let mut i = 0;
    while v >= 1024.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{v:.0}{}", UNITS[i])
    } else {
        format!("{v:.1}{}", UNITS[i])
    }
}

/// Compact lag for the paused badge: seconds under a minute, then m/s.
/// Exposed for tests: the format is a claim about legibility at both ends of
/// the configurable range, which is only checkable by reading the string.
#[cfg(test)]
pub fn fmt_lag_for_test(d: Duration) -> String {
    fmt_lag(d)
}

pub fn fmt_lag(d: Duration) -> String {
    // Both ends matter now that the interval is configurable. Whole seconds
    // rendered a 100ms slot as `0s/slot` — the exact mode sub-second sampling
    // exists for — and a day-long window as `1440m00s buffered`, which is a
    // number nobody can read as a day.
    let ms = d.as_millis();
    let s = d.as_secs();
    match s {
        0 if ms > 0 => format!("{ms}ms"),
        0..60 => format!("{s}s"),
        60..3600 => format!("{}m{:02}s", s / 60, s % 60),
        _ => format!("{}h{:02}m", s / 3600, (s % 3600) / 60),
    }
}

fn fmt_uptime(d: Duration) -> String {
    let s = d.as_secs();
    let (days, hours, mins) = (s / 86400, (s % 86400) / 3600, (s % 3600) / 60);
    // Padded, so the ninth day does not move every figure beside it when it
    // becomes the tenth — and neither does the hour, or the minute.
    if days > 0 {
        format!("{days:>3}d {hours:02}h {mins:02}m")
    } else {
        format!("     {hours:02}h {mins:02}m")
    }
}

/// Fit as many header figures as the width allows, keeping the most useful.
///
/// Chosen by rank, then emitted in reading order. Letting ratatui clip instead
/// drops whatever is rightmost, and rightmost is not least useful — it dropped
/// `PROCS` and kept a memory figure that repeats the bar beside it.
///
/// The kept set is a prefix of the rank order rather than the widest subset
/// that fits: a rule you can predict from the ranking beats one that fits two
/// more characters, since the whole point is that the reader knows what
/// survives.
/// The width every figure would draw to, separators included.
///
/// What "wide enough for everything" means, and so what the heat legend has to
/// fit alongside.
/// How much air the layout is given.
///
/// Every value here is a *maximum*. A narrow terminal gives them up before it
/// gives up a column of the command line, and a short one before it gives up a
/// row of the table — comfort is the first thing surrendered, because a process
/// you cannot identify is a worse loss than a row that touches the edge.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Density {
    /// Everything packed. What poptop looked like before this was a choice.
    Compact,
    #[default]
    Comfortable,
    /// For a wide terminal with room to spare.
    Spacious,
}

impl Density {
    pub const NAMES: &'static str = "compact, comfortable or spacious";
    pub const ALL: [Density; 3] = [Density::Compact, Density::Comfortable, Density::Spacious];

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "compact" | "tight" => Some(Self::Compact),
            "comfortable" | "normal" => Some(Self::Comfortable),
            "spacious" | "loose" => Some(Self::Spacious),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Compact => "Compact",
            Self::Comfortable => "Comfortable",
            Self::Spacious => "Spacious",
        }
    }

    /// The frame's content margin, in columns, either side.
    ///
    /// *One* margin, for every row that is not a full-width divider. Measured
    /// before this existed, content began at column 0, 1, 2 or 3 depending on
    /// which row it was — the menu bar flush left, the tab strip three in, the
    /// header one, the table two, the footer none. Five margins rather than
    /// one, which is what made the layout feel ragged rather than merely tight.
    ///
    /// The panel dividers are the exception and keep spanning: they are what
    /// tells you where a panel starts, and one stopping short of the edge reads
    /// as a box missing its corners.
    pub fn margin(self, width: u16) -> u16 {
        let want = match self {
            Self::Compact => 0,
            Self::Comfortable => 1,
            Self::Spacious => 2,
        };
        // A hundred and four columns is enough to draw a deep tree of Chrome
        // helpers and not enough to spare two.
        want.min(width.saturating_sub(104) / 16)
    }

    /// Columns between two of the table's columns.
    ///
    /// Always one, and this is deliberate. A second column of air between
    /// thirteen columns is thirteen off the command line, and it has to be
    /// known by `command_width` as well as by the hit-testing — a third place
    /// for the same fact, which is the bug this interface keeps having. The
    /// columns are already told apart by their alignment; the air goes into the
    /// inset and the header instead, where it costs one column and none.
    pub fn column_gap(self) -> u16 {
        let _ = self;
        1
    }

    /// The gap between two figures about the same resource, in the header.
    pub fn header_gap(self) -> &'static str {
        match self {
            Self::Compact => "  ",
            Self::Comfortable => "   ",
            Self::Spacious => "    ",
        }
    }

    /// A blank row above the process table, separating it from the timeline.
    ///
    /// Vertical space is the scarcest thing in a terminal, so this is the last
    /// comfort granted and the first withdrawn.
    pub fn panel_gap(self, height: u16) -> u16 {
        u16::from(self == Self::Spacious && height >= 30)
    }
}

/// Between two groups. Wider, and marked, because a group boundary that looks
/// like the gap inside a group is not a boundary — and the mark carries on a
/// terminal with no colour to spend.
const FAR: &str = "  │  ";

/// Exposed for tests: the units the fitting arithmetic is done in.
#[cfg(test)]
pub fn separator_widths_for_test() -> (usize, usize, usize, usize) {
    let near = Density::default().header_gap();
    (sep_w(near), near.len(), sep_w(FAR), FAR.len())
}

/// The columns a separator occupies.
///
/// Not `len()`. `│` is one column and three bytes, so the byte length charged
/// seven for a five-column separator — and `full_width` had the correct five
/// written out by hand, so the two disagreed and the header dropped figures
/// that fitted while leaving nineteen columns of slack.
/// How many terminal columns a string occupies.
///
/// Not `chars().count()`, which counts scalar values. A CJK name or an emoji is
/// two columns per character, so a name elided "to nineteen columns" could draw
/// thirty-eight and be clipped by the terminal anyway — defeating the point of
/// eliding deliberately, which is that the part identifying the process
/// survives.
///
/// This is the same mistake as measuring in *bytes*, which this file has made
/// twice: `│` is three bytes and one column, and `⚠` is one char and two. Both
/// were found by rendering rather than by reading, which is why every width in
/// this file now goes through one function.
pub fn cols(s: &str) -> usize {
    use unicode_width::UnicodeWidthStr;
    // `width_cjk` would be the other choice: it resolves the East-Asian
    // *ambiguous* class as wide. This file already draws `·`, `≤` and `─`,
    // which are all ambiguous, and it lays them out as one column — so
    // resolving them as two would make every existing measurement wrong to fix
    // a case that does not arise.
    s.width()
}

/// How many terminal columns one character occupies.
///
/// Zero for a combining mark, which is the case that makes a per-character loop
/// necessary at all: a name built from `e` plus a combining acute is two
/// scalars and one column.
fn col_width(c: char) -> usize {
    use unicode_width::UnicodeWidthChar;
    c.width().unwrap_or(0)
}

fn sep_w(sep: &str) -> usize {
    cols(sep)
}

fn full_width(figures: &[Figure<'_>], near: &str) -> usize {
    let mut order: Vec<&Figure<'_>> = figures.iter().collect();
    order.sort_by_key(|f| f.group);
    let mut w = 0;
    let mut last: Option<Group> = None;
    for f in order {
        w += match last {
            None => 0,
            Some(g) if g == f.group => sep_w(near),
            Some(_) => sep_w(FAR),
        } + f.spans.iter().map(|s| cols(&s.content)).sum::<usize>();
        last = Some(f.group);
    }
    w
}

fn fit<'a>(figures: Vec<Figure<'a>>, width: usize, theme: &Theme, near: &str) -> Vec<Span<'a>> {
    let widths: Vec<usize> = figures
        .iter()
        .map(|f| f.spans.iter().map(|s| cols(&s.content)).sum())
        .collect();

    let groups: Vec<Group> = figures.iter().map(|f| f.group).collect();

    // The width a set of figures actually draws to, separators included.
    //
    // Measured rather than estimated. Charging the wider separator for every
    // gap — which is what this did first — overstated the line by three columns
    // per group boundary, and the figure that fell off the end was `LOAD` on a
    // 180-column terminal with room to spare.
    let drawn = |keep: &[bool]| {
        let mut shown: Vec<usize> = (0..keep.len()).filter(|&i| keep[i]).collect();
        shown.sort_by_key(|&i| groups[i]);
        let mut w = 0;
        let mut last: Option<Group> = None;
        for &i in &shown {
            w += match last {
                None => 0,
                Some(g) if g == groups[i] => sep_w(near),
                Some(_) => sep_w(FAR),
            } + widths[i];
            last = Some(groups[i]);
        }
        w
    };

    // What to keep: by rank, least diagnostic first out.
    //
    // *Not* by the tab. Biasing this toward the tab's own group was tried and
    // is wrong: `Group` says where a figure sits, not how much it explains, and
    // `Compute` holds both the two figures that answer "why is this slow" and
    // the load average that conflates them. Promoting the group promoted the
    // one figure the ladder had deliberately demoted.
    //
    // The header is about the machine, and "why is this machine slow" has the
    // same answer whichever table you are reading. The tab governs the columns.
    let mut order: Vec<usize> = (0..figures.len()).collect();
    order.sort_by_key(|&i| figures[i].rank);
    let mut keep = vec![false; figures.len()];
    for &i in &order {
        keep[i] = true;
        if drawn(&keep) > width {
            keep[i] = false;
            break;
        }
    }

    // Where they sit: by group, and by declaration order within it — which is
    // why this sort has to be stable. `MEM`, its byte detail and `SWP` are
    // ranked 50, 90 and 60, and belong on screen in that written order.
    let mut shown: Vec<usize> = (0..figures.len()).filter(|&i| keep[i]).collect();
    shown.sort_by_key(|&i| figures[i].group);

    let mut spans: Vec<Vec<Span<'a>>> = figures.into_iter().map(|f| f.spans).collect();
    let mut out = Vec::new();
    let mut last: Option<Group> = None;
    for &i in &shown {
        match last {
            None => {}
            Some(g) if g == groups[i] => out.push(Span::raw(near.to_string())),
            Some(_) => out.push(Span::styled(FAR, theme.chrome_style())),
        }
        last = Some(groups[i]);
        out.append(&mut spans[i]);
    }
    out
}

/// One header figure, and how readily it is given up.
///
/// The header is a fixed line and the figures do not fit on every terminal, so
/// something has to go first. Ranking them is the only way to make that a
/// decision rather than whatever ratatui's clipping happens to reach.
/// Which resource a figure is about, and so where it sits.
///
/// The reading order of the question this tool is opened to answer: is the
/// machine busy, is it out of memory, is it waiting on a disk, is the network
/// unhealthy — and then the two figures that are neither a symptom nor a cause.
///
/// Separate from [`Figure::rank`] because *where a figure sits* and *when it is
/// given up* are different questions with different answers. `LOAD` is the least
/// diagnostic figure here and should go early; if it is shown at all it belongs
/// beside CPU, not after uptime. With one number doing both jobs the header read
/// compute, storage, compute, network, memory, network, memory, machine —
/// network split in half with memory in between.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Group {
    Compute,
    Memory,
    Storage,
    Network,
    Machine,
}

struct Figure<'a> {
    spans: Vec<Span<'a>>,
    group: Group,
    /// Lower is kept longer.
    ///
    /// Spaced by tens rather than numbered consecutively, so a figure can be
    /// slotted between two existing ones without renumbering the ladder below
    /// it. Three insertions in a row each rewrote every rank underneath, and
    /// each rewrite was a chance to introduce a duplicate that nothing would
    /// have caught but a careful reading.
    rank: u8,
}

/// Where a steal percentage sits on the utilisation scale — same argument as
/// [`stall_heat`], same reason it cannot use the raw figure.
///
/// A CPU at 30% is unremarkable; a guest losing 30% of its time to the
/// hypervisor is the condition this figure exists to expose, and
/// `figure_style` would draw it in the calm colour until it reached half.
fn steal_heat(pct: f32, theme: &Theme) -> f32 {
    /// A twentieth of the machine going somewhere else. Noticeable, and worth
    /// knowing before it is worth panicking about.
    const WARN: f32 = 5.0;
    /// A fifth. At this point the instance is doing meaningfully less work than
    /// its size claims, and no other figure on the header will say so.
    const CRITICAL: f32 = 20.0;
    if pct >= CRITICAL {
        theme.critical_pct
    } else if pct >= WARN {
        theme.warn_pct
    } else {
        0.0
    }
}

/// Where a stall percentage sits on the scale the user configured for
/// *utilisation* percentages.
///
/// The two are not the same quantity and cannot share thresholds. A CPU at 50%
/// is unremarkable; a machine that spent 50% of the last ten seconds with
/// nothing at all running is in serious trouble. Feeding the raw figure to
/// `figure_style` would leave it cold until it was catastrophic, and scaling it
/// by a constant — which is what this did first — silently reinterprets whatever
/// the user set: at `--warn 90` a quadrupled figure needs 22.5% before it warns,
/// which is two and a quarter seconds in every ten with the machine stopped.
///
/// So the thresholds are stated here, in the units of the thing being measured,
/// and mapped onto the theme's own scale so a user's colours still apply.
fn stall_heat(pct: f32, theme: &Theme) -> f32 {
    /// Half a second in every ten with nothing running.
    const WARN: f32 = 5.0;
    /// Two seconds in every ten.
    const CRITICAL: f32 = 20.0;
    // Returned as the theme's own boundaries rather than as fixed numbers, so a
    // user who recoloured warn and critical still gets their colours here.
    if pct >= CRITICAL {
        theme.critical_pct
    } else if pct >= WARN {
        theme.warn_pct
    } else {
        0.0
    }
}

/// The share of memory awaiting writeback worth mentioning at all.
///
/// One constant, used by both the decision to show the figure and the decision
/// to colour it. Two would agree today and drift the moment either moved:
/// raising only the colour threshold would leave the figure on screen in the
/// calm style for every share between the two.
const DIRTY_WARN: f32 = 5.0;

/// Dirty pages as a share of the machine's memory.
fn dirty_share(dirty: u64, total: u64) -> f32 {
    if total == 0 {
        return 0.0;
    }
    (dirty as f64 / total as f64 * 100.0) as f32
}

/// Where a dirty share sits on the utilisation scale.
///
/// The same argument as [`stall_heat`] and [`steal_heat`]: this is not a
/// utilisation, so it cannot borrow utilisation's thresholds. A tenth of memory
/// awaiting writeback is a machine that will stall shortly; the default warn of
/// 50% would never fire before it already had.
fn dirty_heat(share: f32, theme: &Theme) -> f32 {
    const CRITICAL: f32 = 10.0;
    if share >= CRITICAL {
        theme.critical_pct
    } else if share >= DIRTY_WARN {
        theme.warn_pct
    } else {
        0.0
    }
}

/// How loudly to colour a share of calls that had to be sent again.
///
/// Its own thresholds mapped onto the theme's, like [`stall_heat`] and
/// [`steal_heat`]: a percent of retransmissions is a mount worth looking at and
/// five percent is one that is failing, where five percent of a *CPU* is
/// nothing at all. Reading the ramp straight would paint a dying mount calm.
fn retrans_heat(share: f32, theme: &Theme) -> f32 {
    const WARN: f32 = 1.0;
    const CRIT: f32 = 5.0;
    if share >= CRIT {
        theme.critical_pct + (share - CRIT).min(10.0)
    } else if share >= WARN {
        theme.warn_pct + (share - WARN) / (CRIT - WARN) * (theme.critical_pct - theme.warn_pct)
    } else {
        share / WARN * theme.warn_pct
    }
}

#[cfg(test)]
pub fn retrans_heat_for_test(share: f32, theme: &Theme) -> f32 {
    retrans_heat(share, theme)
}

/// A count as a figure narrow enough for the header — `1.2k`, `48`.
///
/// Calls a second run to five and six digits on a busy server, and a header
/// figure that grows by three columns under load is one that pushes another
/// figure off the row exactly when the machine is interesting.
pub fn fmt_count(n: u64) -> String {
    match n {
        0..1_000 => n.to_string(),
        1_000..1_000_000 => format!("{:.1}k", n as f32 / 1_000.0),
        _ => format!("{:.1}M", n as f32 / 1_000_000.0),
    }
}

/// A mount point short enough to sit in a header figure.
///
/// Kept from the right, because that is the end that identifies it: the last
/// component of `/var/snap/lxd/common/lxd/storage-pools/default` says more than
/// the first. An elision mark says the middle is missing rather than letting it
/// read as a path that exists.
#[cfg(test)]
pub fn short_mount_for_test(mount: &str) -> String {
    short_mount(mount)
}

fn short_mount(mount: &str) -> String {
    const MAX: usize = 16;
    if cols(mount) <= MAX {
        return mount.to_string();
    }
    // Columns on both sides. The measure was converted to columns and the cut
    // was left as a character index, which for a wide mount overshoots by the
    // difference: `/データベース/ストレージプール` is thirty columns and sixteen
    // characters, and skipping thirty of them left `…ル` — three columns of the
    // sixteen allowed, and none of the last path component, which is the end
    // that identifies it.
    format!("…{}", take_cols(mount, MAX - 1, true))
}

/// How close to nominal counts as not worth mentioning.
///
/// Drivers report ceilings a fraction under the hardware maximum as a matter of
/// course — rounding in the frequency table, a boost bin excluded from the
/// policy — and a permanent `CLK 99.7%` on a machine that is not throttled at
/// all would be the figure that taught everyone to ignore it.
pub const CLOCK_NOMINAL: f32 = 99.0;

/// A per-second count, shortened once it stops being readable in full.
///
/// A busy box switches a hundred thousand times a second, and `103847/s` is six
/// characters of precision nobody uses on a row that is already fighting for
/// width.
#[cfg(test)]
pub fn rate_per_s_for_test(n: u64) -> String {
    rate_per_s(n)
}

fn rate_per_s(n: u64) -> String {
    match n {
        0..=9_999 => format!("{n}/s"),
        10_000..=999_999 => format!("{:.0}k/s", n as f64 / 1_000.0),
        1_000_000..=999_999_999 => format!("{:.1}M/s", n as f64 / 1_000_000.0),
        // A machine cannot switch a billion times a second. A figure this large
        // is a counter that wrapped or a clock that jumped, and the honest
        // rendering of "this number is not a rate" is not to print it — but it
        // is still a fact about the machine, so the row says there was one
        // rather than going blank.
        _ => "≫1G/s".to_string(),
    }
}

fn draw_header(f: &mut Frame, area: Rect, app: &App, s: &Sample) {
    let area = content(app, area);
    let mem_pct = s.mem.used_pct();
    let dim = app.theme.dim_style();
    let cores = s.cpu_per_core.len().max(1);

    // Ordered by how much each answers "why is this machine slow", which is
    // the question a monitor is opened to answer. Utilization first because it
    // is what people look for; saturation immediately after because it is what
    // actually tells them something is wrong.
    let mut figures = vec![Figure {
        group: Group::Compute,
        rank: 0,
        spans: vec![
            Span::styled("CPU ", dim),
            Span::styled(
                format!("{:>5.1}%", s.cpu_total),
                app.theme.figure_style(s.cpu_total),
            ),
        ],
    }];

    // Only when there is something to say. A machine allowed its full clock
    // spends no header space announcing it — the same rule the filesystem
    // figure follows — and a figure that is present on every frame is one
    // nobody reads by the second day.
    //
    // Ranked immediately after CPU because it is the figure that qualifies it:
    // `CPU 100%` and `CLK 62%` together say the processor is flat out and
    // getting two thirds of the work done, which is a different machine from
    // `CPU 100%` alone, and nothing else on this header can tell them apart.
    if let Some(clock) = s.clock_ceiling.filter(|c| *c < CLOCK_NOMINAL) {
        figures.push(Figure {
            group: Group::Compute,
            rank: 5,
            spans: vec![
                Span::styled("CLK ", dim),
                // Heated on how much is *missing*, so a mild cap is quiet and a
                // halved clock is loud. Passing the ceiling itself would colour
                // a throttled machine as though it were idle.
                Span::styled(
                    format!("{clock:>5.1}%"),
                    app.theme.figure_style(100.0 - clock),
                ),
            ],
        });
    }

    // Only when the hypervisor is actually taking time. On bare metal it is
    // zero forever and a permanent `STL 0.0%` is a figure nobody reads by the
    // second day — the same rule as the clock ceiling above.
    //
    // Ranked with it, and for the same reason: this is the other figure that
    // *qualifies* CPU rather than adding to it. `CPU 40%` with `STL 55%` is a
    // machine working as hard as it is being allowed to, and nothing else on
    // this header can say so. A tenth of a percent is scheduling noise on any
    // shared host; a whole percent is somebody else's workload.
    if let Some(steal) = s.steal.filter(|v| *v >= 1.0) {
        figures.push(Figure {
            group: Group::Compute,
            rank: 6,
            spans: vec![
                Span::styled("STL ", dim),
                Span::styled(
                    format!("{steal:>5.1}%"),
                    app.theme.figure_style(steal_heat(steal, &app.theme)),
                ),
            ],
        });
    }

    // The figure that separates "nothing to do" from "cannot get on with
    // anything". Absent on a platform that will not say, rather than zero.
    if let Some(iowait) = s.iowait {
        figures.push(Figure {
            group: Group::Compute,
            rank: 10,
            spans: vec![
                Span::styled("WAIT ", dim),
                Span::styled(format!("{iowait:>5.1}%"), app.theme.figure_style(iowait)),
            ],
        });
    }

    // The device the WAIT figure is about. `WAIT 26.7%` says the CPU is idle
    // waiting on storage and then strands you; this names the device and says
    // how close it is to having no idle time left.
    //
    // Ranked immediately after `WAIT` for that reason — it is the answer to the
    // question the figure beside it raises, so the two should survive or go
    // together on a narrowing panel.
    if let Some(d) = s.busiest_disk() {
        let mut spans = vec![
            Span::styled(format!("{} ", d.name), dim),
            Span::styled(format!("{:>5.1}%", d.util), app.theme.figure_style(d.util)),
        ];
        // Service time only when something completed. A mean of no operations
        // is not zero, and zero would read as an infinitely fast disk.
        if let Some(a) = d.await_ms {
            spans.push(Span::styled(format!(" {a:.1}ms"), dim));
        }
        figures.push(Figure {
            group: Group::Storage,
            rank: 20,
            spans,
        });
    }

    // The mount, when the storage is not a disk on this machine.
    //
    // Ranked immediately after the disk figure and above the filesystem one,
    // because on a box whose working set lives on NFS the disk figure above is
    // describing a local disk that is doing nothing while the machine waits on
    // the network — and this is the only figure that would say so.
    //
    // Retransmissions rather than throughput. An NFS mount in trouble is
    // usually one whose calls are being sent twice, and its byte rates look
    // ordinary throughout; `NET` already carries the interface.
    if let Some(nfs) = s.nfs.as_ref().filter(|n| n.in_use()) {
        if let Some(m) = nfs.busiest().filter(|m| m.ops > 0 || m.retrans > 0) {
            // A share of nothing is not a percentage. A mount whose server has
            // stopped answering has calls going out and none coming back, so
            // `ops` is zero while `retrans` climbs — and dividing by a floor of
            // one printed `300.0% re` for the one failure this figure exists to
            // show. There the count is the fact, and it is as loud as the ramp
            // goes.
            let lost = (m.ops > 0).then(|| m.retrans as f32 / m.ops as f32 * 100.0);
            let heat = app
                .theme
                .figure_style(lost.map_or(f32::MAX, |l| retrans_heat(l, &app.theme)));
            let mut spans = vec![
                Span::styled(format!("{} ", short_mount(&m.mount)), dim),
                Span::styled(format!("{} op/s", fmt_count(m.ops)), heat),
            ];
            // Only when something is going wrong. A healthy mount retransmits
            // nothing for weeks, and `0.0% re` on every frame is a figure
            // nobody reads by the second day — the rule `CLK` and `STL` follow.
            if m.retrans > 0 {
                spans.push(Span::styled(
                    match lost {
                        Some(l) => format!(" {l:.1}% re"),
                        None => format!(" {} re/s", fmt_count(m.retrans)),
                    },
                    heat,
                ));
            }
            figures.push(Figure {
                group: Group::Storage,
                rank: 22,
                spans,
            });
        }
        // The server, which is a different machine's problem arriving here.
        // Only where one is running: `nfsd` publishes zeroes on any kernel
        // with the module loaded.
        //
        // Ranked well below the mount above, and below memory: what this box
        // is *serving* explains somebody else's slowness, not its own. At rank
        // 45 a ninety-column header dropped `MEM` to keep it.
        if let Some(calls) = nfs.server_calls {
            figures.push(Figure {
                group: Group::Storage,
                rank: 65,
                spans: vec![
                    Span::styled("NFSD ", dim),
                    Span::styled(format!("{} op/s", fmt_count(calls)), dim),
                ],
            });
        }
    }

    // What stopped, rather than what was busy. Ranked beside the storage
    // figures because it usually explains them, and above `RUN` because a
    // machine where every task is stalled is in a worse state than one with a
    // deep run queue and work getting done.
    //
    // `full`, not `some`: some task being stalled is what a busy machine does
    // all day. Every runnable task being stalled has no benign reading, which
    // is why it is heated from zero rather than against a threshold.
    if let Some(p) = s.pressure {
        let (what, pct) = p.worst();
        figures.push(Figure {
            group: Group::Compute,
            rank: 25,
            spans: vec![
                Span::styled("STALL ", dim),
                Span::styled(format!("{what} "), dim),
                Span::styled(
                    format!("{pct:>4.1}%"),
                    app.theme.figure_style(stall_heat(pct, &app.theme)),
                ),
            ],
        });
    }

    // Runnable against cores, because a bare count means nothing without its
    // denominator: four is catastrophic on one core and idle on ninety-six.
    if let Some(running) = s.running {
        let pressure = (running as f32 / cores as f32) * 100.0;
        figures.push(Figure {
            group: Group::Compute,
            rank: 30,
            spans: vec![
                Span::styled("RUN ", dim),
                Span::styled(
                    format!("{running}/{cores}"),
                    app.theme.figure_style(pressure),
                ),
            ],
        });
    }

    // A filesystem close to full, and only then. The one figure here that
    // describes a hard failure rather than a slowdown — a machine out of disk
    // space does not get slower, it stops — so it is kept nearly to the end
    // once it appears, and says nothing at all until it does.
    //
    // Against the user's own warn threshold, because "close to full" is exactly
    // the judgement that setting encodes, and unlike a stall percentage a
    // used-space percentage is the same kind of quantity they set it for.
    if let Some(f) = s.fullest().filter(|f| f.used_pct() >= app.theme.warn_pct) {
        let pct = f.used_pct();
        figures.push(Figure {
            group: Group::Storage,
            rank: 15,
            spans: vec![
                // Capped, because a mount point has no length limit and the
                // header budgets a fixed width for every other figure. A
                // container host can mount something at
                // `/var/lib/.../storage-pools/default`.
                Span::styled(format!("{} ", short_mount(&f.mount)), dim),
                Span::styled(format!("{pct:>4.1}% full"), app.theme.figure_style(pct)),
            ],
        });
    }

    // Something went wrong on the network, and only then. A figure reading
    // `NET 0 drops` every second would spend header space to say nothing, and
    // the space is the scarcest thing here.
    //
    // Treated like `BLOCKED`: any of these above zero is worth the critical
    // style, because none of them has a healthy amount. A retransmit is not a
    // slow packet, it is a packet that did not arrive.
    if let Some((what, n)) = s.net.as_ref().and_then(NetStat::trouble) {
        figures.push(Figure {
            group: Group::Network,
            rank: 45,
            spans: vec![
                Span::styled("NET ", dim),
                Span::styled(
                    format!("{n} {what}"),
                    app.theme.figure_style(app.theme.critical_pct),
                ),
            ],
        });
    }

    // Uninterruptible sleep. Thirty processes on one hung mount give a load
    // average of thirty on a completely idle box, and this is the only figure
    // that says so — so it is heated on any value at all, not on a threshold.
    if let Some(blocked) = s.blocked {
        figures.push(Figure {
            group: Group::Compute,
            rank: 40,
            spans: vec![
                Span::styled("BLOCKED ", dim),
                Span::styled(
                    blocked.to_string(),
                    // Any blocked task at all is worth the critical
                    // treatment: there is no healthy amount of "stuck in the
                    // kernel". Styled through the theme's own threshold so it
                    // stays critical whatever the user configured.
                    if blocked > 0 {
                        app.theme.figure_style(app.theme.critical_pct)
                    } else {
                        dim
                    },
                ),
            ],
        });
    }

    // A percentage says how much; a composition says how much trouble you are
    // in. "37% used" reads identically on a box with eight gigabytes free and
    // on one whose only headroom is page cache it is about to have to drop —
    // and the second is the one worth knowing about.
    // Wide enough that the minimum-visible rule cannot distort it much: at
    // eight columns a sliver rounded up to a whole column moved the bar by
    // twelve percentage points, beside a figure stating the real one.
    const MEM_BAR_W: usize = 12;
    let (parts, has_cache) = s.mem.composition();
    let mut mem_spans = vec![
        Span::styled("MEM ", dim),
        Span::styled(format!("{mem_pct:>5.1}%"), app.theme.figure_style(mem_pct)),
    ];
    let widths = glyphs::composition(parts, MEM_BAR_W);
    if widths.iter().any(|&n| n > 0) {
        mem_spans.push(Span::raw(" "));
        for ((glyph, style), n) in [
            (glyphs::SEG_USED, app.theme.figure_style(mem_pct)),
            (glyphs::SEG_CACHE, dim),
            (glyphs::SEG_FREE, app.theme.chrome_style()),
        ]
        .into_iter()
        .zip(widths)
        {
            mem_spans.push(Span::styled(glyph.to_string().repeat(n), style));
        }
    }
    // Nothing says the middle segment is cache except its presence, so a
    // platform that cannot separate cache from free simply has none.
    debug_assert!(has_cache || widths[1] == 0);
    // Throughput, which is what everyone looks for and what least often
    // explains a slow machine — so it sits below memory and is given up before
    // it. The interface is named because a laptop has twenty-odd and only one
    // of them is carrying anything.
    //
    // Which interface is chosen over a window, not per sample, and never
    // loopback — see `App::headline_link`. The two directions are labelled:
    // `46.4K/s 2.1M/s` could be either way round.
    if let Some(name) = app.headline_link() {
        let link = s.net.iter().flat_map(|n| &n.links).find(|l| l.name == name);
        let (rx, tx) = link.map_or((0, 0), |l| (l.rx, l.tx));
        let (down, up) = if app.glyphs == crate::glyphs::GlyphSet::Ascii {
            ("rx ", "tx ")
        } else {
            ("↓", "↑")
        };
        figures.push(Figure {
            group: Group::Network,
            rank: 55,
            spans: vec![
                // The name in a fixed cell too: a laptop's busiest interface
                // flips between `lo0` and `en0` from second to second, and the
                // figure cannot change width when it does.
                Span::styled(format!("{:<IFACE_W$} ", elide_middle(&name, IFACE_W)), dim),
                Span::styled(format!("{down}{}", fmt_rate(rx)), dim),
                Span::styled(" ", dim),
                Span::styled(format!("{up}{}", fmt_rate(tx)), dim),
            ],
        });
    }

    figures.push(Figure {
        group: Group::Memory,
        rank: 50,
        spans: mem_spans,
    });

    // Only when there is enough of it to matter. A box with a fifth of its
    // memory dirty is about to stall on writeback and every other figure on
    // this header looks fine until it does — but a few megabytes is what an
    // ordinary machine carries all the time, and a figure that is always there
    // is one nobody reads.
    //
    // Ranked *below* the `MEM` figure it qualifies, and pushed after it, for a
    // reason the first version got backwards: rank is the drop order, so at
    // rank 25 a narrow header kept `DIRTY` and dropped `MEM` — a component of
    // memory stated while the memory figure itself was gone. `CLK` and `STL`
    // sit just below `CPU` for the same reason.
    if let Some(dirty) = s
        .mem
        .dirty
        .filter(|d| dirty_share(*d, s.mem.total) >= DIRTY_WARN)
    {
        let share = dirty_share(dirty, s.mem.total);
        figures.push(Figure {
            group: Group::Memory,
            rank: 55,
            spans: vec![
                Span::styled("DIRTY ", dim),
                Span::styled(
                    fmt_bytes(dirty),
                    app.theme.figure_style(dirty_heat(share, &app.theme)),
                ),
            ],
        });
    }
    // Ranked below uptime and the process count despite being about memory,
    // which is more diagnostic than either. It is twenty-nine columns wide, and
    // under a prefix rule one wide figure blocks every shorter one behind it:
    // at a hundred columns it fit nothing and cost two figures that would have.
    figures.push(Figure {
        group: Group::Memory,
        rank: 90,
        // Shorter than it was: the bar shows what is available, so saying it
        // again in words was the third statement of one fact on one line.
        spans: vec![Span::styled(
            format!("{} / {}", fmt_bytes(s.mem.used), fmt_bytes(s.mem.total)),
            dim,
        )],
    });

    if s.mem.swap_total > 0 {
        figures.push(Figure {
            group: Group::Memory,
            rank: 60,
            spans: vec![
                Span::styled("SWP ", dim),
                Span::styled(
                    format!("{:>5.1}%", s.mem.swap_pct()),
                    app.theme.heat_style(s.mem.swap_pct()),
                ),
            ],
        });
    }

    figures.push(Figure {
        group: Group::Machine,
        rank: 70,
        spans: vec![Span::styled("UP ", dim), Span::raw(fmt_uptime(s.uptime))],
    });
    figures.push(Figure {
        group: Group::Machine,
        rank: 80,
        spans: vec![
            Span::styled("PROCS ", dim),
            // Right-aligned in four: a box crossing a thousand processes must
            // not move the figures beside it.
            Span::raw(format!("{:>4}", s.procs.len())),
        ],
    });
    // Only where the platform counts them, which is Linux: macOS has no
    // `/proc/stat`, and a zero there would be a fabricated figure about the one
    // thing this row exists to notice.
    //
    // This is the honest end of "energy". Activity Monitor scores it, from a
    // formula that is not public, using a per-process wakeup count that neither
    // platform gives up cheaply — `CONFIG_SCHEDSTATS` is off by default on
    // Linux and `task_power_info` needs root on macOS. What *is* measured is
    // the machine's switch and interrupt rate, and a machine thrashing between
    // threads looks identical to a busy one without it. See cairn 126.
    if let Some(csw) = s.ctxt {
        figures.push(Figure {
            group: Group::Compute,
            rank: 95,
            spans: vec![
                Span::styled("CSW ", dim),
                Span::raw(rate_per_s(csw)),
                Span::styled("  IRQ ", dim),
                Span::raw(s.intr.map_or_else(|| "—".to_string(), rate_per_s)),
            ],
        });
    }
    // Last to survive. Load conflates runnable and blocked into one number,
    // which is exactly the confusion `RUN` and `BLOCKED` exist to undo — and
    // the smoothing it adds is what the timeline is for. Kept for the people
    // who look for it, first to go when the line is tight.
    figures.push(Figure {
        group: Group::Compute,
        rank: 100,
        spans: vec![
            Span::styled("LOAD ", dim),
            Span::raw(format!(
                "{:.2} {:.2} {:.2}",
                s.load[0], s.load[1], s.load[2]
            )),
        ],
    });

    // The state marker sits with the figures rather than on a line of its own.
    // It qualifies them — `PAUSED -12s` means *these numbers are twelve seconds
    // old* — so it belongs in front of them, and the row it used to occupy was
    // shared with the word `poptop`, the only thing on screen that never said
    // anything.
    //
    // Never dropped, and so never passed to `fit`. Reading a stale process
    // table as the current one is the single worst thing this tool could let
    // you do, which makes this the one figure that cannot be given up for room.
    let state = if app.history.is_live() {
        Span::styled(" LIVE ", app.theme.live_style())
    } else {
        Span::styled(
            format!(" PAUSED  -{} ", fmt_lag(app.history.time_behind())),
            app.theme.paused_style(),
        )
    };
    let state_w = cols(&state.content);

    let width = area.width as usize;

    // Room for the legend is judged against *every* figure, not against the
    // ones that happen to fit. Judged against the leftovers it was not
    // monotonic: narrowing the terminal dropped a figure, freed twenty columns,
    // and the scale reappeared on a smaller screen than the one it had just
    // vanished from. Against the full set the condition depends on width alone,
    // and the scale is strictly the first thing given up — which is what the
    // ladder always said it was.
    let scale = heat_scale(area.width, &app.theme).filter(|s| {
        state_w + full_width(&figures, app.density.header_gap()) + 2 + cols(s) <= width
    });
    let reserved = scale.as_ref().map_or(0, |s| cols(s) + 2);

    let mut line = vec![state];
    let spans = fit(
        figures,
        width.saturating_sub(state_w + reserved),
        &app.theme,
        app.density.header_gap(),
    );

    line.extend(spans);
    if let Some(scale) = scale {
        // A plain gap, not the rule that divides one resource from another:
        // this is a reference for the row, not another thing on it.
        line.push(Span::raw("  "));
        line.push(Span::styled(scale, app.theme.dim_style()));
    }

    let mut rows = vec![Line::from(line), core_meters(s, area.width, &app.theme)];
    if app.numa {
        // The row is the machine's, so it is drawn for every sample once the
        // machine has one — and a sample collected before poptop read nodes
        // says so rather than leaving the reserved row blank.
        rows.push(node_meters(s, area.width, &app.theme).unwrap_or_else(|| {
            Line::from(Span::styled(
                "  nodes not recorded in this sample",
                app.theme.dim_style(),
            ))
        }));
    }
    f.render_widget(Paragraph::new(rows), area);
}

/// The per-node line, or nothing at all.
///
/// `None` on a machine with one node and on a platform that will not say —
/// which is the whole rule this row follows. The figures a two-socket box
/// needs and a one-socket box does not are per-node: a node with no memory
/// left beside a node that is idle is the failure the whole-machine `MEM`
/// figure averages away, and there is nothing else on this header that can
/// tell those two machines apart.
fn node_meters(s: &Sample, width: u16, theme: &Theme) -> Option<Line<'static>> {
    let nodes = s.nodes.as_deref()?;
    let n = nodes.len();
    let w = width as usize;
    let label = format!("{n:>3} nodes ");
    let bare = || Line::from(Span::styled(format!("{n} nodes"), theme.dim_style()));

    // Too narrow even for the label: state the count. The same ladder the core
    // meters follow, and for the same reason — a row of node figures that
    // cannot say how many are missing is worse than the count alone.
    if cols(&label) >= w {
        return Some(bare());
    }
    let avail = w - cols(&label);

    let cell = |node: &crate::sample::NodeStat| {
        let cpu = match node.cpu {
            Some(v) => Span::styled(format!("{v:>5.1}%"), theme.figure_style(v)),
            // A node whose CPU list names cores `/proc/stat` did not is not a
            // node that is idle.
            None => Span::styled("    —".to_string(), theme.dim_style()),
        };
        // Free is printed because on a NUMA box that is the number deciding
        // whether the next allocation stays local. The *colour* is what is not
        // coming back: page cache on a node is reclaimable, and heating on
        // free alone would paint a healthy box critical for holding cache —
        // the exact misreading `MemStat::free` warns about, one row down.
        let gone = node
            .total
            .saturating_sub(node.free)
            .saturating_sub(node.file.unwrap_or(0));
        let used = gone as f32 / node.total.max(1) as f32 * 100.0;
        vec![
            Span::styled(format!("n{} ", node.id), theme.dim_style()),
            cpu,
            Span::styled(" ", theme.dim_style()),
            Span::styled(fmt_bytes(node.free), theme.heat_style(used)),
            Span::styled(" free", theme.dim_style()),
        ]
    };

    // Build every cell, then take the ones that fit — cell widths differ, so
    // reserving a per-node guess would either waste columns or overrun.
    let cells: Vec<Vec<Span<'static>>> = nodes.iter().map(cell).collect();
    let cell_w = |c: &Vec<Span<'static>>| c.iter().map(|s| cols(&s.content)).sum::<usize>();
    const GAP: usize = 3;
    let marker = |shown: usize| {
        if shown == n {
            0
        } else {
            cols(&format!(" +{}", n - shown))
        }
    };
    let mut shown = 0;
    let mut used = 0;
    for c in &cells {
        let next = used + if shown == 0 { 0 } else { GAP } + cell_w(c);
        if next + marker(shown + 1) > avail {
            break;
        }
        used = next;
        shown += 1;
    }
    if shown == 0 {
        return Some(bare());
    }

    let mut spans = vec![Span::styled(label, theme.dim_style())];
    for (i, c) in cells.into_iter().take(shown).enumerate() {
        if i > 0 {
            spans.push(Span::raw(" ".repeat(GAP)));
        }
        spans.extend(c);
    }
    if shown < n {
        spans.push(Span::styled(format!(" +{}", n - shown), theme.dim_style()));
    }
    Some(Line::from(spans))
}

/// The heat ramp's scale, when the panel is wide enough to carry it.
///
/// Semantic heat is a legitimate multi-hue ramp, but only with its scale
/// stated. After G5 the ramp survives in three places — the header figures,
/// the core meters and the process table's CPU column — and the numbers behind
/// the colour change appeared nowhere in the UI. They are the same thresholds
/// the timeline draws rules at, so saying them once is enough.
///
/// Measured in columns, not bytes: `·` is two bytes and one column, so byte
/// length silently over-reserves, and would be three times wrong if the text
/// ever gained a wide character.
fn heat_scale(width: u16, theme: &Theme) -> Option<String> {
    // Read from the theme, not from a constant, so the printed numbers cannot
    // drift from the ones the colouring actually uses. That agreement is the
    // whole value of printing them.
    // `{}` rather than `{:.0}`: 50.0 still prints `50`, but 62.5 prints `62.5`
    // instead of rounding to `62` and claiming the colour changes half a point
    // from where it does. Rounding also made `warn = 49.6` print `50`,
    // indistinguishable from the default the user was trying to move off.
    let scale = format!("· warn {} · crit {} ", theme.warn_pct, theme.critical_pct);
    // Room for the state badge. There are no border columns to reserve since
    // L1 — the title is a full-width content line now.
    const RESERVED: usize = 24;
    (width as usize >= cols(&scale) + RESERVED).then_some(scale)
}

/// Cores drawn between gaps.
const CORE_GROUP: usize = 4;

/// The per-core meters, as one line of the header.
///
/// They were a section of their own: two rows, of which one was a divider and
/// one was a single line of glyphs. That is a section's worth of chrome for a
/// status strip, and it read as a chart when it is really a row of figures.
///
/// One column per core rather than two. A 128-core machine needs 128 columns
/// at one glyph each, which a wide terminal has; at two it needs 256, which
/// nothing has. Cores beyond the width are summarised rather than clipped, so
/// the count is never silently wrong.
fn core_meters(s: &Sample, width: u16, theme: &Theme) -> Line<'static> {
    if s.cpu_per_core.is_empty() {
        return Line::from(Span::styled("cores: not reported", theme.dim_style()));
    }
    let n = s.cpu_per_core.len();
    let w = width as usize;
    let label = format!("{n:>3} cores ");

    // Too narrow even for the label: state the count and draw nothing. A row of
    // meters that cannot say how many are missing is worse than no meters.
    if cols(&label) >= w {
        return Line::from(Span::styled(format!("{n} cores"), theme.dim_style()));
    }
    let avail = w - cols(&label);

    // The marker's width depends on how many are hidden, which depends on how
    // many fit — so shrink until the whole line fits rather than reserving a
    // guess. Reserving `" +{n}"` while printing `" +{overflow}"` wasted a
    // column, and once the reservation exceeded the room available it produced
    // a marker ratatui then clipped: `16 cores  +1` for sixteen hidden cores.
    // Gaps are part of the width now, or the overflow count goes wrong again.
    let with_gaps = |shown: usize| shown + shown.saturating_sub(1) / CORE_GROUP;
    let marker_len = |shown: usize| {
        if shown == n {
            0
        } else {
            cols(&format!(" +{}", n - shown))
        }
    };
    let mut shown = n.min(avail);
    while with_gaps(shown) + marker_len(shown) > avail {
        if shown == 0 {
            break;
        }
        shown -= 1;
    }
    // Even a bare marker does not fit. State the count alone: drawing meters
    // that cannot say how many are missing is the failure this exists to avoid.
    if with_gaps(shown) + marker_len(shown) > avail {
        return Line::from(Span::styled(format!("{n} cores"), theme.dim_style()));
    }

    let mut spans = vec![Span::styled(label, theme.dim_style())];
    for (i, &pct) in s.cpu_per_core.iter().take(shown).enumerate() {
        // A gap every four. Fourteen cores drawn solid read as one progress
        // bar; grouped, they read as fourteen meters, which is what they are.
        if i > 0 && i % CORE_GROUP == 0 {
            spans.push(Span::raw(" "));
        }
        let idx = ((pct / 100.0 * 7.0).round() as usize).min(7);
        spans.push(Span::styled(BARS[idx].to_string(), theme.heat_style(pct)));
    }
    if shown < n {
        spans.push(Span::styled(format!(" +{}", n - shown), theme.dim_style()));
    }
    Line::from(spans)
}

/// Draw just the timeline panel, for visual inspection in tests.
#[cfg(test)]
pub fn draw_timeline_for_test(f: &mut Frame, area: Rect, app: &App) {
    draw_timeline(f, area, app);
}

/// The span of history the timeline is showing: first sample, sample count, and
/// the zoom those samples are aggregated at.
///
/// Extracted so the table's sparklines can be drawn on the same clock. A spike
/// halfway along the timeline has to sit halfway along the row's history too,
/// or the two pictures are of different spans and the reader has to know which
/// before either can be believed.
///
/// Derived rather than stored, like `window_start` itself: it depends on panel
/// width and zoom, both of which are render-time facts.
pub fn shown_window(app: &App, area: Rect) -> (usize, usize, usize) {
    // The drawn width, margin included, or the window the table's sparklines
    // are aggregated over would not be the window the graph shows.
    let area = content(app, area);
    let inner_w = area.width as usize;
    let inner_h = area.height.saturating_sub(1) as usize;
    if inner_w == 0 || inner_h == 0 {
        return (0, 0, 1);
    }
    let graph_rows = inner_h.saturating_sub(1).max(1);
    let gutter = if inner_w >= MIN_WIDTH_FOR_GUTTER && graph_rows >= MIN_ROWS_FOR_AXIS {
        GUTTER_W
    } else {
        0
    };
    let slots = inner_w.saturating_sub(gutter) * app.glyphs.samples_per_cell();
    let len = app.history.len();
    let zoom = app::effective_zoom(app.zoom(), len, slots);
    let shown = (slots * zoom).min(len);
    (window_start(&app.history, shown), shown, zoom)
}

/// The scrubable timeline, oldest on the left.
///
/// Two packings compose here: each character cell holds `samples_per_cell`
/// display slots, and each slot aggregates `zoom` samples by peak. At zoom 5
/// with braille that is ten seconds per cell, so a normal terminal shows the
/// entire buffer.
fn draw_timeline(f: &mut Frame, area: Rect, app: &App) {
    // The graph rows take the margin; the divider above them does not. A
    // divider that stopped short of the edge reads as a box missing its
    // corners, which is why the process panel's spans too.
    let full = area;
    let area = content(app, area);
    let inner_w = area.width as usize;
    let inner_h = area.height.saturating_sub(1) as usize;
    if inner_w == 0 || inner_h == 0 {
        return;
    }

    // Reserve the cursor marker and legend, then divide the rest exactly so no
    // row is left blank. CPU takes the larger share as the spikier signal.
    // One row below the graph, not two: the axis and the caption share it now.
    let graph_rows = inner_h.saturating_sub(1).max(1);

    // A left gutter carrying the scale. Dropped entirely on a narrow panel:
    // four columns of axis is a poor trade against four columns of history
    // when there is little room, and the threshold rules still anchor the
    // graph without it.
    // Reserve the gutter only when some section can actually fill it. A section
    // shorter than `MIN_ROWS_FOR_AXIS` carries no anchors and no label, so the
    // columns would be blank while the graph lost that much history.
    let gutter = if inner_w >= MIN_WIDTH_FOR_GUTTER && graph_rows >= MIN_ROWS_FOR_AXIS {
        GUTTER_W
    } else {
        0
    };
    let graph_w = inner_w.saturating_sub(gutter);

    // One sample a cell. The old packing put two side by side to double the
    // horizontal resolution of an *area*; a line has one stroke a column, and two
    // values sharing a cell would be a smear rather than two readings. The trade
    // is real — half as many samples on screen — and `+`/`-` answers it, since
    // zoom aggregates by peak so a spike survives the compression.
    let spc = app.glyphs.samples_per_cell();
    let slots = graph_w * spc;
    let samples: Vec<&Sample> = app.history.iter().collect();
    // From the shared computation, not a second copy of it: the table's
    // sparklines are drawn on this window too, and two derivations of the same
    // window drift the moment either is touched.
    let (window_start, shown, zoom) = shown_window(app, area);
    // Text-editor scrolling. The window stays anchored to the live edge while
    // the cursor is inside it, and follows only once the cursor would leave —
    // so the live view never shuffles, and scrubbing never takes you somewhere
    // you cannot see.
    //
    // Stateless on purpose: the window position is derived from the cursor each
    // frame rather than stored, so there is no scroll offset to keep in sync
    // with a buffer that is being written to at the same time.
    let window = &samples[window_start..window_start + shown];

    // The selected process's own history, in place of the machine's. Same
    // window, same zoom, same cursor: this is the timeline asking its question
    // of one process rather than a second panel that would have to reimplement
    // all three.
    let subject = app.detail.then(|| app.watched_series(window)).flatten();

    // In the order they earn their place. `WAIT` is second because a machine
    // that is stalled rather than busy is the case a monitor is opened to
    // diagnose, and memory over ten minutes is a flat line or a slow ramp that
    // repeats what the header already says.
    //
    // `WAIT` is absent where the platform will not say, and then memory takes
    // the row back rather than the graph carrying an empty one.
    let mut candidates: Vec<(&str, Vec<f32>, Unit)> = vec![(
        "CPU",
        window.iter().map(|s| s.cpu_total).collect(),
        Unit::Percent,
    )];
    if app.history.current().is_some_and(|s| s.iowait.is_some()) {
        // `unwrap_or` is safe rather than fabricating: whether the platform
        // publishes iowait is a property of the platform, not of the moment,
        // so within one buffer it is `Some` throughout or `None` throughout.
        candidates.push((
            "WAIT",
            window.iter().map(|s| s.iowait.unwrap_or(0.0)).collect(),
            Unit::Percent,
        ));
    }
    candidates.push((
        "MEM",
        window.iter().map(|s| s.mem.used_pct()).collect(),
        Unit::Percent,
    ));

    // Disk utilisation, from whichever device was worst in each sample. Not a
    // series per device: the graph block has room for three or four rows and a
    // machine can have a dozen disks, so the one closest to saturated is the
    // honest summary — the same choice the header figure makes.
    //
    // After memory, not before it. The rows are dropped from the end, and a
    // three-row graph that showed CPU, WAIT and DISK while dropping memory
    // would have made every existing layout worse to add this one. It appears
    // when there is a fourth row to give it.
    //
    // Present only where the platform reads disks at all, so macOS keeps the
    // layout it already had rather than carrying an empty row.
    if app.history.current().is_some_and(|s| s.disks.is_some()) {
        candidates.push((
            "DISK",
            window
                .iter()
                .map(|s| s.busiest_disk().map_or(0.0, |d| d.util))
                .collect(),
            Unit::Percent,
        ));
    }

    // And what the machine lost to waiting, which is not the same question as
    // `WAIT` one row up. `iowait` is the CPU's view — idle with IO outstanding
    // — so a box with plenty of other work to do reports a calm `iowait` while
    // every task that matters is stuck behind the disk. This is the row that
    // catches that, so it is worth a row of its own despite the family
    // resemblance.
    if app.history.current().is_some_and(|s| s.pressure.is_some()) {
        candidates.push((
            "STALL",
            window
                .iter()
                .map(|s| s.pressure.map_or(0.0, |p| p.worst().1))
                .collect(),
            Unit::Percent,
        ));
    }

    // Throughput of the busiest interface, in bytes a second. It was left out
    // while every series had to be a percentage: normalising to the window's
    // own peak makes the busiest sample 100 by construction, so an idle laptop
    // moving 8 B/s of loopback painted a full-scale graph through the critical
    // rule. With its own unit it carries a byte axis and no rules, and says
    // what it is.
    //
    // The busiest link rather than the sum, matching the header figure and for
    // the same reason: a machine can have a dozen interfaces and the panel has
    // room for one row, so the one carrying the most is the honest summary.
    //
    // One interface for the whole line — the header's, chosen over a window.
    // Picking the busiest per sample spliced interfaces together: the line was
    // lo0 where lo0 won and en0 where en0 did (0108).
    if let Some(name) = app
        .headline_link()
        .filter(|_| app.history.current().is_some_and(|s| s.net.is_some()))
    {
        candidates.push((
            "NET",
            window
                .iter()
                .map(|s| {
                    s.net
                        .iter()
                        .flat_map(|n| &n.links)
                        .find(|l| l.name == name)
                        // `Link::bytes`, not `rx + tx`: the same measure the
                        // headline interface is chosen by.
                        .map_or(0.0, |l| l.bytes() as f32)
                })
                .collect(),
            Unit::Rate,
        ));
    }

    // Swapped wholesale rather than merged: a panel showing one process's CPU
    // beside the machine's memory would be two subjects in one graph. Before
    // the split, because the split is derived from how many series there are.
    if let Some(series) = &subject {
        candidates = series
            .rows
            .iter()
            .map(|r| (r.name, r.values.clone(), r.unit))
            .collect();
    }

    let row_split = sections(graph_rows, candidates.len(), gutter);
    candidates.truncate(row_split.len());

    // Slotted once, here, because two things read these values and they have to
    // be the same values: the ceiling below is picked from them, and the rows
    // are drawn from them.
    let slotted: Vec<Vec<Option<f32>>> = candidates
        .iter()
        .map(|(_, raw, _)| history::peak_slots(raw, zoom, slots))
        .collect();

    // One ceiling per unit, not one per panel.
    //
    // Each panel used to walk the ladder on its own peak, which made the stack
    // of graphs move as three pictures rather than one: memory sat at 100 while
    // CPU crossed 25 and jumped to 100 in a single frame, redrawing every
    // sample already on screen a quarter as tall. Nothing about the past had
    // changed — only the axis — and a graph whose history redraws itself is one
    // nobody can read a trend off.
    //
    // It also made the two panels incomparable, which is the older complaint:
    // CPU at 20% on a ceiling of 25 is drawn taller than memory at 72% on a
    // ceiling of 100, and the shapes say the opposite of the figures.
    //
    // Shared, not fixed. A machine idle at 3% CPU and 20% memory still gets a
    // ceiling of 25 rather than a panel of blank rows — which is what a fixed
    // 0..100 axis would cost, and the reason the ladder exists at all. Percent
    // shares with percent and a byte rate with a byte rate; the two never share
    // with each other, because they are not the same question.
    // And held across frames, so it rises the instant the data needs it and
    // falls only once the peak has stayed under it. A byte rate has no natural
    // maximum to pin it to, so without this the network panel redraws its whole
    // history every time a burst arrives or leaves — 512K to 1.0M and back,
    // with the same samples drawn half as tall each way. See `HeldCeilings`.
    //
    // Only while live. Scrubbing is a deliberate move to another span, and a
    // scale chosen by a moment the reader has left is not a scale for the one
    // they are looking at.
    let live = app.history.is_live();
    let unit_ceiling = |unit: Unit| -> f32 {
        let peak = candidates
            .iter()
            .zip(&slotted)
            .filter(|((_, _, u), _)| *u == unit)
            .flat_map(|(_, v)| v.iter().flatten().copied())
            .fold(0.0_f32, f32::max);
        let want = unit.ceiling(peak);
        match samples.last().map(|s| s.at).filter(|_| live) {
            Some(now) => app.ceilings.settle(unit, subject.is_some(), want, now),
            None => {
                // Dropped rather than merely ignored, so coming back to the
                // live edge starts from what is there now instead of from a
                // scale chosen before the reader went looking.
                app.ceilings.forget();
                want
            }
        }
    };

    // Gaps are found over the whole buffer, not the window, so a discontinuity
    // falling on the first drawn sample is still seen — within the window it
    // has no predecessor to be discontinuous with.
    let all_times: Vec<std::time::SystemTime> = samples.iter().map(|s| s.at).collect();
    let all_gaps = history::gaps_in(&all_times, app.interval);
    // Kept apart, not merged. A seam in the machine's own record and a process
    // that was not running are different claims, and captioning both the same
    // way tells the reader that postgres was absent for the ten minutes their
    // laptop was asleep — when in fact the tool was not looking and postgres
    // ran throughout.
    let record_gaps =
        history::any_slots(&all_gaps[window_start..window_start + shown], zoom, slots);
    // Where the process was not running, drawn as a gap rather than as zero. A
    // process that did not exist did not use no CPU — it used none of anything
    // because it was not there, and a flat line at the bottom says the
    // opposite. The moments it started and went are often the whole answer, so
    // they are the one thing this panel must not smooth over.
    let absent_slots = subject
        .as_ref()
        .map(|s| history::any_slots(&s.absent, zoom, slots))
        .unwrap_or_default();
    let or_into = |a: &[bool], b: &[bool]| -> Vec<bool> {
        (0..slots)
            .map(|i| a.get(i).copied().unwrap_or(false) || b.get(i).copied().unwrap_or(false))
            .collect()
    };
    let gap_slots = or_into(&record_gaps, &absent_slots);

    // The threshold rule. Its whole point is that the boundary is readable
    // without colour — until now the 50/80 thresholds existed *only* as a hue
    // change, which is invisible to the most common colour vision deficiency
    // and to anyone on a monochrome terminal. It doubles as the scale anchor
    // the graph otherwise completely lacked.
    // Whether the gutter can name each series itself. A direct label beats a
    // legend — the reader stops having to hold "top is cpu" in their head — but
    // it needs a row that is not already carrying an axis anchor.
    let labelled = gutter > 0 && row_split.iter().all(|&r| r >= MIN_ROWS_FOR_LABEL);

    let mut lines: Vec<Line> = Vec::with_capacity(inner_h);
    for (i, (name, _, unit)) in candidates.iter().enumerate() {
        let rows = row_split[i];
        let values = &slotted[i];
        // Alternating rather than one hue each, because there is no sixth hue
        // to give the third series: the palette avoids green for colour vision
        // reasons and the remaining space is warning-orange or beside `ok`.
        // Alternating guarantees the only thing that matters — that two graphs
        // touching each other never share a colour.
        let series = if i % 2 == 0 {
            app.theme.series_cpu
        } else {
            app.theme.series_mem
        };
        // Each graph scales to its own data: memory at 78% and CPU at 16% are
        // different questions and deserve different axes. The floor moves too —
        // a series living in a narrow band high up gets an axis fitted to that
        // band, because a zero-based panel would spend most of its rows on ink
        // that never changes. See `glyphs::Scale`.
        let peak = values.iter().flatten().copied().fold(0.0_f32, f32::max);
        let trough = values
            .iter()
            .flatten()
            .copied()
            .fold(f32::INFINITY, f32::min);
        let scale = glyphs::Scale::pick(trough, peak, unit_ceiling(*unit), app.axis);
        // Both thresholds, not just critical. The warn boundary is the one the
        // roadmap actually asked for, and leaving it hue-only kept it invisible
        // to the commonest colour vision deficiency and on any mono terminal.
        // Not on a process's own rows. The warn and critical percentages are
        // about a machine's saturation, and ruling them across a row measured
        // in threads or megabytes a second invents a boundary that does not
        // exist — a dashed line at 50 MB/s wearing the chrome that elsewhere
        // means "half of everything there is". It is wrong for per-process CPU
        // too: a process at 283% gets a ceiling of 400, and "critical" lands at
        // 80% of one core.
        let rules: Vec<(usize, usize)> = if subject.is_some() || !unit.takes_thresholds() {
            Vec::new()
        } else {
            [app.theme.warn_pct, app.theme.critical_pct]
                .iter()
                .filter_map(|&pct| glyphs::rule_position(scale, pct, rows))
                .collect()
        };
        // A figure this row could not read joins the gaps, for this row only.
        let row_gaps = match subject.as_ref().and_then(|s| s.rows.get(i)) {
            Some(r) => or_into(&gap_slots, &history::any_slots(&r.unknown, zoom, slots)),
            None => gap_slots.clone(),
        };
        for row in 0..rows {
            let rule_level = rules.iter().find(|(r, _)| *r == row).map(|(_, l)| *l);
            let mut spans = axis_label(
                row,
                rows,
                gutter,
                &app.theme,
                labelled.then_some(name),
                scale,
                *unit,
            );
            spans.extend(
                glyph_row(
                    GraphRow {
                        set: app.glyphs,
                        values,
                        row,
                        rows,
                        spc,
                        rule_level,
                        series,
                        scale,
                        gaps: &row_gaps,
                    },
                    &app.theme,
                )
                .spans,
            );
            lines.push(Line::from(spans));
        }
    }

    let live = app.history.is_live();
    // Computed whichever way the panel went. It used to be gated on there being
    // room for a second row below the axis; there is no second row now, and the
    // gate was quietly emptying the caption — taking the series identification
    // with it on exactly the narrow panels where the gutter cannot label the
    // rows either.
    let ladder = {
        // Real elapsed time, not sample count. A caption saying `4m32s shown`
        // beside a seam saying `time missing` is the graph contradicting
        // itself in adjacent characters — the window really did span nine
        // minutes, and 272 of those seconds are the gap.
        let span = fmt_lag(
            window
                .first()
                .zip(window.last())
                .and_then(|(a, b)| b.at.duration_since(a.at).ok())
                .unwrap_or_default(),
        );
        let per_slot = fmt_lag(app.interval * zoom as u32);
        // Only the identification half is dropped when the gutter names the
        // series. The span, the slot size and the keys are not a legend and
        // are not duplicated anywhere else.
        // Names whatever is actually on screen, in order, when the gutter is
        // too short to label the rows itself. A fixed "cpu · mem" was wrong the
        // moment the series became a decision rather than a constant.
        let ident = if labelled {
            String::new()
        } else {
            format!(
                "{} — ",
                candidates
                    .iter()
                    .map(|(n, _, _)| n.to_lowercase())
                    .collect::<Vec<_>>()
                    .join(" · ")
            )
        };
        // Named only when one is on screen. A seam is self-evidently not data,
        // but "time is missing here" is not something a reader can deduce from
        // a dotted line, and a permanent legend entry for something you may
        // never see is clutter charged against every other frame.
        // Named from whichever mask produced the seams that are actually on
        // screen, not from which panel this is. A hole in the record and a
        // process that was not running are different claims, and a machine
        // that was asleep for ten minutes with postgres selected must not be
        // told postgres was absent — the tool was not looking, and postgres ran
        // throughout. Both can be true at once, and then both are said.
        let mut why: Vec<&str> = Vec::new();
        if record_gaps.iter().any(|&g| g) {
            why.push("time missing");
        }
        if absent_slots.iter().any(|&g| g) {
            why.push("not running");
        }
        let gap_note = if why.is_empty() {
            String::new()
        } else {
            format!(", {} {}", app.glyphs.gap_glyph(), why.join(" / "))
        };
        // Drop the key hints before letting anything be cut mid-word. The
        // scale is a fact about what is on screen and the gap note is a
        // correction to it; the keys are a reminder, and a reminder is the
        // right thing to lose first. Without this the gap note cost about
        // sixteen columns and silently truncated `+/- zoom` on a narrow panel.
        // A ladder rather than a pair, and identification sits at the top of
        // it. `ident` is non-empty only when the gutter could not label the
        // rows, which is exactly when it is the only thing naming them — so it
        // is the last part to go, not the first.
        //
        // Below it: the span, then the slot size, then the key hints, which
        // are already in the footer. Nothing is ever cut mid-phrase; the
        // identification growing from a fixed `cpu · mem` to as much as
        // `cpu · wait · mem` is what pushed the old two-tier version past a
        // narrow panel and let the terminal cut `1s/slot` in half.
        // No key hints. They were the bottom rung of this ladder and are listed
        // in full in the footer of every frame — a reminder that is always on
        // screen twice is not a reminder, it is noise charged against the row
        // it shares.
        [
            format!("{ident}{span} shown, {per_slot}/slot{gap_note}"),
            format!("{ident}{span} shown"),
            ident.trim_end_matches([' ', '—']).trim_end().to_string(),
        ]
    };
    // The widest rung that fits. While live that is a straight width test;
    // while scrubbing the row also carries the cursor, and how much room is
    // left depends on where the marker is — so the choice moves into
    // `cursor_row`, which knows. A single narrower budget could not do it: with
    // the marker halfway across, a caption two columns shorter still lands
    // under it.
    let legend = ladder
        .iter()
        .find(|l| cols(l) <= inner_w)
        .cloned()
        .unwrap_or_default();
    lines.push(if live {
        axis_with_caption(&legend, inner_w, &app.theme)
    } else {
        cursor_row(
            app,
            Window {
                captions: &ladder,
                len: window.len(),
                start: window_start,
                zoom,
                slots,
                spc,
                graph_w,
                gutter,
            },
        )
    });

    // Retained is what the clock says; capacity is what the buffer will hold at
    // the nominal rate, which is a claim about the future and so is nominal by
    // nature. Mixing a measured figure with a projected one is deliberate.
    // Whose history this is. A panel that has changed subject and kept its old
    // name is worse than one that never changed: the graphs look like the
    // machine's and are not.
    //
    // Sized against the panel, with a ladder. The name is a command line and
    // can be any length, so a constant elide width truncated the clause after
    // it — `39s of 9m59s` losing the word `buffered`, or the closing rule — and
    // a title cut mid-phrase is the thing every other title here gives up whole
    // clauses to avoid.
    let span = fmt_lag(app.history.span());
    let cap = fmt_lag(app.interval * app.history.capacity().saturating_sub(1) as u32);
    let title = match &subject {
        Some(_) => {
            let name = app
                .selected
                .as_ref()
                .map_or_else(String::new, |w| w.name().to_string());
            let room = (area.width as usize).saturating_sub(6);
            let fitted = [
                format!("{name} — {span} of {cap} buffered"),
                format!("{name} — {span} of {cap}"),
                format!("{name} — {span}"),
                name.clone(),
            ]
            .into_iter()
            .find(|l| cols(l) <= room)
            // Every rung too long: elide the name itself rather than let the
            // terminal cut it, so what survives identifies the process.
            .unwrap_or_else(|| elide_middle(&name, room));
            format!(" {fitted} ")
        }
        // Asked for with nothing selected. The key is advertised in the
        // footer, so pressing it and getting the panel you already had is the
        // one outcome that reads as broken — and the fix is one arrow key,
        // which nothing on screen would otherwise say.
        None if app.detail && app.selected.is_none() => {
            format!(" timeline — {span} of {cap} — ↑/↓ to pick a process first ")
        }
        None => format!(" timeline — {span} of {cap} buffered "),
    };

    let m = (full.width - area.width) / 2;
    let pad = " ".repeat(m as usize);
    let mut all = vec![divider(&title, full.width, &app.theme)];
    all.extend(lines.into_iter().map(|l| {
        let mut spans = vec![Span::raw(pad.clone())];
        spans.extend(l.spans);
        Line::from(spans)
    }));
    f.render_widget(Paragraph::new(all), full);

    // The time before the buffer starts, said rather than left blank.
    //
    // At the widest zoom the graph is anchored right and the left of the panel
    // is time from before there was any history — meaningful, as
    // `effective_zoom` says, and indistinguishable from a graph with nothing to
    // show. For the first minutes that is most of the biggest panel (0109).
    // Stretching the time axis to fill it was the other answer, and the wrong
    // one: a scale that changed as history arrived would reshape every line on
    // screen during exactly the minutes someone is watching an incident.
    //
    // A clock time rather than "poptop started", because a replayed day's
    // buffer starts where its log does, not where this process did.
    //
    // Placed against `area`, the content rect the rows are drawn in, so the
    // margin is counted once: the rows carry it as padding and this carries it
    // in its origin.
    let used = shown.div_ceil(zoom).div_ceil(spc);
    let empty = graph_w.saturating_sub(used);
    if window_start == 0
        && let Some(first) = window.first()
    {
        let since = crate::log::clock_string(first.at);
        let label = [
            format!("no history before {since} — it fills from the right"),
            format!("no history before {since}"),
            format!("before {since}"),
        ]
        .into_iter()
        .find(|l| cols(l) + 4 <= empty);
        if let Some(label) = label {
            let w = cols(&label);
            f.render_widget(
                Paragraph::new(Span::styled(label, app.theme.dim_style())),
                Rect {
                    x: area.x + (gutter + (empty - w) / 2) as u16,
                    y: area.y + 1 + (graph_rows / 2) as u16,
                    width: w as u16,
                    height: 1,
                },
            );
        }
    }
}

/// A peak that is a value, or `None` for a cell no sample landed in.
///
/// `peak` folds with `f32::max` from `NEG_INFINITY`, so an empty cell comes back
/// as that rather than as a number. Passing it on as zero is the one thing this
/// tool must never do.
fn finite(v: f32) -> Option<f32> {
    v.is_finite().then_some(v)
}

/// One row of graph. `row` counts from the top of a `rows`-tall graph.
/// Everything one graph row needs to draw itself. Bundled because seven
/// positional parameters had become eight and the call site was unreadable.
/// `row` counts from the top of a `rows`-tall graph.
struct GraphRow<'a> {
    set: GlyphSet,
    values: &'a [Option<f32>],
    /// Counted from the top of a `rows`-tall graph.
    row: usize,
    rows: usize,
    /// Samples per character cell.
    spc: usize,
    /// Dot height of a threshold rule crossing this row, if any.
    rule_level: Option<usize>,
    /// Identity of the series — never a judgement about its value.
    series: Color,
    /// The range this graph's rows cover, floor to ceiling.
    scale: glyphs::Scale,
    /// Per-slot flags marking where time is missing from the buffer.
    gaps: &'a [bool],
}

/// Draw one row of a graph.
fn glyph_row(g: GraphRow, theme: &Theme) -> Line<'static> {
    let (set, values, row, rows, spc, rule_level, series, scale, gaps) = (
        g.set,
        g.values,
        g.row,
        g.rows,
        g.spc,
        g.rule_level,
        g.series,
        g.scale,
        g.gaps,
    );
    let spans = values
        .chunks(spc)
        .enumerate()
        .map(|(i, cell)| {
            // A seam where time is missing, drawn full height and in chrome so
            // it cannot be read as a bar.
            //
            // It costs the whole cell — `spc * zoom` samples, so two at the
            // default and sixteen at maximum zoom on a braille terminal. That
            // is a real loss and worth naming: a machine that has just woken
            // or just unwedged is exactly when a spike is likely, and a spike
            // in the sample beside the resumed one is hidden by this.
            //
            // Taken anyway, because the alternative is worse in kind rather
            // than in degree. Compressing twenty minutes of absence into one
            // cell of idle is not a lost sample, it is a graph whose x-axis is
            // untrue, and every reading taken from it after that is wrong. The
            // cost also scales the right way: the more samples a cell covers,
            // the more time the seam is standing for.
            if gaps
                .get(i * spc..(i * spc + spc).min(gaps.len()))
                .is_some_and(|g| g.iter().any(|&f| f))
            {
                return Span::styled(set.gap_glyph().to_string(), theme.gap_style());
            }
            // This cell's value and the next one, so the stroke can join them.
            // The peak within a cell, matching how zoom aggregates: a line drawn
            // through the mean would smooth away the spike the tool exists to
            // catch.
            let peak = |c: &[Option<f32>]| {
                c.iter()
                    .filter_map(|v| *v)
                    .fold(f32::NEG_INFINITY, f32::max)
            };
            let here = finite(peak(cell));
            // A cell with no sample draws nothing. An area fill got this for
            // free — level zero is a blank glyph — but a line does not: it
            // would draw a flat stroke along the baseline across the part of
            // the buffer that has not been filled yet, which says the machine
            // was idle then. It was not. Nothing was recorded then.
            let Some(here) = here else {
                return Span::raw(" ");
            };
            // Joined to the next cell only if there is one. Running the stroke
            // into an empty cell invents the same zero at one remove.
            let next = values
                .get((i + 1) * spc..((i + 2) * spc).min(values.len()))
                .map(peak)
                .and_then(finite)
                .unwrap_or(here);
            // The form follows the axis, not the setting. Bars encode
            // magnitude by area, so a truncated axis makes 74 look like a third
            // of 84 — the classic misleading chart. A line encodes change, for
            // which a fitted axis is standard and honest, and the gutter states
            // the floor either way.
            let draws = if scale.fitted {
                glyphs::Draw::Line
            } else {
                set.draws()
            };
            let glyph = match draws {
                // A bar from the baseline to the value. Every cell below the
                // value is full, the cell the value lands in is part-full, and
                // everything above is empty — the shape a sparkline has always
                // had, read as height rather than traced as a path.
                glyphs::Draw::Bars => set.bar(glyphs::fill_in_row(
                    scale.frac(here),
                    row,
                    rows,
                    set.sub_rows(),
                )),
                // Box drawing needs the direction of travel to pick a corner,
                // which a height cannot carry.
                glyphs::Draw::Line => set.line(scale.frac(here), scale.frac(next), row, rows),
            };
            // Colour is identity here, not magnitude — see `Theme::series_style`.
            // The threshold rules now carry "is this bad", which is what the
            // heat ramp was doing redundantly on top of the bar height.
            // Data always wins the cell. The rule fills gaps only, and dashes
            // so it reads as a reference line rather than a row of samples — at
            // the mono tier a solid rule is indistinguishable from a low bar,
            // since both render a dim `⣀`.
            // A cell with no sample at all is not the same as a cell whose
            // bar does not reach this row. Drawing the rule across the part of
            // the buffer that has not been filled yet is noise about a region
            // where there is nothing to reference.
            // How often the rule shows through depends on how much of the panel
            // the series leaves empty. A bar leaves the space *above* it, a
            // minority on a busy machine; a line leaves nearly every cell, so
            // the same spacing would paint half the panel in chrome and the
            // reference would compete with the signal.
            // Proportional to the panel, not a fixed stride. Every second cell
            // was tuned against an area fill that reached most of them, and it
            // is roughly forty marks on a hundred-column terminal: on an idle
            // machine, where nearly every cell is empty, that is not a
            // reference line but the loudest thing on the screen. A reference
            // has to be findable and recessive at the same time, and about ten
            // marks across a panel is both however wide the panel is.
            let cells = values.len().div_ceil(spc.max(1));
            let every = (cells / 10).clamp(4, 24);
            // The alphabet in force is a function of the set *and* the form,
            // and both of these questions are asked of it: which character
            // means "empty", and which one means "a reference line". A fitted
            // braille panel is drawn in box characters, whose empty is a space
            // — while braille's own is `U+2800`, so asking the set alone said
            // no cell was ever empty and no rule was ever drawn.
            let alphabet = set.drawn_as(draws);
            match rule_level {
                Some(lvl) if glyph == alphabet.blank() && i % every == 0 => {
                    let mark = alphabet.rule_glyph(lvl);
                    Span::styled(mark.to_string(), theme.chrome_style())
                }
                _ => Span::styled(glyph.to_string(), theme.series_style(series)),
            }
        })
        .collect::<Vec<_>>();
    Line::from(spans)
}

/// How many series a graph of this height carries, and how many rows each gets.
///
/// The length of the result is how many of the candidates fit; the values are
/// their row counts. One function rather than a count and a split, because the
/// two have to agree — and the tests need the same answer the renderer used.
///
/// A series is only worth a row if it can be named. Identity in the timeline
/// rests on the gutter label, since the palette has five meaning-bearing hues
/// and no room for a sixth, so an unlabelled extra series would be two
/// indistinguishable graphs stacked on each other.
///
/// The remainder goes to the first, which is CPU: it is the spikiest signal and
/// the one where a dot of extra vertical resolution buys the most.
pub fn sections(graph_rows: usize, candidates: usize, gutter: usize) -> Vec<usize> {
    // Without a gutter there are no per-row labels, so the legend is doing the
    // naming — and it names them in one flat list that the reader has to map
    // onto the stack by position. That is workable for two rows and guesswork
    // for three, especially since the hues alternate and a third graph shares
    // the first one's colour. So a gutterless panel carries the two that answer
    // the question and stops.
    let ceiling = if gutter == 0 { 2 } else { candidates.max(1) };
    let series = (graph_rows / MIN_ROWS_FOR_LABEL).clamp(1, candidates.max(1).min(ceiling));
    let base = graph_rows / series;
    let extra = graph_rows % series;
    (0..series).map(|i| base + usize::from(i < extra)).collect()
}

/// Every name the timeline gutter may have to hold.
///
/// Written down so [`GUTTER_W`] can be derived from it. `STALL` was added and
/// silently rendered as `STAL` for exactly as long as the width was a hand-
/// maintained number with a comment claiming `WAIT` was the longest.
pub const SERIES_NAMES: [&str; 7] = ["CPU", "WAIT", "MEM", "DISK", "STALL", "NET", "THR"];

const fn widest(names: &[&str]) -> usize {
    let (mut max, mut i) = (0, 0);
    while i < names.len() {
        if names[i].len() > max {
            max = names[i].len();
        }
        i += 1;
    }
    max
}

/// Width of the scale gutter, and the panel width below which it is dropped.
///
/// One wider than the longest series name: [`axis_label`] right-aligns into
/// `gutter - 1` so a label never abuts its graph. Derived rather than written
/// down, because a gutter that cannot hold its own labels truncates them
/// silently — nothing looks wrong, the name is simply a letter shorter.
pub const GUTTER_W: usize = widest(&SERIES_NAMES) + 1;
const MIN_WIDTH_FOR_GUTTER: usize = 30;
/// A section shorter than this cannot carry both ends of the scale, so it
/// carries none: see [`axis_label`].
const MIN_ROWS_FOR_AXIS: usize = 2;
/// A section needs a row spare — beyond the two anchors — to name itself.
const MIN_ROWS_FOR_LABEL: usize = 3;

// The gutter must fit inside the panel it is dropped from, or `graph_w`
// underflows. The two constants are unrelated by construction, so tie them.
const _: () = assert!(MIN_WIDTH_FOR_GUTTER > GUTTER_W);

/// Exposed for tests: the names the gutter has to be wide enough for.
#[cfg(test)]
pub fn series_names() -> &'static [&'static str] {
    &SERIES_NAMES
}

/// Exposed for tests: where a stall percentage lands on the theme's scale.
#[cfg(test)]
pub fn stall_heat_for_test(pct: f32, theme: &Theme) -> f32 {
    stall_heat(pct, theme)
}

/// Exposed for tests: where a steal percentage lands on it.
#[cfg(test)]
pub fn steal_heat_for_test(pct: f32, theme: &Theme) -> f32 {
    steal_heat(pct, theme)
}

/// Exposed for tests: where a dirty share lands on it.
#[cfg(test)]
pub fn dirty_heat_for_test(share: f32, theme: &Theme) -> f32 {
    dirty_heat(share, theme)
}

/// Exposed for tests: the gutter's width guarantee is a claim about a string,
/// and the only way to check it is to read one. Same pattern as
/// `draw_timeline_for_test`.
#[cfg(test)]
pub fn axis_label_for_test(row: usize, rows: usize, name: Option<&str>) -> String {
    axis_label(
        row,
        rows,
        GUTTER_W,
        &Theme::new(crate::theme::Palette::Safe, Theme::default().tier),
        name,
        glyphs::Scale::zero(100.0),
        Unit::Percent,
    )
    .iter()
    .map(|s| s.content.as_ref())
    .collect()
}

/// The scale marks for one graph row: `100` on the top row, `0` on the bottom.
///
/// A percentage graph always spans 0..100, so these do not tell a reader
/// anything they could not assume — what they do is anchor the *geometry*, so
/// a bar's height can be read as a value rather than only compared to its
/// neighbours.
/// What a series is measured in, which decides its axis, its rules and whether
/// a threshold means anything to it.
///
/// The panel used to assume every series was a percentage of a fixed
/// denominator: it printed a bare ceiling, ruled the warn and critical
/// thresholds across the graph, and any row not measured that way had to be
/// left out. Bytes per second have no denominator — a link's capacity is not
/// portably knowable, and normalising to the window's own peak makes the
/// busiest sample 100 by construction, so an idle laptop moving 8 B/s of
/// loopback drew a full-scale graph through the critical rule.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    /// A share of a fixed whole. The axis prints a number and the warn and
    /// critical rules apply, because those are percentages.
    Percent,
    /// Bytes a second. The axis prints `4.0M`, and no rule is drawn: there is
    /// no threshold at which a number of bytes is "critical".
    Rate,
    /// A plain count, like threads. An axis, and no rules for the same reason.
    Count,
}

impl Unit {
    /// Which slot of [`crate::app::HeldCeilings`] this unit's ceiling lives in.
    pub fn slot(self) -> usize {
        match self {
            Unit::Percent => 0,
            Unit::Rate => 1,
            Unit::Count => 2,
        }
    }
}

#[cfg(test)]
impl Unit {
    pub fn takes_thresholds_for_test(self) -> bool {
        self.takes_thresholds()
    }
    pub fn axis_for_test(self, ceiling: f32) -> String {
        self.axis(ceiling)
    }
    pub fn ceiling_for_test(self, peak: f32) -> f32 {
        self.ceiling(peak)
    }
}

impl Unit {
    /// How the axis writes this series' ceiling.
    fn axis(self, ceiling: f32) -> String {
        match self {
            Unit::Percent | Unit::Count => format!("{ceiling:.0}"),
            Unit::Rate => axis_bytes(ceiling as u64),
        }
    }

    /// Whether the machine's warn and critical percentages mean anything here.
    fn takes_thresholds(self) -> bool {
        self == Unit::Percent
    }

    /// A ceiling in this series' own units, rounded to something legible.
    fn ceiling(self, peak: f32) -> f32 {
        match self {
            Unit::Percent => glyphs::ceiling_for(peak),
            // Powers of two from a kilobyte. A byte rate has no natural
            // hundred, and a ladder in its own base is what makes `4.0M`
            // readable where `3.7M` is arithmetic.
            // Strictly above the peak, not merely at it. `while c < peak`
            // returns the peak exactly whenever the peak is a power of two, and
            // the row is then drawn solid to the top — the busiest sample being
            // 100 by construction, which is the failure that kept this row out
            // of the panel to begin with.
            //
            // Counts start at eight rather than one, or a single-threaded
            // process gets a ceiling of one and a permanently saturated row.
            Unit::Rate | Unit::Count => {
                let mut c = if self == Unit::Rate { 1024.0 } else { 8.0 };
                while c <= peak {
                    c *= 2.0;
                }
                c
            }
        }
    }
}

/// A byte figure for the axis gutter, in at most five columns.
///
/// `fmt_bytes` writes one decimal always, so `128.0K` is six characters and the
/// gutter's hard truncation left `128.0` — a byte rate drawn as what looks
/// exactly like a percentage, which is the confusion this row was excluded to
/// avoid. Three of every ten rungs on the power-of-two ladder land there.
///
/// The decimal is dropped once the mantissa has three digits, where it was
/// never carrying information anyway: the ladder only ever produces 1, 2, 4 …
/// 512, so the choice is between `512.0K` and `512K` and never between `512.0K`
/// and `512.4K`.
fn axis_bytes(b: u64) -> String {
    const UNITS: [&str; 5] = ["B", "K", "M", "G", "T"];
    let mut v = b as f64;
    let mut i = 0;
    while v >= 1024.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 || v >= 100.0 {
        format!("{v:.0}{}", UNITS[i])
    } else {
        format!("{v:.1}{}", UNITS[i])
    }
}

fn axis_label(
    row: usize,
    rows: usize,
    gutter: usize,
    theme: &Theme,
    series: Option<&str>,
    scale: glyphs::Scale,
    unit: Unit,
) -> Vec<Span<'static>> {
    if gutter == 0 {
        return Vec::new();
    }
    // The caller decides whether a section is tall enough to name itself. Tie
    // that decision to this function, or a second caller could pass a label
    // into a two-row section and watch it silently vanish.
    debug_assert!(
        series.is_none() || rows >= MIN_ROWS_FOR_LABEL,
        "a label was passed to a section with only {rows} rows"
    );
    // A section only one row tall spans the entire 0..100 range in that row.
    // Labelling its top `100` states that the top of the row is the maximum,
    // which is true, while implying the bottom is not the minimum — and `0`
    // never appears at all. An axis that misleads is worse than no axis, so a
    // section too short to carry both ends carries neither.
    let text = if rows < MIN_ROWS_FOR_AXIS {
        String::new()
    } else if row == 0 {
        // The ceiling, not a fixed 100 — the axis has to say what it is, or
        // scaling it would be the misleading kind of clever. In the series'
        // own units, because `4194304` is arithmetic and `4.0M` is a scale.
        unit.axis(scale.ceiling)
    } else if row + 1 == rows {
        // The floor, not a fixed `0`. A fitted axis that labelled its bottom
        // row zero would be the misleading kind of clever — the whole reason
        // the floor is allowed to move is that it is stated when it does.
        unit.axis(scale.floor)
    } else if row == 1 {
        // The first row not already carrying an anchor.
        series.unwrap_or_default().to_string()
    } else {
        String::new()
    };
    // Right-aligned in `gutter - 1`, leaving one column of separation, so the
    // label stays exactly `gutter` wide whatever the constant becomes.
    //
    // Truncated, not just padded: a format width is a *minimum*, so a label one
    // character too long silently widened its own row and shifted that graph
    // sideways relative to the ones above it. Two graphs whose columns are not
    // the same instant are worse than one graph.
    let w = gutter.saturating_sub(1);
    let text: String = text.chars().take(w).collect();
    vec![Span::styled(format!("{text:>w$} "), theme.dim_style())]
}

/// Index of the first sample drawn.
///
/// The buffer is divided into pages of `shown` samples counted back from the
/// live edge, and the window shows whichever page the cursor is on. Page 0 is
/// the live window, so the live view is unchanged.
///
/// Paging rather than following. A window derived directly from the cursor
/// drags one sample sideways on every keypress — the graph slides under the
/// reader, and at `zoom > 1` the buckets re-form so bar heights change too. It
/// also pins the cursor to column 0, which means never seeing anything *older*
/// than where you are: exactly the behaviour G7 exists to remove, reintroduced.
///
/// Paging gives that hysteresis without storing a scroll offset. `ui::draw`
/// takes `&App`, and the window size depends on panel width and zoom — both
/// render-time facts — so a remembered offset would need interior mutability
/// or a mutable draw. Deriving the page from the cursor needs neither.
///
/// `peak_slots` remains right-aligned within whatever slice it is given; only
/// the choice of slice changed. The right edge is therefore "newest in the
/// window" rather than "now", which is why the axis claims `now` only while
/// live and shows the cursor's own figures otherwise.
fn window_start(history: &history::History, shown: usize) -> usize {
    let len = history.len();
    let last_page = len.saturating_sub(shown);
    if history.is_live() || shown == 0 {
        return last_page;
    }
    // Pages counted back from the live edge, so page 0 is the live window.
    let from_newest = len.saturating_sub(1) - history.cursor_index();
    let page = from_newest / shown;
    last_page.saturating_sub(page * shown)
}

/// Where the timeline's window sits and how it maps to columns. Bundled for
/// the same reason `GraphRow` is: the parameter list had outgrown readability.
struct Window<'a> {
    /// The caption ladder, widest rung first. This row picks its own rung
    /// because only it knows where the marker is, and the room left for text
    /// is whatever the marker is not standing in.
    captions: &'a [String],
    /// Samples drawn.
    len: usize,
    /// Index of the first sample drawn.
    start: usize,
    zoom: usize,
    slots: usize,
    /// Samples per character cell.
    spc: usize,
    /// Columns available to the graph, excluding the gutter.
    graph_w: usize,
    gutter: usize,
}

/// The row under the graph while the timeline is live: which way time runs,
/// and what one cell is worth.
///
/// The two used to be separate rows. `past … now` and `1s shown, 1s/slot` are
/// both statements about the same axis, and neither is worth a row of a
/// thirty-row terminal on its own — every chrome row is a process the table
/// cannot show.
///
/// The caption is centred between the anchors rather than left-aligned under
/// them, so the row reads as one axis instead of three left-aligned fragments.
/// It is dropped, not truncated, when the panel is too narrow to hold all
/// three: a caption cut mid-phrase is worse than no caption, and `past`/`now`
/// is the half a first-time reader needs.
fn axis_with_caption(caption: &str, width: usize, theme: &Theme) -> Line<'static> {
    const ANCHORS: usize = 4 + 3; // "past" and "now"
    // The full panel width, gutter included. `past` marks the oldest sample on
    // screen and the gutter is left of every sample there is, so the anchor
    // belongs at column zero — and a row indented past the gutter leaves the
    // leftmost column of the panel unused on every frame.
    let n = cols(caption);
    let text = if caption.is_empty() {
        format!("{:<w$}now", "past", w = width.saturating_sub(3))
    } else if n + ANCHORS + 4 <= width {
        let room = width - ANCHORS - n;
        let left = room / 2;
        format!(
            "past{}{caption}{}now",
            " ".repeat(left),
            " ".repeat(room - left)
        )
    } else {
        // The anchors go, not the caption. The caption has its own ladder and
        // gives up identification last — which on a panel this narrow is the
        // only thing naming the rows, since the gutter cannot label them
        // either. Dropping it here would leave an unlabelled graph.
        caption.to_string()
    };
    Line::from(Span::styled(text, theme.dim_style()))
}

/// The row under the graph marking where the scrub cursor sits.
///
/// When a cell holds two samples the marker picks the correct half, so packing
/// never costs cursor precision.
fn cursor_row(app: &App, w: Window<'_>) -> Line<'static> {
    let (n_values, window_start, zoom, slots, spc, graph_w, gutter) =
        (w.len, w.start, w.zoom, w.slots, w.spc, w.graph_w, w.gutter);
    let pad = " ".repeat(gutter);
    // Live is handled by `axis_with_caption`; this stays for the empty buffer,
    // where there is no cursor to place and no span to caption.
    if app.history.is_live() || n_values == 0 {
        return Line::from(Span::styled(
            format!(
                "{:<width$}now",
                "past",
                width = (gutter + graph_w).saturating_sub(3)
            ),
            app.theme.dim_style(),
        ));
    }

    // The cursor is an index into the whole buffer; the graph shows a window of
    // the newest `n_values`, so rebase before locating it.
    // Paging always contains the cursor, so this is unreachable. Asserted in
    // debug so a windowing mistake is loud in tests, but kept as a fallback in
    // release: a wrong marker is a bug, a panicking monitor is worse.
    debug_assert!(
        app.history.cursor_index() >= window_start,
        "cursor {} precedes window start {window_start}",
        app.history.cursor_index()
    );
    if app.history.cursor_index() < window_start {
        let mut row = vec![' '; graph_w];
        if let Some(c) = row.first_mut() {
            *c = '◀';
        }
        return Line::from(vec![
            Span::raw(pad),
            Span::styled(
                row.into_iter().collect::<String>(),
                app.theme.cursor_style(),
            ),
        ]);
    }

    let idx = app.history.cursor_index() - window_start;
    let slot = history::slot_of_index(idx, n_values, zoom, slots);

    let cell = (slot / spc).min(graph_w.saturating_sub(1));
    let marker = app.glyphs.cursor_marker();

    // This row is positional. It used to carry the values at the cursor as
    // well, which read as a crosshair readout and was in fact a copy: while
    // scrubbing the header *is* showing the sample under the cursor, so
    // `CPU 16.5%  MEM 82.7%` appeared twice on the same screen, two
    // centimetres apart.
    //
    // What it carries instead is what only it can say — where in time the
    // cursor is — plus the two things a scrubbing reader actually wants from
    // it. The anchors, so the marker's position means something; and the slot
    // size, which was dropped entirely while scrubbing because this row
    // replaced the caption that used to state it. The exact lag stays in the
    // header, where the state marker's loudness is what stops a stale table
    // being read as live.
    // The whole panel width, gutter included, exactly as the live axis is
    // drawn: `past` marks the oldest sample on screen and the gutter is left of
    // every sample there is, so the anchor belongs at column zero. Indenting
    // this row by the gutter made the axis jump six columns sideways the moment
    // anyone pressed an arrow key.
    let width = gutter + graph_w;
    let mut row = vec![' '; width];
    let put = |row: &mut Vec<char>, at: usize, s: &str| {
        for (i, c) in s.chars().enumerate() {
            if let Some(slot) = row.get_mut(at + i) {
                *slot = c;
            }
        }
    };
    // The marker is the one thing here that must line up with the graph, so it
    // alone is measured from the gutter.
    let cell = gutter + cell;

    const ANCHOR_L: usize = 4;
    const ANCHOR_R: usize = 3;
    // The widest rung that fits entirely on one side of the marker, preferring
    // to keep the anchors. Without this the caption was placed under the marker
    // and lost a character to it — `1▐/slot`, which names nothing and hides the
    // scale it was there to state.
    let room = |a: usize, b: usize| b.saturating_sub(a);
    //
    // Both the side and the column are independent of where exactly the marker
    // is. Centring the caption in the space beside it made the caption chase
    // the cursor across the row, sliding a column on every keypress; the side
    // now flips once, when the cursor crosses the midpoint, and the caption
    // sits at a fixed column on whichever side it lands.
    let side = |n: usize, anchors: bool| -> Option<usize> {
        let (l0, r1) = if anchors {
            (ANCHOR_L + 1, width.saturating_sub(ANCHOR_R))
        } else {
            (0, width)
        };
        let pad = if anchors { 2 } else { 1 };
        let fits_left = room(l0, cell.min(r1)) >= n + pad;
        let fits_right = room((cell + 1).max(l0), r1) >= n + pad;
        let at_left = l0 + 1;
        let at_right = r1.saturating_sub(n + 1);
        // The caption takes the half the marker is not in.
        match (cell * 2 < width, fits_left, fits_right) {
            (true, _, true) => Some(at_right),
            (false, true, _) => Some(at_left),
            (_, _, true) => Some(at_right),
            (_, true, _) => Some(at_left),
            _ => None,
        }
    };
    // Anchors first, because they are what make the marker's position mean
    // anything; only when no rung fits beside them are they given up — the same
    // choice `axis_with_caption` makes while live, and for the same reason: on
    // a panel this narrow the caption is the only thing naming the rows, since
    // the gutter cannot label them either.
    let chosen = w
        .captions
        .iter()
        .find_map(|c| {
            let n = cols(c);
            (n > 0 && n <= width).then(|| side(n, true).map(|s| (c.as_str(), s, true)))?
        })
        .or_else(|| {
            w.captions.iter().find_map(|c| {
                let n = cols(c);
                (n > 0 && n <= width).then(|| side(n, false).map(|s| (c.as_str(), s, false)))?
            })
        });

    match chosen {
        Some((caption, at, anchors)) => {
            if anchors {
                // An anchor the marker would land in is not drawn at all. That
                // is agreement rather than collision — a marker at the right
                // edge *is* `now` — and the alternative is overwriting one
                // character of it and leaving `▌ow`, which names nothing.
                if cell >= ANCHOR_L {
                    put(&mut row, 0, "past");
                }
                if cell < width.saturating_sub(ANCHOR_R) {
                    put(&mut row, width.saturating_sub(ANCHOR_R), "now");
                }
            }
            put(&mut row, at, caption);
        }
        // Nothing to say but where the cursor is.
        None => {
            if cell >= ANCHOR_L {
                put(&mut row, 0, "past");
            }
            if cell < width.saturating_sub(ANCHOR_R) {
                put(&mut row, width.saturating_sub(ANCHOR_R), "now");
            }
        }
    }

    // Last, so it wins its cell outright: a marker a caption can overwrite is
    // a marker that sometimes lies about where the cursor is.
    let head: String = row[..cell].iter().collect();
    let tail: String = row[cell + 1..].iter().collect();
    Line::from(vec![
        Span::styled(head, app.theme.dim_style()),
        Span::styled(marker.to_string(), app.theme.cursor_style()),
        Span::styled(tail, app.theme.dim_style()),
    ])
}

/// The narrowest table that can carry the disk IO columns.
///
/// Every column here is a fixed `Length`, so ratatui squeezes them all when
/// they do not fit rather than dropping any — which turned an eighty-column
/// terminal into a table of truncated figures the moment the columns became a
/// default. Adding up what the table actually asks for is the only way to know
/// where that starts, and doing it here rather than by eye means it cannot
/// drift as columns change.
/// The shape a plain CPU table has, as the width arithmetic's starting point.
fn cpu_shape(show_io: bool, show_user: bool) -> TableShape {
    TableShape {
        bars: true,
        thr: true,
        io: show_io,
        pss: false,
        vsize: false,
        majflt: false,
        grow: false,
        rss: true,
        state: true,
        pid: true,
        spark: true,
        // The widest the table can be, which is what this shape is for: the
        // question it answers is where the columns stop fitting, and a
        // buffer that happens to be flat today is not a width.
        flat: false,
        user: show_user,
        cid: false,
    }
}

/// Room enough for a command name to be worth reading, over the column's floor.
///
/// What `min_width_for_io` spends on top of the table's own request: the disk
/// figures are worth having only if what they crowd out still identifies the
/// row they are on.
const COMMAND_WORTH_READING: u16 = 16;

/// The narrowest terminal the IO columns will appear on.
///
/// Takes `show_user` for the same reason [`command_width`] does: when that
/// column has been folded into the title its ten columns are free, and the IO
/// columns were refusing to appear until the terminal was eleven columns wider
/// than they needed to be.
#[cfg(test)]
pub fn command_width_for_test(width: u16, show_io: bool, show_user: bool) -> usize {
    command_width(&cpu_shape(show_io, show_user), width, 1)
}

#[cfg(test)]
pub fn min_width_for_io_for_test(show_user: bool) -> u16 {
    min_width_for_io(show_user)
}

fn min_width_for_io(show_user: bool) -> u16 {
    table_request(&cpu_shape(true, show_user), 1) - MIN_COMMAND_W + COMMAND_WORTH_READING
}

/// The command column's own `Constraint::Min`, and so the narrowest it is ever
/// actually drawn at.
const MIN_COMMAND_W: u16 = 10;

/// Major faults a second above which the column is worth looking at.
///
/// A rate, not a percentage — which is why it is not `heat_style`'s threshold.
/// Ten a second is a process being paged in steadily rather than one taking the
/// odd fault at startup, and anything above that is the answer to why it is
/// slow.
const MAJFLT_WARN: u32 = 10;

/// Twelve characters, which is what `docker ps` shows.
const CID_W: u16 = 12;

/// The header over the sparkline column, carrying its scale.
///
/// The scale used to be a clause in the section title, three metres from the
/// column it described. A legend belongs with the thing it explains, and the
/// column header is as close as it gets.
///
/// One ceiling is shared by every row so the shapes can be compared, which
/// means the column *has* a scale — and an unlabelled scale that moves is the
/// same trap as an unlabelled y-axis. It steps 10 / 25 / 50 / 100 below one
/// core, a tenfold swing: a column read at 10% one second and 100% the next,
/// because one process briefly touched 60%, has changed every shape in it with
/// nothing said.
///
/// Named as well as scaled when both fit in [`SPARK_W`], and scaled alone when
/// they do not — the scale is the part that cannot be guessed from a column of
/// braille.
#[cfg(test)]
pub fn spark_header_for_test(ceiling: f32) -> String {
    spark_header(ceiling)
}

fn spark_header(ceiling: f32) -> String {
    let scale = format!("≤{ceiling:.0}%");
    // Three tiers, because the ceiling doubles past one core and the name is
    // the part that runs out of room first. On a sixteen-core box a busy
    // process gives a ceiling of 1600, and `HIST ≤1600%` is eleven columns
    // against ten — which used to leave the bare scale and nothing anywhere on
    // screen saying that column was history, on exactly the machines where the
    // sparkline matters most. `H` is a stub, but it is a stub of a name.
    for candidate in [format!("HIST {scale}"), format!("H {scale}"), scale] {
        if cols(&candidate) <= SPARK_W {
            return candidate;
        }
    }
    // A ceiling wide enough to crowd out even `≤N%` would need a machine with
    // hundreds of cores and a process using all of them.
    format!("≤{:.0}", ceiling / 100.0)
}

/// Width of the `USER` column, and the width `COMMAND` gets back when it is
/// folded into the title. See [`crate::app::App::one_user`].
pub const USER_W: u16 = 10;

/// How much of the line is left for the command name.
///
/// The identity column is the one that takes what nothing else claimed, so it
/// is the one that runs out — at 104 columns with the IO columns shown it gets
/// nineteen, one more than the two disk-rate columns together. Knowing the
/// figure is what lets the name be elided deliberately rather than clipped by
/// the terminal.
///
/// Asked of the same column list the table is laid out from, rather than added
/// up again here. The hand-added version had to know which columns a view
/// drops and which it adds, and it got that wrong three times: once too
/// generous by thirteen columns and the command chopped at the right edge with
/// no elision mark, once too cautious and a name cut for no reason, once
/// thirty-five columns out on the memory tab. There is nothing left to get
/// wrong when the two arithmetics are one arithmetic.
///
/// Floored at the column's own `Min`, because below that width ratatui stops
/// honouring the fixed lengths and squeezes them instead — the command cell is
/// then *wider* than this says, and eliding against the arithmetic rendered
/// `Google Chrome Helper (Renderer)` as the single letter `G`.
fn command_width(shape: &TableShape, width: u16, gap: u16) -> usize {
    let others = table_request(shape, gap) - MIN_COMMAND_W;
    width.saturating_sub(others).max(MIN_COMMAND_W) as usize
}

/// A signed byte delta, with the sign carried rather than implied.
///
/// `+400M` and `-400M` are different facts about a process and a bare `400M`
/// is neither. Zero is written as `·`, the same mark the IO columns use for a
/// real nothing, so a row that did not move reads as flat rather than as a
/// growth of nothing.
fn fmt_growth(delta: i64) -> String {
    match delta {
        0 => "·".into(),
        d if d > 0 => format!("+{}", fmt_bytes(d as u64)),
        d => format!("-{}", fmt_bytes(d.unsigned_abs())),
    }
}

/// A tree prefix trimmed so the name it indents still has room to be read.
///
/// Returns the prefix to draw and the columns left for the name. Deep enough
/// nesting starves the identifier — three columns a level against a column that
/// is nineteen wide — and a row that says only `└` says nothing at all. The
/// indent is trimmed from the left, which is where its repeated spacing lives,
/// so the connector that shows *this* row's relationship survives.
fn fit_prefix(prefix: &str, cmd_w: usize) -> (String, usize) {
    /// Enough for a head, an elision mark and a tail.
    const FLOOR: usize = 5;
    let n = cols(prefix);
    if n + FLOOR <= cmd_w {
        return (prefix.to_string(), cmd_w - n);
    }
    let keep = cmd_w.saturating_sub(FLOOR);
    let kept = take_cols(prefix, keep, true);
    // What the prefix actually took, not what it was allowed: a tree spine of
    // double-width characters can come up a column short, and the name should
    // have that column rather than nobody having it.
    let room = cmd_w.saturating_sub(cols(&kept));
    (kept, room)
}

/// A name shortened to `w` columns, keeping both ends.
///
/// Cutting the tail is what the terminal does on its own, and for a process
/// name it removes exactly the part that tells two of them apart: three rows
/// reading `Google Chrome Helpe` are a renderer, a GPU process and a network
/// service. Cutting the head is no better — `…Helper (Renderer)` could belong
/// to any application on the machine.
///
/// So both ends stay and the middle goes. The head keeps slightly more, because
/// it is what a reader scans down the column for; the tail keeps enough to carry
/// a parenthetical role.
pub fn elide_middle(name: &str, w: usize) -> String {
    let n = cols(name);
    if n <= w {
        return name.to_string();
    }
    if w <= 3 {
        return take_cols(name, w, false);
    }
    // The mark itself is a column. Budgeted from `w`, not from the halves, so a
    // double-width character straddling the boundary cannot push the result one
    // column over — which is the whole failure this function exists to prevent.
    let room = w - 1;
    let tail = room / 2;
    let head = room - tail;
    let mut out = take_cols(name, head, false);
    out.push('…');
    out.push_str(&take_cols(name, tail, true));
    out
}

/// As much of `s` as fits in `w` columns, from the front or the back.
///
/// Measured with [`cols`] on the candidate itself rather than by summing
/// per-character widths, because those two disagree and the disagreement is not
/// small. `UnicodeWidthStr` applies the emoji-sequence rules and
/// `UnicodeWidthChar` does not, so `☂\u{FE0F}` is two columns as a string and
/// one as a sum, and a family emoji joined by zero-width joiners is two as a
/// string and six as a sum. Summing overran the budget by a factor of two on
/// the first — the exact clipped row this function exists to prevent — and
/// threw away two columns of a name it had been given on the second.
///
/// Quadratic in the length of `s`, which is bounded: command lines are cut to
/// `CMD_MAX` before they ever reach here, and a mount name is shorter still.
/// Measuring the thing that will be drawn is worth more than an incremental
/// count that can be wrong about it.
fn take_cols(s: &str, w: usize, from_end: bool) -> String {
    if cols(s) <= w {
        return s.to_string();
    }
    if from_end {
        // The longest suffix that fits.
        let mut start = s.len();
        for (i, _) in s.char_indices().rev() {
            if cols(&s[i..]) > w {
                break;
            }
            start = i;
        }
        // A zero-width mark at the front of the suffix belongs to the character
        // that was dropped. Kept, it renders on the elision mark instead — and
        // a variation selector there makes the `…` itself take emoji
        // presentation and two columns, which is the budget overrun arriving by
        // the back door.
        let mut out = &s[start..];
        while let Some(c) = out.chars().next().filter(|c| col_width(*c) == 0) {
            out = &out[c.len_utf8()..];
        }
        out.to_string()
    } else {
        let mut end = 0;
        for (i, c) in s.char_indices() {
            let next = i + c.len_utf8();
            if cols(&s[..next]) > w {
                break;
            }
            end = next;
        }
        s[..end].to_string()
    }
}

/// A numeric cell, right-aligned.
///
/// Scanning a column for the largest value is the commonest thing anyone does
/// with this table, and right alignment is what makes magnitude a *visual*
/// property: digits line up, longer numbers stick out to the left, and the
/// outlier is found without reading. Left-aligned, `103.4`, `21.3` and `6.1`
/// share no decimal point and `6.5G`, `62.8M` and `5.4M` share no unit
/// position, so comparing two rows means parsing both.
///
/// Byte figures get it for free: the unit is the last character, so aligning
/// the right edge aligns `G` under `M`.
///
/// The header has always done this — `format!("{:>5.1}%", …)` — so this is the
/// two panels agreeing rather than a new convention.
fn num<'a>(s: impl Into<std::borrow::Cow<'a, str>>) -> Cell<'a> {
    Cell::from(Line::from(Span::raw(s)).alignment(Alignment::Right))
}

// The selected row, plus anything spliced *below* it that belongs to it.
//
// The offset pins the selected row to the bottom visible line, so thread rows —
// which are inserted immediately after their process — all landed off-screen
// the moment the process list was longer than the panel. The feature worked
// only on a table that fitted on one screen, which is every fixture and no real
// machine.
fn last_row_to_keep(rows: &[crate::tree::TreeRow<'_>], selected: usize) -> usize {
    let mut last = selected;
    while rows
        .get(last + 1)
        .is_some_and(crate::tree::TreeRow::is_thread)
    {
        last += 1;
    }
    last
}

/// The cgroup table.
///
/// A view rather than a set of columns, because it is a table of different
/// things — atop makes the same call with `G`. Sorted by pressure, deepest
/// first: the reason to open it is to find what is stalled, and the figure that
/// answers that is the one nothing else on screen can give.
fn draw_cgroups(f: &mut Frame, area: Rect, app: &App) {
    let rows_data: &[crate::sample::CgroupStat] = app
        .history
        .current()
        .and_then(|s| s.cgroups.as_deref())
        .unwrap_or(&[]);

    let psi = |p: Option<crate::sample::Pressure>, pick: fn(&crate::sample::Pressure) -> f32| {
        match p {
            // The `—` is the point: a node whose `cgroup.pressure` is switched
            // off is not a node that never stalls.
            None => num("—"),
            Some(p) => {
                let v = pick(&p);
                num(format!("{v:.1}")).style(app.theme.heat_style(v))
            }
        }
    };

    let rows: Vec<Row> = rows_data
        .iter()
        .take(area.height.saturating_sub(2) as usize)
        .map(|c| {
            Row::new(vec![
                match c.cpu {
                    Some(v) => num(format!("{v:.1}")).style(app.theme.heat_style(v)),
                    None => num("—").style(app.theme.dim_style()),
                },
                num(match c.cpu_max {
                    Some(v) => format!("{v:.0}"),
                    // Unlimited, which is not the same as a limit of zero.
                    None => "∞".into(),
                }),
                num(c.mem.map_or("—".to_string(), fmt_bytes)),
                num(c.read.map_or("—".to_string(), fmt_bytes)),
                num(c.write.map_or("—".to_string(), fmt_bytes)),
                psi(c.pressure, |p| p.cpu.some),
                psi(c.pressure, |p| p.io.some),
                psi(c.pressure, |p| p.memory.some),
                num(c.procs.map_or("—".to_string(), |n| n.to_string())),
                Cell::from(Line::from(vec![
                    Span::styled("  ".repeat(c.depth as usize), app.theme.chrome_style()),
                    Span::raw(elide_middle(short_cgroup(&c.path), 48)),
                ])),
            ])
        })
        .collect();

    let right = |s: &str| num(s.to_string()).style(app.theme.table_header_style());
    let header = Row::new(vec![
        right("CPU%"),
        right("MAX%"),
        right("MEM"),
        right("READ"),
        right("WRITE"),
        right("PSI CPU"),
        right("PSI IO"),
        right("PSI MEM"),
        right("PROCS"),
        Cell::from("CGROUP".to_string()).style(app.theme.table_header_style()),
    ]);
    let widths = [
        Constraint::Length(6),
        Constraint::Length(5),
        Constraint::Length(8),
        Constraint::Length(8),
        Constraint::Length(8),
        Constraint::Length(8),
        Constraint::Length(7),
        Constraint::Length(8),
        Constraint::Length(6),
        Constraint::Min(20),
    ];
    let title = match app.history.current().map(|s| s.cgroups.is_some()) {
        // The distinction this codebase never collapses: a machine with no
        // unified hierarchy, versus one with no cgroups.
        Some(false) | None => " cgroups — not collected here".to_string(),
        // A truncated tree says so. Reaching the cap and reporting the count
        // as if it were the whole hierarchy is the one thing this panel must
        // not do: a reader looking for a stalled cgroup would be looking at a
        // list that does not contain it.
        Some(true) if rows_data.len() >= crate::collect::CGROUP_MAX_NODES => format!(
            " cgroups (first {} of more) — depth {}, sorted by pressure",
            rows_data.len(),
            crate::collect::CGROUP_DEPTH
        ),
        Some(true) => format!(
            " cgroups ({}) — depth {}, sorted by pressure",
            rows_data.len(),
            crate::collect::CGROUP_DEPTH
        ),
    };
    let title = vec![Span::styled(title, app.theme.title_style())];
    f.render_widget(
        Paragraph::new(divider_of(title, area.width, &app.theme)),
        Rect { height: 1, ..area },
    );
    f.render_widget(
        Table::new(rows, widths).header(header),
        Rect {
            y: area.y + 1,
            height: area.height.saturating_sub(1),
            ..area
        },
    );
}

/// The part of a cgroup path worth showing.
///
/// A Kubernetes path is `/kubepods.slice/kubepods-burstable.slice/…-pod<uuid>
/// .slice/cri-containerd-<64 hex>.scope`, and the indent already carries the
/// ancestry — so the row shows the last component, which is the only part that
/// differs between siblings.
fn short_cgroup(path: &str) -> &str {
    if path == "/" {
        return path;
    }
    path.rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or(path)
}

/// What the rows on screen add up to.
///
/// About the *shown* processes, not the machine: the header above already says
/// what the machine is doing, and the question this answers is the one the
/// filter just asked. Four postgres processes using 142% of a core between them
/// is a fact nothing else on screen states, and it changes with every filter,
/// every grouping and every kernel-thread toggle.
pub struct Totals {
    pub procs: usize,
    pub cpu: f32,
    pub rss: u64,
    /// `None` where any row would not say, rather than a sum that quietly
    /// leaves some out — the same rule the thread column follows.
    pub threads: Option<u64>,
    /// How many of the rows are folded groups, if any are.
    pub groups: usize,
    /// Bytes a second read and written, where the rows say.
    pub disk: Option<(u64, u64)>,
}

/// Add up what is on screen.
///
/// Counted the same way the scope line counts, so the two cannot disagree about
/// how many processes are being described: thread rows are skipped, and a
/// folded row stands for everything folded into it.
pub fn totals(app: &App) -> Totals {
    let rows = app.visible_rows();
    let mut out = Totals {
        procs: 0,
        cpu: 0.0,
        rss: 0,
        threads: Some(0),
        groups: 0,
        disk: Some((0, 0)),
    };
    for r in rows.iter().filter(|r| !r.is_thread()) {
        out.procs += r.count();
        out.cpu += r.proc.cpu;
        out.rss += r.proc.rss;
        out.groups += usize::from(r.members.is_some());
        out.threads = match (out.threads, r.proc.threads) {
            (Some(n), Some(t)) => Some(n + u64::from(t)),
            _ => None,
        };
        out.disk = match (out.disk, r.proc.io) {
            (Some((r0, w0)), Some(io)) => Some((r0 + io.read, w0 + io.write)),
            _ => None,
        };
    }
    out
}

/// The summary strip, drawn between the panel title and the column headers.
fn summary_line(app: &App, total_mem: u64, width: usize) -> Option<Line<'static>> {
    let t = totals(app);
    if t.procs == 0 {
        return None;
    }
    let dim = app.theme.dim_style();
    let share = if total_mem > 0 {
        format!(" ({:.0}%)", t.rss as f64 / total_mem as f64 * 100.0)
    } else {
        String::new()
    };
    // Its own ladder, given up from the least diagnostic end. The count goes
    // last because the scope line already says it — this row is here for the
    // magnitudes, which nothing else states.
    // Led by whatever the tab is about. The strip is inside the table panel and
    // describes the rows in it, so on the Memory tab the first figure after the
    // count should be memory — a tab that changes the columns and leaves the
    // summary reading the same way has only half-changed the question.
    let cpu = format!("CPU {:.1}%", t.cpu);
    let mem = format!("MEM {}{share}", fmt_bytes(t.rss));
    let disk = t
        .disk
        .map(|(r, w)| format!("DISK {} · {}", fmt_rate(r).trim(), fmt_rate(w).trim()));
    let lead: Vec<String> = match app.view {
        crate::app::View::Cpu => vec![cpu.clone(), mem.clone()],
        crate::app::View::Memory => vec![mem.clone(), cpu.clone()],
        crate::app::View::Disk => match &disk {
            Some(d) => vec![d.clone(), cpu.clone()],
            None => vec![cpu.clone(), mem.clone()],
        },
    };
    // Dropped rather than dashed when the platform will not say. An em dash
    // here is a clause that says nothing on every frame — and on macOS, where
    // a process poptop cannot open reports no thread count, that is most of
    // them.
    let mut rungs = Vec::new();
    if let Some(thr) = t.threads {
        rungs.push(format!(
            " {} shown · {} · {thr} threads",
            t.procs,
            lead.join(" · ")
        ));
    }
    rungs.extend([
        format!(" {} shown · {}", t.procs, lead.join(" · ")),
        format!(" {}", lead.join(" · ")),
        format!(" {}", lead[0]),
    ]);
    let text = rungs.into_iter().find(|r| cols(r) <= width)?;
    let mut spans = vec![Span::styled(text, dim)];
    // Only when something is folded, because otherwise it is a fact about
    // nothing: an ungrouped table has as many rows as processes and saying so
    // is noise.
    if t.groups > 0 {
        let note = format!("  ({} groups)", t.groups);
        if cols(&note) + spans.iter().map(|s| cols(&s.content)).sum::<usize>() <= width {
            spans.push(Span::styled(note, dim));
        }
    }
    Some(Line::from(spans))
}

/// The part of a panel its content is drawn in.
///
/// Every row that is not a full-width divider starts here, which is the whole
/// of what `Density::margin` buys: a left edge you can run your eye down.
pub fn content(app: &App, area: Rect) -> Rect {
    let m = app.density.margin(area.width);
    Rect {
        x: area.x + m,
        width: area.width.saturating_sub(m * 2),
        ..area
    }
}

/// Where the table's rows and headers are drawn, inside the panel.
///
/// One derivation, because the mouse resolves a click through the same
/// arithmetic — and a hit box an inset away from the column it is over is the
/// bug this milestone has already had twice.
pub fn table_body(app: &App, area: Rect) -> Rect {
    Rect {
        y: area.y + 1 + summary_height(area),
        height: area.height.saturating_sub(1 + summary_height(area)),
        ..content(app, area)
    }
}

/// Whether the summary strip is drawn, for a table panel of this height.
///
/// Given up before the table drops below its floor, the same way the tab strip
/// is: a summary of rows you cannot see is worth less than the rows.
pub fn summary_height(table: Rect) -> u16 {
    u16::from(table.height > PROCS_FLOOR_H + 3)
}

/// The row the column headers are drawn on.
///
/// One derivation, because the mouse needs it to know a header was clicked and
/// the table needs it to draw them — and an off-by-one between those two is a
/// click that sorts by the column above the one under the pointer.
pub fn table_header_y(table: Rect) -> u16 {
    table.y + 1 + summary_height(table)
}

/// Which of the table's optional columns are on, for a table drawn in `area`.
///
/// One derivation, used by `draw_procs` to lay the table out and by the mouse
/// to work out which header was clicked. It was briefly two, and the second one
/// guessed `show_user` — so a click on `COMMAND` landed on the column before it
/// and sorted by something else. A hit box computed separately from the column
/// it is over agrees until it does not.
#[derive(Debug)]
pub struct TableShape {
    pub bars: bool,
    pub thr: bool,
    pub io: bool,
    /// The memory tab's own columns, each on only where the platform has the
    /// figure to put under it.
    ///
    /// One flag each rather than one for the set, because the set is never
    /// whole: macOS publishes none of the three the kernel is asked for, and
    /// Linux publishes proportional memory only to a process allowed to read
    /// another's `smaps_rollup`. Thirty-one columns of em dash is the same
    /// waste `App::one_user` exists to stop, and here it was crowding out the
    /// RSS figure beside it.
    pub pss: bool,
    pub vsize: bool,
    pub majflt: bool,
    pub grow: bool,
    /// The per-process history. The thing no other monitor draws, and so the
    /// last picture given up — but it is a picture, and a figure beside it
    /// that has been truncated to keep it is a worse trade than losing it.
    /// The resident figure, the state letter and the pid.
    ///
    /// Droppable, and last of all, because below about sixty columns the
    /// alternative is worse than losing them: every column here is a fixed
    /// `Length`, ratatui squeezes a set that does not fit rather than dropping
    /// any, and a squeezed right-aligned figure loses its *leading* digits —
    /// `100.9` renders as `.9` (0105). A column that is not there says nothing;
    /// a column that is there and wrong says something false.
    pub rss: bool,
    pub state: bool,
    pub pid: bool,
    pub spark: bool,
    /// Why `spark` is off, when the reason is the data rather than the width.
    ///
    /// The column is dropped by two different rules and the title says so only
    /// for one of them, so the shape carries which — recomputing the movement
    /// test beside the title would be a second fold over the buffer every
    /// frame, and a second chance for the two answers to disagree.
    pub flat: bool,
    pub user: bool,
    pub cid: bool,
}

/// What a shape's columns add up to, gaps and a readable command included.
///
/// Derived from [`table_columns`] rather than re-added by hand, for the reason
/// that function exists: a second copy of this arithmetic agrees until it does
/// not, and the way it fails here is silent.
#[cfg(test)]
pub fn table_request_for_test(shape: &TableShape, gap: u16) -> u16 {
    table_request(shape, gap)
}

fn table_request(shape: &TableShape, gap: u16) -> u16 {
    let (widths, _) = table_columns(shape);
    let fixed: u16 = widths
        .iter()
        .map(|c| match c {
            Constraint::Length(w) | Constraint::Min(w) => *w,
            _ => 0,
        })
        .sum();
    fixed + gap * (widths.len() as u16).saturating_sub(1)
}

pub fn table_shape(app: &App, area: Rect) -> TableShape {
    // The width the columns actually get, which is the panel less the air
    // either side of them. Using the panel's own width made every column
    // decision two columns too generous, and the command was elided to a width
    // it was then chopped at.
    let area = Rect {
        width: table_body(app, area).width,
        ..area
    };
    let one_user = app.one_user();
    let user = one_user.is_none() && app.group != crate::app::Grouping::User;
    let bars = app.view != crate::app::View::Disk;
    let thr = app.view == crate::app::View::Cpu;
    let mem_cols = app.view == crate::app::View::Memory;
    let io = app.show_io
        && app.view.wants_io()
        && (app.view == crate::app::View::Disk || area.width >= min_width_for_io(user));
    // A column nobody can fill is a column of em dashes. The platform decides
    // three of these four: macOS publishes none of them, and on Linux
    // proportional memory needs permission to read another process's
    // `smaps_rollup`. Growth is poptop's own arithmetic over two samples and is
    // always available, so the memory tab always has something on it.
    let has = app.mem_columns_available();
    // The history column earns its width by showing change. The CPU% beside it
    // already says how busy each process is; what only the sparkline can say is
    // how that moved. When no row on screen moved — every line flat, which on a
    // quiet machine is every line — it was ten columns repeating the CPU column
    // as a picture, while the command was elided for want of room (0110). So it
    // is drawn when some row's history moves, on the one shared scale, and
    // otherwise gives its width back and says why.
    //
    // Not a log scale, which was tried: four levels cannot be both fine at the
    // bottom and readable at the top, and the top is where a process pinning
    // several cores lives.
    //
    // Not judged until there is history to judge: before `App::CONSTANT_FOR`
    // samples nothing has had time to move, and a column that appeared a few
    // seconds after start would move the layout under the reader for no reason.
    // The same threshold the user column folds on.
    let flat = app.history.len() >= crate::app::App::CONSTANT_FOR
        && !any_history_moves(app, buffer_ceiling(app));
    let mut shape = TableShape {
        bars,
        thr,
        io,
        pss: mem_cols && has.pss,
        vsize: mem_cols && has.vsize,
        majflt: mem_cols && has.majflt,
        grow: mem_cols,
        rss: true,
        state: true,
        pid: true,
        spark: !flat,
        flat,
        user,
        cid: false,
    };

    // What the reader asked not to see, before the width ladder runs: the room
    // a hidden column would have taken goes to the command rather than to
    // whatever the ladder would have dropped next.
    //
    // Here rather than where the cells are drawn, because this function is the
    // one derivation both the table and the mouse ask — a column hidden in one
    // and not the other puts the caret over the wrong header.
    for hidden in &app.hidden_columns {
        use crate::app::Column;
        match hidden {
            Column::Bars => shape.bars = false,
            Column::Rss => shape.rss = false,
            Column::State => shape.state = false,
            Column::Thr => shape.thr = false,
            Column::Io => shape.io = false,
            // One name for the tab's own set: a reader hiding `mem` is asking
            // for the memory columns gone, not for four names to learn.
            Column::Mem => {
                shape.pss = false;
                shape.vsize = false;
                shape.majflt = false;
                shape.grow = false;
            }
            Column::Hist => shape.spark = false,
            Column::Pid => shape.pid = false,
            Column::User => shape.user = false,
            Column::Cid => shape.cid = false,
        }
    }

    // Drop columns until the rest fit, least identifying first.
    //
    // Every column but the command is a fixed `Length`, and ratatui squeezes a
    // set of fixed lengths that does not fit rather than dropping any. A
    // right-aligned figure squeezed by two columns keeps its tail: `301.7M`
    // renders as `01.7M`, which is not a narrower number but a wrong one, and
    // nothing on screen says so. `min_width_for_io` was this argument applied
    // to the disk columns alone; below seventy-six columns the same thing was
    // happening to CPU% and RSS, and on the memory tab it started at eighty.
    //
    // The order is what each column costs against what it says. The bars
    // restate the figure beside them; the thread count and the owner are
    // usually implied by the command; reserved address space says least of the
    // four memory figures and proportional memory says most. A view's own
    // columns go late, because without them it is not that view any more — but
    // they do go: the disk tab exists to show figures the width test would
    // otherwise hide, not to show them wrong.
    // Below the view's own columns come the three that every view has had
    // since the first version — the resident figure, the state letter and the
    // pid. They go last and they do go: at twenty columns the alternative is
    // not a narrower table but a wrong one, with `100.9` drawn as `.9`.
    // What survives to the bottom is CPU% and the name, which is the least a
    // row can be and still be about a process.
    let gap = app.density.column_gap();
    for step in 0..12 {
        if table_request(&shape, gap) <= area.width {
            break;
        }
        match step {
            0 => shape.bars = false,
            1 => shape.thr = false,
            2 => shape.vsize = false,
            3 => shape.majflt = false,
            4 => shape.user = false,
            5 => shape.grow = false,
            6 => shape.pss = false,
            7 => shape.spark = false,
            8 => shape.io = false,
            9 => shape.state = false,
            10 => shape.pid = false,
            _ => shape.rss = false,
        }
    }

    shape.cid = app.group != crate::app::Grouping::User
        && app.any_container()
        && command_width(&shape, area.width, gap) as u16 > MIN_COMMAND_W + CID_W + gap;
    shape
}

/// The sort key of the column at `x`, for a table drawn in `area`.
///
/// Uses the same list the header and the table do, split the same way: a hit
/// box computed separately from the column it is over agrees until it does not.
///
/// The widths depend on what the tab is showing, which is what `table_shape`
/// answers — one derivation, so the caret and the click cannot disagree.
pub fn sort_at(app: &App, area: Rect, x: u16) -> Option<crate::app::Sort> {
    let s = table_shape(app, area);
    let (widths, sorts) = table_columns(&s);
    let cells = Layout::horizontal(widths)
        .spacing(app.density.column_gap())
        .split(table_body(app, area));
    cells
        .iter()
        .position(|r| x >= r.x && x < r.x + r.width)
        .and_then(|i| sorts.get(i).and_then(|c| c.sort))
}

/// One of the table's columns.
pub struct Column {
    /// The ordering this column stands for, or `None` if it is not one.
    pub sort: Option<crate::app::Sort>,
    /// Figures right, text left. Bars and the sparkline are neither.
    pub numeric: bool,
}

/// The table's columns: how wide each is, and which sort key it stands for.
///
/// One list, used by the header to mark the sorted column, by the table to lay
/// itself out, and by the mouse to work out which header was clicked. Three
/// copies of this arithmetic would put the caret over one column and the click
/// target over another, and nothing would say so.
///
/// `None` is a column nothing can be sorted by — a bar, a state letter, the
/// sparkline. Clicking one does nothing rather than doing something arbitrary.
pub fn table_columns(s: &TableShape) -> (Vec<Constraint>, Vec<Column>) {
    use crate::app::Sort;
    let (show_bars, show_thr, show_io) = (s.bars, s.thr, s.io);
    let (show_user, show_cid) = (s.user, s.cid);
    let mut widths = Vec::new();
    let mut sorts = Vec::new();
    // `numeric` is the alignment rule written down: figures right, text left,
    // and the bars and the sparkline are pictures rather than either. It was a
    // convention followed by hand in eleven places and checked nowhere.
    let mut col = |w: Constraint, sort: Option<Sort>, numeric: bool| {
        widths.push(w);
        sorts.push(Column { sort, numeric });
    };
    col(Constraint::Length(6), Some(Sort::Cpu), true);
    if show_bars {
        // The bar, plus room for the over-100 mark.
        // The bar is the same key as the figure beside it, and `None` here
        // because the caret belongs on the label, not on both.
        col(Constraint::Length(BAR_W as u16 + 1), None, false);
    }
    if s.rss {
        col(Constraint::Length(8), Some(Sort::Mem), true);
        if show_bars {
            col(Constraint::Length(BAR_W as u16), None, false);
        }
    }
    if s.state {
        col(Constraint::Length(2), None, false);
    }
    if show_thr {
        col(Constraint::Length(4), None, true);
    }
    if show_io {
        // The pair is ordered by read *plus* write. The caret goes on the
        // first of them, which reads as "sorted from here" rather than as a
        // claim about that column alone.
        col(Constraint::Length(9), Some(Sort::Disk), true);
        col(Constraint::Length(9), None, true);
    }
    for (on, w) in [(s.pss, 8), (s.vsize, 8), (s.majflt, 7), (s.grow, 8)] {
        if on {
            col(Constraint::Length(w), None, true);
        }
    }
    if s.spark {
        col(Constraint::Length(SPARK_W as u16), None, false);
    }
    if s.pid {
        col(Constraint::Length(7), Some(Sort::Pid), true);
    }
    if show_user {
        col(Constraint::Length(USER_W), None, false);
    }
    if show_cid {
        // Twelve characters, which is what `docker ps` shows.
        col(Constraint::Length(CID_W), None, false);
    }
    col(Constraint::Min(MIN_COMMAND_W), Some(Sort::Name), false);
    (widths, sorts)
}

/// How long the table's figures are averaged over, in the units it was asked in.
fn fmt_smooth(samples: usize, interval: Duration) -> String {
    let secs = samples as f64 * interval.as_secs_f64();
    if secs >= 1.0 {
        format!("{secs:.0}s")
    } else {
        format!("{:.0}ms", secs * 1000.0)
    }
}

/// `strip` is whether the tab strip is on screen. When it is not — a terminal
/// too short to spend a row on it — the settings it names come back here, at
/// the rank the sort clause used to have. A setting that is stated nowhere is
/// a table whose ordering has no visible reason, and the row this panel would
/// save by staying quiet is not worth that.
fn draw_procs(f: &mut Frame, area: Rect, app: &App, timeline: Rect, strip: bool) {
    // Dropped on a panel too narrow to carry them, like every other element
    // here. Collection is untouched: the columns are a rendering decision and
    // the ratchet is a history one, so widening the window brings them back
    // with their history intact.
    // A column whose every value is the same is telling you one fact, and a
    // fact belongs in a sentence. See `App::one_user`.
    let one_user = app.one_user();
    // Folded away when it is the *identity* as well as a column: grouping by
    // user puts the name in the command column, and a `USER` column beside it
    // would be the same word twice. That inverts 0043's rule, which drops the
    // column when every row shares a value — here every row has a different
    // one and it is still redundant.
    // From `table_shape`, not computed again here: the mouse asks that function
    // which column it clicked, and two derivations of the same six flags put
    // the caret over one column and the click target over another.
    let shape = table_shape(app, area);
    let show_user = shape.user;
    // In the disk view the throughput columns are the point, so they are not
    // subject to the width test that hides them elsewhere — which is the
    // concrete thing views fix: today those figures vanish on a narrow terminal
    // with nothing to bring them back, and this key is what brings them back.
    let show_io = shape.io;
    // What the disk columns are given room by. A view is a named list of
    // columns over one renderer, not a second renderer.
    let show_bars = shape.bars;
    let show_thr = shape.thr;
    // Dropped on a box running no containers, where it would be twelve columns
    // of nothing. The same rule as the user column, and why a process in no
    // container shows a blank rather than an em dash.
    //
    // Measured against the columns actually drawn, not against the IO columns'
    // minimum: `min_width_for_io` budgets forty-five columns for DISK R, DISK W
    // and the sparkline whether or not they are on screen, so gating on it hid
    // the column at every width a hundred-column terminal has — on exactly the
    // container host this exists for.
    // Not while grouping by user: every group row's container is `None` — its
    // members can be in different ones — so the column would be a header and
    // twelve blank columns on every row, which is the argument that drops
    // `USER` two lines up.
    let show_cid = shape.cid;
    let cmd_w = command_width(
        &shape,
        table_body(app, area).width,
        app.density.column_gap(),
    );
    // The same averaging the ordering used, so a row's figure and its position
    // are describing the same thing. Computed again rather than threaded
    // through `visible_rows`: it is a fold over a few hundred processes across
    // five samples, and the alternative is a cache invalidated by every one of
    // `History`'s six cursor movements.
    let sm = app.smoothing();
    let rows_data = app.visible_rows();
    // Memory bars are scaled against the displayed sample's total, not the
    // live one, so they stay correct while scrubbed like everything else here.
    let total_mem = app.history.current().map_or(0, |s| s.mem.total);

    // Per-process history, for the rows actually on screen.
    //
    // This is the thing no other monitor can draw. Every retained sample holds
    // its whole process list, so "what has *this* process been doing" is
    // already in the buffer — htop, btop and bottom keep no per-process
    // history, zenith's is aggregate-only, and atop has the data but replays
    // whole intervals from a logfile rather than putting a trend beside a row.
    let visible_rows = area.height.saturating_sub(2) as usize;
    // Resolved from the watched process each frame, not carried as an index.
    // When it is not on screen the viewport holds where it was rather than
    // snapping home — absence should suppress the highlight, not the scroll
    // position.
    let selected = app.row_of(&rows_data);
    let selected_row = selected.unwrap_or_else(|| app.resume_row());
    let row_offset =
        last_row_to_keep(&rows_data, selected_row).saturating_sub(visible_rows.saturating_sub(1));
    let keys: Vec<(i32, u64)> = rows_data
        .iter()
        .skip(row_offset)
        .take(visible_rows)
        .filter_map(|r| r.proc.key())
        .collect();
    // The window the timeline is showing, not the whole buffer.
    //
    // This used to be the whole buffer, on the reasoning that "what has this
    // process been doing" is a fixed question deserving a fixed answer. The
    // objection to that is stronger: the two pictures are then of different
    // spans, side by side, with nothing saying so. A spike halfway along the
    // timeline sits somewhere else entirely in the row beside it, and the
    // reader has to know which span each is drawn over before either can be
    // read against the other. Same window, same zoom, same cursor — the same
    // rule the detail view already follows.
    let (spark_start, spark_shown, _) = shown_window(app, timeline);
    let series = history::series_in(&app.history, &keys, spark_start, spark_shown);
    // Same span, harder compression. Ten cells against the timeline's hundred
    // means each one covers ten times as much, so the sparkline needs its own
    // zoom over the same samples rather than the timeline's — synchronised is
    // about the span, not the stride.
    let spark_slots = SPARK_W * app.glyphs.spark_samples_per_cell();
    let spark_zoom = spark_shown.div_ceil(spark_slots.max(1)).max(1);

    // One ceiling across every row. Scaling each sparkline to its own peak
    // makes a flat 12% process look exactly like one spiking to 90%, which
    // defeats the only reason to put them in a column together.
    //
    // Taken from the whole buffer and every process in it, not from `series`,
    // which holds only the rows on screen. Drawn from the visible slice the
    // ceiling would move as you scrolled: bring a 400% process into view and
    // every other row's history collapses to the floor, then springs back when
    // it scrolls off. That is the same objection as the comment above — the
    // answer to "what has this process been doing" must not depend on where the
    // list happens to be sitting.
    // Over the window rather than the whole buffer, now that the window is what
    // is drawn: a ceiling set by a spike that scrolled out of view flattens
    // every row still on screen.
    let spark_ceiling = glyphs::ceiling_for(
        app.history
            .iter()
            .skip(spark_start)
            .take(spark_shown)
            .flat_map(|s| s.procs.iter())
            .map(|p| p.cpu)
            .fold(0.0_f32, f32::max),
    );
    let collected = app.history.current().is_some_and(|s| s.io_collected);
    let rows_visible = area.height.saturating_sub(2) as usize;

    // Keep the selected row on screen while scrolling through a long list.
    let offset =
        last_row_to_keep(&rows_data, selected_row).saturating_sub(rows_visible.saturating_sub(1));

    // Measurements first, contiguous, scanned down the left where the eye
    // starts; the sparkline closing them; then identity — PID, USER, COMMAND —
    // together at the right edge, where the variable-width column belongs.
    //
    // The three columns that say *which process this is* used to sit at
    // opposite ends of the row with eight columns of measurement between them
    // and ten of braille immediately before the name, so reading a row meant
    // starting at the left, jumping seventy columns right to find out what it
    // was, and coming back.
    //
    // Three arrangements were rendered before choosing. The rejected two:
    //
    //   - COMMAND second, beside PID. Reads best for identification and fails
    //     on any wide terminal: COMMAND is the column that absorbs the slack,
    //     so the measurements end up against the right edge with a widening
    //     gulf in front of them.
    //   - Leave the order and move HISTORY off the boundary. Nearly free, and
    //     it does not do the job — PID and COMMAND are still seventy columns
    //     apart — and it puts a block of braille immediately after PID, which
    //     interrupts the numeric scan it was meant to protect.
    //
    // The cost is the convention that PID comes first. It is paid because the
    // order now matches what the tool is for: spot a row that is hot, then read
    // what it is, with its pid beside the name rather than seventy columns away.
    let rows: Vec<Row> = rows_data
        .iter()
        .enumerate()
        .skip(offset)
        .take(rows_visible)
        .map(|(i, r)| {
            let p = &r.proc;
            // Every other row gets a slightly lighter ground. A process table
            // is wide — a figure on the left and the name it belongs to on the
            // right, with eight columns between — and the eye loses the line it
            // is on somewhere in the middle. Striping is the oldest fix there
            // is for that, and it costs nothing a border would not cost more.
            //
            // Subtle on purpose: a stripe loud enough to notice competes with
            // the figures it is there to help you read across. See
            // `Theme::stripe_style`.
            let mut style = if i % 2 == 1 {
                app.theme.stripe_style()
            } else {
                Style::default()
            };
            if Some(i) == selected {
                style = app.theme.selection_style();
            } else if r.context_only {
                // Present only as an ancestor of a filter match: visible for
                // parentage, but clearly not itself a hit.
                style = style.add_modifier(Modifier::DIM);
            }
            // A thread row. Every column a thread does not have its own answer
            // for is an em dash rather than the process's value: memory, disk
            // and thread count belong to the process, and repeating them on
            // each of forty rows would say the process's RSS forty times and
            // imply forty copies of it.
            if let Some(th) = &r.thread {
                let mut cells =
                    vec![num(format!("{:.1}", th.cpu)).style(app.theme.heat_style(th.cpu))];
                if show_bars {
                    cells.push(Cell::from(cpu_bar(th.cpu)).style(app.theme.dim_style()));
                }
                if shape.rss {
                    cells.push(num("—").style(app.theme.dim_style()));
                    if show_bars {
                        cells.push(Cell::from(""));
                    }
                }
                if shape.state {
                    cells.push(Cell::from(th.state.to_string()));
                }
                if show_thr {
                    cells.push(num("—").style(app.theme.dim_style()));
                }
                if show_io {
                    cells.push(num("—").style(app.theme.dim_style()));
                    cells.push(num("—").style(app.theme.dim_style()));
                }
                // A thread has no memory of its own; it shares its process's,
                // one row up. One dash per column actually drawn — a fixed four
                // put the sparkline under `GROW` the moment a column was
                // dropped for want of anything to put in it.
                for _ in 0..[shape.pss, shape.vsize, shape.majflt, shape.grow]
                    .iter()
                    .filter(|on| **on)
                    .count()
                {
                    cells.push(num("—").style(app.theme.dim_style()));
                }
                // No sparkline. The retained history is per process, so the
                // only series available here is the parent's — drawing it on
                // every thread row would put the same shape beside forty
                // different numbers and invite reading it as each one's. Blank
                // rather than absent, because the column is still there.
                if shape.spark {
                    cells.push(Cell::from(""));
                }
                if shape.pid {
                    cells.push(num(th.tid.to_string()));
                }
                if show_user {
                    // The process's, one row up. A thread does not have its
                    // own.
                    cells.push(Cell::from(""));
                }
                let (prefix, room) = fit_prefix(&r.prefix, cmd_w);
                cells.push(Cell::from(Line::from(vec![
                    Span::styled(prefix, app.theme.chrome_style()),
                    Span::raw(elide_middle(&th.name, room)),
                ])));
                return Row::new(cells).style(style);
            }
            // Withheld, not zero: see `ProcSample::unmeasured`.
            let unmeasured = p.unmeasured();
            let mut cells = vec![if unmeasured {
                num("—").style(app.theme.dim_style())
            } else {
                num(format!("{:.1}", sm.cpu(p))).style(app.theme.heat_style(sm.cpu(p)))
            }];
            // A bar beside the number turns a column that must be read into
            // one that can be scanned. htop does the same, for the same reason.
            //
            // Both bars are neutral. Length already carries the magnitude, and
            // the number beside each one already carries its status colour —
            // colouring the bar too would spend a third channel on the same
            // fact. Using a series hue here was worse still: that is an
            // identity token, and a share of memory is not an identity. The C6
            // test caught it. An unmeasured process has no bar: a bar of
            // nothing is a measurement of nothing.
            if show_bars {
                cells.push(if unmeasured {
                    Cell::from("")
                } else {
                    Cell::from(cpu_bar(sm.cpu(p))).style(app.theme.dim_style())
                });
            }
            if shape.rss {
                cells.push(if unmeasured {
                    num("—").style(app.theme.dim_style())
                } else {
                    num(fmt_bytes(sm.rss(p)))
                });
                if show_bars {
                    cells.push(if unmeasured {
                        Cell::from("")
                    } else {
                        Cell::from(glyphs::micro_bar(mem_frac(sm.rss(p), total_mem), BAR_W))
                            .style(app.theme.dim_style())
                    });
                }
            }
            if shape.state {
                cells.push(Cell::from(p.state.to_string()));
            }
            if show_thr {
                // An em dash, never a number we do not have. See
                // `ProcSample::threads`: a fabricated `1` sits next to a CPU
                // percentage that can openly contradict it.
                cells.push(num(match p.threads {
                    Some(n) => n.to_string(),
                    None => "—".into(),
                }));
            }
            if show_io {
                cells.push(io_cell(collected, p.io, false, &app.theme));
                cells.push(io_cell(collected, p.io, true, &app.theme));
            }
            // Never a zero for any of these: a share nobody measured, a size
            // the platform does not publish and a fault count that was not
            // collected are all "not known", and this table has one way of
            // saying that. The column is there at all only where somebody
            // answers — see `App::mem_columns_available`.
            if shape.pss {
                cells.push(num(match p.pss {
                    Some(b) => fmt_bytes(b),
                    None => "—".into(),
                }));
            }
            if shape.vsize {
                cells.push(num(match p.vsize {
                    Some(b) => fmt_bytes(b),
                    None => "—".into(),
                }));
            }
            if shape.majflt {
                cells.push(match p.majflt {
                    // Coloured against a *fault* threshold, not through
                    // `heat_style`: that compares against the warn and critical
                    // *percentages*, so a process taking five faults a second —
                    // a real symptom — rendered as dim nothing, and anything
                    // over eighty pinned to critical. A rate per second is not
                    // a percentage of anything.
                    Some(n) if n >= MAJFLT_WARN => {
                        num(n.to_string()).style(app.theme.warning_style())
                    }
                    Some(n) => num(n.to_string()),
                    None => num("—").style(app.theme.dim_style()),
                });
            }
            if shape.grow {
                // Not for a group. Its synthesised pid is the lowest member's
                // and its `started` is `None`, so on a platform that also
                // reports `None` there the lookup matches that one member and
                // draws its delta beside group-summed memory as if it were the
                // group's. Every other grouped figure either sums or collapses
                // to an em dash; so does this.
                let grew = (!r.is_group())
                    .then(|| app.growth(p.pid, p.started))
                    .flatten();
                cells.push(match grew {
                    Some(d) => num(fmt_growth(d)),
                    None => num("—").style(app.theme.dim_style()),
                });
            }
            // The sparkline closes the measurements, so the eye can run down a
            // column of shapes rather than hunting for it past ragged names —
            // and it makes the boundary between what a row *measures* and what
            // a row *is*.
            if shape.spark {
                cells.push(
                    Cell::from(sparkline(
                        p.key().and_then(|k| series.get(&k)).map(Vec::as_slice),
                        app.glyphs,
                        spark_zoom,
                        spark_ceiling,
                    ))
                    .style(app.theme.dim_style()),
                );
            }
            // Identity, all of it together — see the note above `rows`.
            // A group has no pid — it is not a process. The column carries how
            // many were folded in instead, which is the fact that replaces it.
            // A group of one keeps the pid: there is a single process there and
            // `×1` says less than its number does.
            if shape.pid {
                cells.push(num(match r.members {
                    Some(n) if n > 1 => format!("×{n}"),
                    _ => p.pid.to_string(),
                }));
            }
            // Dropped, not blanked: an empty cell still occupies its ten
            // columns, and giving them to `COMMAND` is the whole point.
            if show_user {
                cells.push(Cell::from(p.user.to_string()));
            }
            if show_cid {
                cells.push(match &p.container {
                    Some(c) => Cell::from(c.to_string()),
                    // Not in one. An em dash would say poptop could not tell.
                    None => Cell::from(""),
                });
            }
            // The spine is structural, not data: it takes the chrome token so
            // it recedes the way a gridline should, while the name stays at
            // full contrast.
            // Elided here rather than clipped by the terminal, so the part
            // that identifies the process survives — see `elide_middle`.
            //
            // The indent gives way before the name does. A tree prefix grows
            // three columns per level, so at nineteen columns a chain nine deep
            // left the name nothing at all and the row said only `└`. Losing a
            // level of visible nesting is a smaller loss than losing which
            // process the row is about.
            let (prefix, room) = fit_prefix(&r.prefix, cmd_w);
            cells.push(Cell::from(Line::from(vec![
                Span::styled(prefix, app.theme.chrome_style()),
                // A group shows the name its members share. Their command
                // lines are what differ — that is why they are separate
                // processes — so there is no one command line to show.
                Span::raw(elide_middle(p.command(), room)),
            ])));
            Row::new(cells).style(style)
        })
        .collect();

    // In the order the cells are pushed, which is what `Table` pairs them by.
    // With the IO columns shown these had drifted a place: `HISTORY` sat over
    // DISK R, `DISK R` over DISK W, and `DISK W` over the sparkline — every one
    // of the three naming the column beside it.
    // The caret goes in the header of the column the ordering is over, which is
    // where the reader is already looking. It used to be stated in the panel
    // title several rows away, in a clause the width ladder can drop — so the
    // ordering was named furthest from the thing it ordered.
    //
    // Always descending, because "what is using the most" is the question. The
    // caret says *which* column, not which direction.
    // One walk of the column list, giving each header both its caret and its
    // alignment. The alignment used to be chosen by hand at each of fourteen
    // push sites, so a column could be declared numeric and drawn left with
    // nothing to say the two had parted company.
    let (_, cols) = table_columns(&shape);
    let mut nth = 0usize;
    let mut head = move |label: &str| {
        let col = cols.get(nth);
        nth += 1;
        let mark = if col.and_then(|c| c.sort) == Some(app.sort) {
            "▾"
        } else {
            ""
        };
        // Prefixed on a right-aligned header and suffixed on a left-aligned
        // one, so the caret sits in the padding the column already has.
        // Appending it to a right-aligned label pushes the label two columns
        // left and the header stops sharing a right edge with the figures under
        // it — `a_column_of_figures_shares_a_right_edge` is about exactly that.
        if col.is_some_and(|c| c.numeric) {
            num(format!("{mark}{label}")).style(app.theme.table_header_style())
        } else {
            Cell::from(format!("{label}{mark}")).style(app.theme.table_header_style())
        }
    };
    let mut header_cells = vec![head("CPU%")];
    if show_bars {
        header_cells.push(head(""));
    }
    if shape.rss {
        header_cells.push(head("RSS"));
        if show_bars {
            header_cells.push(head(""));
        }
    }
    if shape.state {
        header_cells.push(head("S"));
    }
    if show_thr {
        header_cells.push(head("THR"));
    }
    if show_io {
        header_cells.push(head("DISK R"));
        header_cells.push(head("DISK W"));
    }
    for (on, label) in [
        (shape.pss, "PSS"),
        (shape.vsize, "VSZ"),
        (shape.majflt, "MAJF/s"),
        (shape.grow, "GROW"),
    ] {
        if on {
            header_cells.push(head(label));
        }
    }
    if shape.spark {
        header_cells.push(head(&spark_header(spark_ceiling)));
    }
    if shape.pid {
        header_cells.push(head("PID"));
    }
    if show_user {
        header_cells.push(head("USER"));
    }
    if show_cid {
        header_cells.push(head("CID"));
    }
    header_cells.push(head("COMMAND"));
    let header = Row::new(header_cells).style(app.theme.table_header_style());

    // What the table cannot show, said out loud. A process that lived 200ms is
    // not in this list, and a burst of them is one of the commonest causes of
    // the spike you scrubbed back to find — so without this the table sits
    // under a graph it cannot explain and says nothing about why.
    //
    // Suppressed across a sampling gap. The two samples either side of a sleep
    // can be hours apart, and the counter would attribute a whole night's task
    // creation to the one second the table is describing — `1204331 came and
    // went` beside a table of one instant. The timeline already draws a seam
    // there; this is the same event, and it should not be summed through.
    //
    // Said in tasks rather than processes because that is what it counts: a
    // thread pool recycling workers advances the same counter, and calling
    // those processes would invent an event.
    let churn = app
        .history
        .previous()
        .zip(app.history.current())
        .filter(|(prev, now)| {
            now.at
                .duration_since(prev.at)
                .is_ok_and(|d| d < history::gap_limit(app.interval))
        })
        .and_then(|(prev, now)| history::churn(prev, now))
        .filter(|c| c.unseen() > 0)
        .map_or(String::new(), |c| {
            format!(" · {} tasks came and went", c.unseen())
        });

    // An event rather than a level, so it sits beside the other event this
    // panel reports. The sharpest figure poptop has: a process the OOM killer
    // ended is gone from the next sample with nothing anywhere saying why, and
    // scrubbing back to this moment shows the table from the instant before.
    // Behind the same gap guard as the churn above, and for the reason stated
    // there. This is a raw since-boot delta, not a rate: the paging figures
    // divide by elapsed time and so survive a sleep, but a *count* does not —
    // the first sample after a laptop suspend carries every kill from the whole
    // night and the panel would attribute them to the one second the table is
    // describing.
    // A sample whose predecessor is not in the buffer is not across a gap —
    // the same convention the seam detector states, that index zero has no
    // predecessor here and inventing one would put a seam at the left edge of
    // every fresh buffer. The collector's delta was taken against a real
    // previous collection either way.
    let gapped = app
        .history
        .previous()
        .zip(app.history.current())
        .is_some_and(|(prev, now)| {
            now.at
                .duration_since(prev.at)
                .is_ok_and(|d| d >= history::gap_limit(app.interval))
        });
    let killed = app
        .history
        .current()
        .filter(|_| !gapped)
        .and_then(|s| s.oom_kills)
        .filter(|n| *n > 0)
        .map_or(String::new(), |n| match n {
            1 => " · 1 process killed for memory".to_string(),
            n => format!(" · {n} processes killed for memory"),
        });

    // Never silently shorter than the count beside it. Placed early, before
    // the parts a narrow terminal drops: a table missing two hundred rows with
    // nothing saying so is worse than a table with no axis label.
    let hidden = match app.hidden_kernel_threads() {
        0 => String::new(),
        n => format!(" · {n} kernel hidden"),
    };

    // What poptop stopped measuring because it could not afford it. Ranked
    // just under the withheld rows above: both are omissions, and an omission
    // that does not state itself is the one thing this panel never does. A
    // budget that silently dropped a figure would be the objection to having a
    // budget at all.
    let afford = match app.withheld() {
        // Over budget with nothing optional big enough to be the reason. A
        // different message because it has a different remedy: the interval is
        // too short for this machine, and no column poptop could drop would
        // change that.
        [] if app.baseline_over_budget() => {
            " · sampling takes longer than a quarter of the interval".to_string()
        }
        [] => String::new(),
        w => format!(
            " · {} withheld, sampling was over budget",
            w.iter()
                .map(|s| s.label())
                .collect::<Vec<_>>()
                .join(" and ")
        ),
    };

    // What poptop had to assume at the moment being looked at — from the
    // sample itself, so a day opened a week later carries its own reasons
    // rather than this session's. Ranked with the other omissions: an
    // assumption nobody is told about is the same shape as a wrong number.
    let assumed = match app.history.current().and_then(|s| s.notes.as_deref()) {
        Some([]) | None => String::new(),
        Some(notes) => format!(" · {}", notes.join(" · ")),
    };

    // Why the log stopped. Ranked with the withheld sources above and for the
    // same reason: history nobody is recording is an omission, and one the
    // reader has to hear about while it is happening rather than when they
    // quit.
    let logging = match app.log_note.as_deref() {
        Some(why) => format!(" · {why}"),
        None => String::new(),
    };

    // Why the expansion is showing nothing. An expanded process with no rows
    // under it is indistinguishable from a process with one thread, and the
    // reader who pressed the key deserves to know which.
    let threads = match app.thread_note() {
        Some(why) => format!(" · {why}"),
        None => String::new(),
    };

    // Processes, not rows. They were the same thing until a row could stand
    // for six of them, and then the title said `processes (2)` above seven
    // running processes — the lie by omission this panel is careful never to
    // tell, and which the hidden-kernel-thread count exists to prevent one
    // line over.
    // Thread rows excluded. They are rows and not processes, and counting them
    // would make `processes (7)` appear above a table holding two processes and
    // five threads of one of them — the same lie by omission the hidden-kernel
    // count exists to prevent, four lines up.
    let shown_procs: usize = rows_data
        .iter()
        .filter(|r| !r.is_thread())
        .map(|r| r.count())
        .sum();

    // What the column said, said once.
    // Unknown owners are counted, not claimed: `all oddurs` over a table with
    // two hundred root-owned daemons in it would be false, and the column that
    // would have said otherwise has been folded.
    let all_one = one_user
        .as_deref()
        .map_or(String::new(), |u| match app.unknown_owners() {
            0 => format!(" · all {u}"),
            n => format!(" · all {u} but {n} unknown"),
        });

    // What the table cannot show, ranked and given up from the least important
    // end — because at eighty columns not all of it fits, and a clipped title
    // reads as a message called `io: panel too narr`.
    //
    // Everything here is an omission or an event: something withheld, stopped,
    // missing or over. What the *reader* did to the table — the sort, the
    // folding, the averaging — is on the strip above, where the tabs that
    // change it are. The two kinds of statement were mixed in this one line
    // and read as one string of facts, so a reason a column was empty sat
    // behind an identical `·` as a preference somebody had set.
    //
    // The ranks are the argument:
    //
    //   0  the count            — the panel's subject
    //  10  `all <user>`         — this one *replaces a column*; without it the
    //                             table has silently dropped a field
    //  20  `N kernel hidden`    — rows withheld; its absence is a lie by
    //                             omission, which is the one thing this panel
    //                             is careful never to do
    //  22  `... withheld`       — poptop stopped measuring something; an
    //                             omission, and the reason a budget is allowed
    //                             to exist at all
    //  28  the thread note      — the message the `y` key looks broken without:
    //                             an expanded process with no rows under it
    //  30  the io status        — the message the disk columns look broken
    //                             without
    //  36  `history flat`       — why the history column is not there
    //  45  the constraint       — advice, and the only clause here the reader
    //                             can act on
    //  58  OOM kills            — an event, and the answer to "what happened
    //                             to my process"
    //  60  churn                — a nicety
    //
    // Display order and drop order are separate: the list below reads left to
    // right as it appears on screen, and the rank beside each says when it
    // goes.
    //
    // Each clause carries its own style, because they are not all the same kind
    // of statement. `N/M need root` is a warning — a reason a column is empty,
    // and something someone can act on — and behind an identical `·` it read as
    // one more fact in a string of facts.
    // The watched process is not in this sample. Said, not silently swapped: a
    // process that appears partway through the buffer is information, and often
    // it is *the* information — the reader scrubbed back to find out when it
    // started. Ranked just under the count, because while scrubbing it explains
    // why nothing is highlighted.
    let absent = app
        .watched_but_absent(&rows_data)
        .map_or(String::new(), |w| {
            format!(" · {} not running here", w.name())
        });

    // Named, never imposed. A table that reorders itself under the reader is
    // worse than one that does not, so this says what is in the way and `S`
    // acts on it. Silent when nothing is constrained, and silent when the table
    // is already sorted that way — there would be nothing to accept.
    let constraint = app
        .constraint()
        .filter(|c| c.sort() != app.sort)
        .map_or(String::new(), |c| {
            format!(" · {} is the constraint (S)", c.name())
        });

    // The same principle for the view that folds a crowd. Twelve rows of one
    // program crowded out everything else while the key that folds them was
    // off screen and unadvertised (0112) — so it is named when it would help,
    // and `g` acts on it.
    let crowd = app.crowding().map_or(String::new(), |(name, n)| {
        format!(" · {n} {name} (g folds them)")
    });

    // A filter that could not be parsed is filtering nothing, which is a
    // surprising thing for the table to be doing silently once the filter box
    // has closed.
    let bad_filter = app
        .filter_error()
        .map_or(String::new(), |why| format!(" ! filter: {why}"));

    let (io_text, io_is_warning) = io_status(show_io, app, collected);
    let plain = app.theme.title_style();
    // An em dash before the first of them and a `·` before the rest, so they
    // read as one clause about the table rather than as three more facts —
    // and so the first one still reads correctly when the other two are gone.
    let settings = if strip {
        [String::new(), String::new(), String::new()]
    } else {
        let [sort, fold, avg] = settings_parts(app);
        [
            format!(" — {sort}"),
            if fold.is_empty() {
                fold
            } else {
                format!(" · {fold}")
            },
            if avg.is_empty() {
                avg
            } else {
                format!(" · {avg}")
            },
        ]
    };
    let parts = [
        (0u8, format!(" processes ({})", shown_procs), plain),
        (5, absent, plain),
        (7, bad_filter, app.theme.warning_style()),
        (10, all_one, plain),
        (20, hidden, plain),
        (22, afford, app.theme.warning_style()),
        // Beside the withheld sources, and just under them. Both are poptop
        // saying it has stopped recording something; this one is history
        // rather than a column, and the reader can act on it — the interval,
        // the byte budget, or the disk.
        (23, logging, app.theme.warning_style()),
        // Under the log note and above the thread note: what was assumed is
        // an omission of certainty rather than of data, and it is true of the
        // instant on screen.
        (24, assumed, app.theme.warning_style()),
        (28, threads, plain),
        // Only when the strip above the table is not there to say them. See
        // the note on this function. One clause each, at three ranks, so a
        // narrow rule gives up the averaging before the folding and the
        // folding before the ordering — the same order the strip drops them
        // in, reached through the machinery this line already has.
        (40, settings[0].clone(), plain),
        (48, settings[1].clone(), plain),
        (52, settings[2].clone(), plain),
        // Why the history column is missing, when that is the reason: said,
        // because a column that comes and goes unexplained reads as a bug.
        // Under the averaging note, which is about a figure on every row
        // rather than about a column that is not there.
        (
            36,
            if shape.flat {
                " · history flat".to_string()
            } else {
                String::new()
            },
            plain,
        ),
        // Just under the sort it is about, and above the modes: a suggestion a
        // narrow terminal drops is one nobody can act on, but it is still
        // advice rather than a fact about the data.
        (45, constraint, plain),
        (46, crowd, plain),
        (60, churn, plain),
        // Ranked with the churn it sits beside, one above: a process that was
        // killed is a stronger fact than one that merely came and went, and it
        // is the answer to a question somebody is actively asking.
        (58, killed, app.theme.warning_style()),
        (
            30,
            io_text,
            if io_is_warning {
                app.theme.warning_style()
            } else {
                plain
            },
        ),
    ];
    let title = fit_title(&parts, (area.width as usize).saturating_sub(4));

    let (widths, sorts) = table_columns(&shape);
    let _ = &sorts;

    f.render_widget(
        Paragraph::new(divider_of(title, area.width, &app.theme)),
        Rect { height: 1, ..area },
    );
    // Between the title and the column headers: the title says which processes
    // these are, this says what they add up to, and the headers name the
    // columns. Each row is one step closer to the figures.
    let summary = summary_height(area);
    if summary > 0
        && let Some(line) = summary_line(
            app,
            app.history.current().map_or(0, |s| s.mem.total),
            area.width as usize,
        )
    {
        f.render_widget(
            Paragraph::new(line).style(app.theme.panel_style()),
            Rect {
                y: area.y + 1,
                height: 1,
                x: table_body(app, area).x,
                width: table_body(app, area).width,
            },
        );
    }
    // The same gap the hit-testing splits with. Left at ratatui's default of
    // one while `sort_at` split with two, a click on a header landed a column
    // short of the column it was over.
    let table = Table::new(rows, widths)
        .header(header)
        .column_spacing(app.density.column_gap());
    f.render_widget(table, table_body(app, area));
}

/// Width of the per-process history sparkline, in cells.
/// The cursor's mark, named once so the renderer and the tests cannot drift.
#[cfg(test)]
pub const MARK: char = '▲';

pub const SPARK_W: usize = 10;

/// One process's CPU history as a sparkline.
///
/// Peak aggregation, like the timeline: averaging a spike with idle samples
/// renders it as nothing, and a spike is the entire reason to look.
///
/// A process absent from a sample leaves a gap rather than a zero. "It was not
/// running" and "it was running and idle" are different facts, and a graph that
/// conflates them invents history.
fn sparkline(series: Option<&[Option<f32>]>, set: GlyphSet, zoom: usize, ceiling: f32) -> String {
    let Some(series) = series else {
        return " ".repeat(SPARK_W);
    };
    let spc = set.spark_samples_per_cell();
    let slots = SPARK_W * spc;
    let values: Vec<f32> = series.iter().map(|v| v.unwrap_or(0.0)).collect();
    let agg = history::peak_slots(&values, zoom.max(1), slots);
    if spc == 1 {
        // The eighths ramp: one sample a cell, nine heights.
        return agg
            .iter()
            .map(|v| match v {
                None => ' ',
                Some(v) => set.spark_glyph(eighths(*v, ceiling)),
            })
            .collect();
    }
    agg.chunks(spc)
        .map(|cell| {
            let a = cell[0].unwrap_or(0.0);
            let b = *cell.get(1).and_then(|v| v.as_ref()).unwrap_or(&a);
            let l = glyphs::level_in_row_scaled(a, 0, 1, ceiling);
            let r = glyphs::level_in_row_scaled(b, 0, 1, ceiling);
            set.glyph(l, r)
        })
        .collect()
}

/// A value as eighths of a ceiling, 0..=8.
///
/// Anything above zero rounds *up* to at least one eighth. A process using 0.4%
/// of the machine is running, and a blank cell says it was not there at all —
/// the same distinction the gap above is drawing.
fn eighths(v: f32, ceiling: f32) -> usize {
    if v <= 0.0 || !v.is_finite() || ceiling <= 0.0 {
        return 0;
    }
    ((v / ceiling * 8.0).ceil() as usize).clamp(1, 8)
}

/// The scale the movement test is judged on: the whole buffer's peak.
///
/// Not the window's, which is what the drawn sparklines are scaled to. The
/// question here is whether anything moved at all, and an answer that changed
/// as the window scrolled would take the column away and give it back while
/// the reader scrubbed.
fn buffer_ceiling(app: &App) -> f32 {
    glyphs::ceiling_for(
        app.history
            .iter()
            .flat_map(|s| s.procs.iter())
            .map(|p| p.cpu)
            .fold(0.0_f32, f32::max),
    )
}

/// Whether any process in the buffer has a history that moves: whether, on the
/// shared scale, any of them was ever drawn at two different heights.
///
/// Over the whole buffer and every process in it, like the scale itself — not
/// over the rows on screen. Judged from the visible rows, scrolling the one
/// busy process out of view took the column away, and the table changed shape
/// because of where the list was sitting.
///
/// One pass, stopping at the first process seen at a second height, which on a
/// real machine is within the first few samples. Measured per sample rather
/// than per drawn slot: a process whose samples differ in height is one whose
/// line is not flat, whatever the zoom packs together.
fn any_history_moves(app: &App, ceiling: f32) -> bool {
    let mut first: std::collections::HashMap<(i32, u64), usize> = std::collections::HashMap::new();
    for s in app.history.iter() {
        for p in &s.procs {
            let Some(key) = p.key() else { continue };
            let level = glyphs::level_in_row_scaled(p.cpu, 0, 1, ceiling);
            if *first.entry(key).or_insert(level) != level {
                return true;
            }
        }
    }
    false
}

/// Width of a process-table bar. Four cells at eight sub-steps is thirty-two
/// levels, which is enough to compare two rows at a glance.
const BAR_W: usize = 4;

/// The CPU bar, with the over-one-core case marked rather than clipped.
///
/// A threaded process really can use 400% of a core. Clipping it to a full bar
/// would make it indistinguishable from one using exactly 100%, so the excess
/// gets a mark of its own.
fn cpu_bar(pct: f32) -> String {
    let bar = glyphs::micro_bar(pct / 100.0, BAR_W);
    if pct > 100.0 {
        format!("{bar}+")
    } else {
        format!("{bar} ")
    }
}

/// A process's share of the machine's memory.
fn mem_frac(rss: u64, total: u64) -> f32 {
    if total == 0 {
        return 0.0;
    }
    rss as f32 / total as f32
}

/// One disk-rate cell.
///
/// Three distinct states, none of them a zero: `·` for history recorded before
/// the column was switched on, `—` for a process this user may not read, and a
/// rate otherwise.
fn io_cell(collected: bool, io: Option<IoRates>, write: bool, theme: &Theme) -> Cell<'static> {
    let dim = theme.dim_style();
    // Right-aligned with the other figures, including the two placeholders: a
    // column of rates with `·` hanging off the left edge reads as a different
    // column.
    match (collected, io) {
        (false, _) => num("·").style(dim),
        (true, None) => num("—").style(dim),
        (true, Some(io)) => {
            let bytes = if write { io.write } else { io.read };
            if bytes == 0 {
                num("0").style(dim)
            } else {
                num(format!("{}/s", fmt_bytes(bytes)))
            }
        }
    }
}

/// Panel-title note about IO availability.
///
/// If most processes are unreadable the table would otherwise look broken; this
/// says why, and implies the fix.
/// What to say about the disk columns, and whether it is a warning.
///
/// Two kinds of statement wearing the same clothes was the problem: a reader
/// could not tell which of the title's clauses was telling them something was
/// wrong. `N/M need root` is a *reason a column is empty* — someone can act on
/// it — while the rest explain why the columns are absent and need no action.
/// The `bool` is what lets the two be drawn differently.
fn io_status(show_io: bool, app: &App, collected: bool) -> (String, bool) {
    // Nothing to report on a tab that does not carry these columns. The memory
    // tab was announcing `! io: panel too narrow` at every width, about columns
    // it would not have drawn at any of them — a warning that named a problem
    // the reader could not have, next to the four columns they had asked for.
    if !app.show_io || !app.view.wants_io() {
        return (String::new(), false);
    }
    // Asked for but not drawn. Without this the key is a silent no-op on a
    // narrow panel: the columns do not appear, nothing says why, and the
    // obvious conclusion is that the feature is broken.
    // A warning, not a legend: widening the terminal fixes it, which is the
    // test the two are split on. It is also the one message this panel goes out
    // of its way to guarantee — without it the `i` key is a silent no-op.
    if !show_io {
        return (" ! io: panel too narrow".into(), true);
    }
    // A kernel question rather than a permission one, and they want different
    // words: nothing the user does will make this appear.
    if app.history.current().is_some_and(|s| !s.io_supported) {
        return (
            " · io: this kernel keeps no per-process accounting".into(),
            false,
        );
    }
    if !collected {
        return (" · io: not collected here".into(), false);
    }
    // Only unreadable processes are worth mentioning: a process awaiting its
    // second reading also shows a dash, but resolves on its own and needs no
    // action from anyone.
    let Some(s) = app.history.current() else {
        return (String::new(), false);
    };
    // Nothing to say: the columns are there and they are readable. The columns
    // themselves are the legend.
    if s.io_denied == 0 {
        return (String::new(), false);
    }
    // Against the processes IO was attempted for, not the rows on screen. The
    // two are different numbers the moment a filter is active, and `90/2 need
    // root` is not a ratio of anything.
    let eligible = s.procs.iter().filter(|p| !p.is_kernel_thread()).count();
    // Says what needs root. Without the subject the clause read
    // `processes (312) — sort: CPU ⚠ 41/298 need root` with nothing tying it to
    // the disk columns it is about.
    //
    // `!` rather than `⚠`: the warning sign is given emoji presentation by
    // several terminals and drawn two columns wide, while every width in this
    // file is counted in `chars`. A rule that runs one column past its panel is
    // the byte-versus-column mistake again, one layer up. The style carries the
    // severity; the marker only has to be visible.
    (format!(" ! io: {}/{eligible} need root", s.io_denied), true)
}

/// What can be done to the selected process, and what cannot and why.
///
/// Activity Monitor puts three controls in its title bar and attaches them to
/// the selection: they are visible, they are few, and which of them are
/// available tells you what can be done to what you have picked. This is the
/// terminal's version — the footer already changes with the mode, and a
/// selection is a mode.
///
/// `None` when nothing is selected, so the row says nothing rather than
/// offering actions with no subject.
fn selection_actions(app: &App, width: usize) -> Option<Line<'static>> {
    let name = match app.selected.as_ref()? {
        // The *short* name, not the command line. `Watched` keeps the full
        // command so a missing process can be named unambiguously, and putting
        // that in a one-line bar spends sixty columns identifying a row the
        // reader is already looking at.
        crate::app::Watched::Process { pid, name, .. } => {
            let short = name.split_whitespace().next().unwrap_or(name);
            let short = short.rsplit('/').next().unwrap_or(short);
            format!("{short} · {pid}")
        }
        // A folded row is several processes, and "signal the one under the
        // cursor" there means picking one of them — which is not a decision a
        // confirmation could describe. So it offers nothing.
        crate::app::Watched::Group { .. } => return None,
    };
    let mut spans = vec![
        Span::styled(" ", app.theme.dim_style()),
        Span::styled(name, app.theme.title_style()),
        Span::styled("  ⏎ inspect", app.theme.dim_style()),
    ];
    // Said before it is attempted, not after. An action bar offering `x quit`
    // on a recorded day and then refusing it is worse than one that never
    // offered it: the reader has already decided by the time they find out.
    match app.signal_refusal() {
        None => spans.push(Span::styled(
            " · x TERM · X KILL".to_string(),
            app.theme.dim_style(),
        )),
        Some(why) => spans.push(Span::styled(
            format!(" · no signal: {}", why.short()),
            app.theme.warning_style(),
        )),
    }
    // Its own ladder, because this row is shared. What the reader picked and
    // what can be done to it outrank the key hints, which are a reminder; but
    // the whole bar is given up before it clips, because a clipped action list
    // reads as an action that does not exist.
    let used: usize = spans.iter().map(|s| cols(&s.content)).sum();
    if used > width {
        return None;
    }
    let rest = width.saturating_sub(used + 3);
    let hints = fit_hints(KEY_HINTS, rest as u16);
    if !hints.is_empty() {
        spans.push(Span::styled(format!("   {hints}"), app.theme.dim_style()));
    }
    Some(Line::from(spans))
}

fn draw_help(f: &mut Frame, area: Rect, app: &App) {
    let area = content(app, area);
    let line = if app.editing_filter {
        // The field itself is on the scope line, where the filter is the scope.
        // What is left for this row is what the field takes and how to leave
        // it — which a one-line box had nowhere else to put, and which is the
        // reason the box existed at all.
        match app.filter_error() {
            Some(why) => Line::from(Span::styled(format!(" {why}"), app.theme.warning_style())),
            None => Line::from(Span::styled(
                " ⏎ keep · esc cancel · try `postgres`, `user:root`, `cpu>50`".to_string(),
                app.theme.dim_style(),
            )),
        }
    } else if let Some(p) = app.pending.as_ref() {
        // The question, naming the process. The number is the part that gets
        // misread, and it is the only thing the alternative workflow — reading
        // a pid off one screen and typing it into another — carries across.
        Line::from(vec![
            Span::styled(" ", app.theme.warning_style()),
            Span::styled(p.question(area.width as usize), app.theme.warning_style()),
        ])
    } else if app.editing_jump {
        // The forms, where the moment is being typed. A one-line box has
        // nowhere else to say what it takes, and a prompt that rejects what you
        // typed without saying which forms it accepts is one you type into
        // twice.
        //
        // Fitted, like the key hints, and given up from the least useful end. A
        // fixed tail is seventy-six columns before a single character is typed,
        // so on an eighty-column terminal it clipped to `Esc to c` after four
        // keystrokes — the box stopping saying what it takes exactly where it
        // has to.
        let typed = cols(&app.jump) + cols("jump to: ") + 1;
        let tail = [
            "   -2h · 03:00 · 2026-09-08 03:00   (Enter to jump, Esc to cancel)",
            "   -2h · 03:00 · 2026-09-08 03:00",
            "   -2h · 03:00",
            "   -2h",
            "",
        ]
        .into_iter()
        .find(|t| typed + cols(t) <= area.width as usize)
        .unwrap_or("");
        Line::from(vec![
            Span::styled("jump to: ", app.theme.cursor_style()),
            Span::raw(&app.jump),
            Span::styled("█", app.theme.cursor_style()),
            Span::styled(tail, app.theme.dim_style()),
        ])
    } else if let Some(note) = app.theme_note.as_deref() {
        // What the last theme reload did. Colours are judged by looking, so
        // the message says which file and whether it took.
        Line::from(Span::styled(format!(" {note}"), app.theme.warning_style()))
    } else if let Some(note) = app.signal_note.as_deref() {
        Line::from(Span::styled(format!(" {note}"), app.theme.warning_style()))
    } else if let Some(note) = app.jump_note.as_deref() {
        // What the last jump did, after the box has closed. Whether anything
        // was recorded at 03:00 is the whole point of asking, and a message
        // that vanished with the prompt would be one nobody read.
        Line::from(Span::styled(format!(" {note}"), app.theme.warning_style()))
    } else if let Some(actions) = selection_actions(app, area.width as usize) {
        // Last, after every prompt: a pending confirmation, a jump note and a
        // filter error are all about something the reader just did, and this is
        // about something they are still looking at.
        actions
    } else {
        Line::from(Span::styled(
            fit_hints(KEY_HINTS, area.width),
            app.theme.dim_style(),
        ))
    };
    f.render_widget(Paragraph::new(line), area);
}

/// The key hints, in the order they are given up.
///
/// Least useful last, because that is the end a narrow terminal loses. `K` is
/// the newest and the most niche; `/` is the one people reach for constantly,
/// and it used to be what fell off — adding `K kernel` pushed the footer two
/// columns past an eighty-… past a hundred-column terminal and `/ filter`
/// rendered as `/ filt`.
#[cfg(test)]
pub fn fit_hints_for_test(width: u16) -> String {
    fit_hints(KEY_HINTS, width)
}

pub const KEY_HINTS: &[&str] = &[
    "q quit",
    // Second, because it is the one hint that leads to all the others: the bar
    // names every command there is, beside the key that also runs it.
    "F10 menu",
    "←/→ scrub",
    "b jump",
    "+/- zoom",
    "Space live",
    "↑/↓ select",
    "s sort",
    "/ filter",
    // Below the keys used constantly and above the niche ones. Signalling is
    // off unless it has been asked for, so the hint is mostly there to say the
    // key exists — which is worth less than `s sort` and more than `K kernel`.
    "x signal",
    "t tree",
    // `Tab` rather than `v`: the strip above the table is what it moves, and
    // the strip is on screen saying so.
    "Tab tabs",
    "y threads",
    "C cgroups",
    "K kernel",
    "g group",
    "d detail",
    "S constraint",
];

/// As many hints as fit, joined, never cut mid-hint.
///
/// A clipped footer reads as a key called `filt`. Dropping whole hints from the
/// end is the same degradation ladder the header figures and the timeline rows
/// use, and it means what is on screen is always true.
/// Every key, for the `?` overlay: how it is shown, how `--help` spells it, and
/// what it does. The one list: the footer's hints must all be in it and every
/// key in it must be in `--help`, which a test holds — the three had drifted,
/// and `--help` had never mentioned `v`, `y` or `C` (0113).
pub const HELP: &[(&str, &str, &str)] = &[
    ("q", "q, Esc", "quit"),
    (
        "Esc",
        "q, Esc",
        "back out one level: a box, a signal, the selection — then quit",
    ),
    (
        "←/→",
        "Left/Right",
        "scrub through history, ten at a time with Shift",
    ),
    ("b", "b", "jump to a moment: -2h, 03:00, 2026-09-08 03:00"),
    ("+/-", "+ / -", "zoom the timeline in and out"),
    ("Space", "Space", "pause on this sample, or go back to live"),
    ("Home, End", "Home/End", "the oldest sample, or live"),
    ("↑/↓", "Up/Down", "select a process"),
    ("s", "s", "cycle the sort column"),
    ("S", "S", "sort by what the panel names as the constraint"),
    (
        "/",
        "/",
        "filter: a word, or a query like `cpu > 5 and user = root`",
    ),
    (
        "x, X",
        "x, X",
        "send TERM or KILL to the selected process (--signals=on)",
    ),
    ("t", "t", "the process tree"),
    (
        "g",
        "g",
        "fold processes by name, then by user, then by container",
    ),
    (
        "d",
        "d",
        "the selected process's own history in place of the machine's",
    ),
    ("y", "y", "the selected process's threads"),
    ("v", "v", "the next view: memory, then disk"),
    ("C", "C", "cgroups in place of processes"),
    ("K", "K", "kernel threads"),
    ("R", "R", "read the theme file again, for trying a colour"),
    (
        "Tab",
        "Tab",
        "the next tab, Shift-Tab the previous, 1-9 one by number",
    ),
    ("Enter", "Enter", "the inspector on the selected process"),
    (
        "F10",
        "F10",
        "the menu bar, which names every command there is",
    ),
    ("?", "?", "this list"),
];

/// The `?` overlay: every key and what it does, over the middle of the screen.
///
/// Modal, and put away by any key — the key that closes it is not also acted
/// on, so `q` closes the list rather than quitting behind it.
fn draw_key_list(f: &mut Frame, app: &App) {
    let area = f.area();
    let key_w = HELP.iter().map(|(k, _, _)| cols(k)).max().unwrap_or(0);
    let text_w = HELP
        .iter()
        .map(|(_, _, what)| key_w + 2 + cols(what))
        .max()
        .unwrap_or(0);
    let w = (text_w + 4).min(area.width as usize) as u16;
    let rows = HELP.len() + usize::from(!app.keys.is_default());
    let h = (rows + 2).min(area.height as usize) as u16;
    let rect = Rect {
        x: area.x + (area.width - w) / 2,
        y: area.y + (area.height - h) / 2,
        width: w,
        height: h,
    };
    let mut lines: Vec<Line> = HELP
        .iter()
        .map(|(key, _, what)| {
            Line::from(vec![
                Span::styled(format!(" {key:<key_w$}  "), app.theme.title_style()),
                Span::raw(what.to_string()),
            ])
        })
        .collect();
    // The list above is the keys poptop ships with. Where a config file has
    // moved one, saying so beats printing a list that is wrong: the resolved
    // map is `poptop --keys`, which needs no running monitor to read.
    if !app.keys.is_default() {
        lines.push(Line::from(Span::styled(
            " keys have been rebound — poptop --keys".to_string(),
            app.theme.dim_style(),
        )));
    }
    f.render_widget(ratatui::widgets::Clear, rect);
    f.render_widget(
        Paragraph::new(lines).block(
            ratatui::widgets::Block::bordered()
                .title(" keys — any key closes ")
                .border_style(app.theme.border_style()),
        ),
        rect,
    );
}

/// Where the footer points when it could not show every key.
const MORE: &str = "? more";

fn fit_hints(hints: &[&str], width: u16) -> String {
    const SEP: &str = " · ";
    let width = width as usize;
    // Everything, if everything fits. Otherwise as much as fits with room kept
    // for `? more` at the end — at 120 columns six keys never appeared, and
    // nothing said there were more (0113).
    let all = hints.join(SEP);
    if cols(&all) <= width {
        return all;
    }
    // Narrower than the pointer itself: nothing, rather than a pointer cut in
    // half.
    if cols(MORE) > width {
        return String::new();
    }
    let width = width.saturating_sub(cols(SEP) + cols(MORE));
    let mut out = String::new();
    for h in hints {
        let need = if out.is_empty() {
            cols(h)
        } else {
            cols(&out) + cols(SEP) + cols(h)
        };
        if need > width {
            break;
        }
        if !out.is_empty() {
            out.push_str(SEP);
        }
        out.push_str(h);
    }
    if out.is_empty() {
        MORE.to_string()
    } else {
        format!("{out}{SEP}{MORE}")
    }
}
