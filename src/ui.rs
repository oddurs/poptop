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
use ratatui::widgets::{Cell, Paragraph, Row, Table};
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
    let top = HEADER_H;
    top..top + timeline_height(total_height, HEADER_H)
}

pub fn draw(f: &mut Frame, app: &App) {
    // Measured rather than assumed, so the node row is a row the layout knows
    // about instead of one drawn over the timeline.
    let header = header_height(app);
    let chunks = Layout::vertical([
        Constraint::Length(header),
        Constraint::Length(timeline_height(f.area().height, header)),
        // Whatever remains. `timeline_height` has already reserved the table's
        // share, and a `Min` here would outrank the timeline's `Length` and
        // silently shrink it below the height that function reports.
        Constraint::Min(1),
        Constraint::Length(1), // help
    ])
    .split(f.area());

    let Some(sample) = app.history.current() else {
        f.render_widget(
            Paragraph::new("collecting first sample…").style(app.theme.dim_style()),
            f.area(),
        );
        return;
    };

    draw_header(f, chunks[0], app, sample);
    draw_timeline(f, chunks[1], app);
    if app.show_cgroups {
        draw_cgroups(f, chunks[2], app);
    } else {
        draw_procs(f, chunks[2], app);
    }
    draw_help(f, chunks[3], app);
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

fn fmt_lag(d: Duration) -> String {
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
    if days > 0 {
        format!("{days}d {hours}h {mins}m")
    } else {
        format!("{hours}h {mins}m")
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
/// Between two figures about the same resource.
const NEAR: &str = "  ";
/// Between two groups. Wider, and marked, because a group boundary that looks
/// like the gap inside a group is not a boundary — and the mark carries on a
/// terminal with no colour to spend.
const FAR: &str = "  │  ";

/// Exposed for tests: the units the fitting arithmetic is done in.
#[cfg(test)]
pub fn separator_widths_for_test() -> (usize, usize, usize, usize) {
    (sep_w(NEAR), NEAR.len(), sep_w(FAR), FAR.len())
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

fn full_width(figures: &[Figure<'_>]) -> usize {
    let mut order: Vec<&Figure<'_>> = figures.iter().collect();
    order.sort_by_key(|f| f.group);
    let mut w = 0;
    let mut last: Option<Group> = None;
    for f in order {
        w += match last {
            None => 0,
            Some(g) if g == f.group => sep_w(NEAR),
            Some(_) => sep_w(FAR),
        } + f.spans.iter().map(|s| cols(&s.content)).sum::<usize>();
        last = Some(f.group);
    }
    w
}

fn fit<'a>(figures: Vec<Figure<'a>>, width: usize, theme: &Theme) -> Vec<Span<'a>> {
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
                Some(g) if g == groups[i] => sep_w(NEAR),
                Some(_) => sep_w(FAR),
            } + widths[i];
            last = Some(groups[i]);
        }
        w
    };

    // What to keep: by rank, least diagnostic first out.
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
            Some(g) if g == groups[i] => out.push(Span::raw(NEAR)),
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

fn draw_header(f: &mut Frame, area: Rect, app: &App, s: &Sample) {
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
    if let Some(l) = s.net.as_ref().and_then(NetStat::busiest) {
        figures.push(Figure {
            group: Group::Network,
            rank: 55,
            spans: vec![
                Span::styled(format!("{} ", l.name), dim),
                Span::styled(format!("{}/s", fmt_bytes(l.rx)), dim),
                Span::styled(" ", dim),
                Span::styled(format!("{}/s", fmt_bytes(l.tx)), dim),
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
            Span::raw(s.procs.len().to_string()),
        ],
    });
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
    let scale = heat_scale(area.width, &app.theme)
        .filter(|s| state_w + full_width(&figures) + 2 + cols(s) <= width);
    let reserved = scale.as_ref().map_or(0, |s| cols(s) + 2);

    let mut line = vec![state];
    let spans = fit(
        figures,
        width.saturating_sub(state_w + reserved),
        &app.theme,
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

/// The scrubable timeline, oldest on the left.
///
/// Two packings compose here: each character cell holds `samples_per_cell`
/// display slots, and each slot aggregates `zoom` samples by peak. At zoom 5
/// with braille that is ten seconds per cell, so a normal terminal shows the
/// entire buffer.
fn draw_timeline(f: &mut Frame, area: Rect, app: &App) {
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

    let spc = app.glyphs.samples_per_cell();
    let slots = graph_w * spc;
    let samples: Vec<&Sample> = app.history.iter().collect();
    let zoom = app::effective_zoom(app.zoom(), samples.len(), slots);
    let shown = (slots * zoom).min(samples.len());
    // Text-editor scrolling. The window stays anchored to the live edge while
    // the cursor is inside it, and follows only once the cursor would leave —
    // so the live view never shuffles, and scrubbing never takes you somewhere
    // you cannot see.
    //
    // Stateless on purpose: the window position is derived from the cursor each
    // frame rather than stored, so there is no scroll offset to keep in sync
    // with a buffer that is being written to at the same time.
    let window_start = window_start(&app.history, shown);
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
    if app.history.current().is_some_and(|s| s.net.is_some()) {
        candidates.push((
            "NET",
            window
                .iter()
                .map(|s| {
                    s.net
                        .as_ref()
                        .and_then(|n| n.busiest())
                        // `Link::bytes`, not `rx + tx`: that function is what
                        // `busiest` selects on, so re-deriving it here would
                        // keep the "matches the header figure" claim true in
                        // two places instead of one.
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
    for (i, (name, raw, unit)) in candidates.iter().enumerate() {
        let rows = row_split[i];
        let slots_for = history::peak_slots(raw, zoom, slots);
        let values = &slots_for;
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
        // Each graph scales to its own peak: memory at 78% and CPU at 16% are
        // different questions and deserve different axes.
        let peak = values.iter().flatten().copied().fold(0.0_f32, f32::max);
        let ceiling = unit.ceiling(peak);
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
                .filter_map(|&pct| glyphs::rule_position_scaled(pct, rows, ceiling))
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
                ceiling,
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
                        ceiling,
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

    let mut all = vec![divider(&title, area.width, &app.theme)];
    all.extend(lines);
    f.render_widget(Paragraph::new(all), area);
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
    /// Top of the y-axis for this graph.
    ceiling: f32,
    /// Per-slot flags marking where time is missing from the buffer.
    gaps: &'a [bool],
}

/// Draw one row of a graph.
fn glyph_row(g: GraphRow, theme: &Theme) -> Line<'static> {
    let (set, values, row, rows, spc, rule_level, series, ceiling, gaps) = (
        g.set,
        g.values,
        g.row,
        g.rows,
        g.spc,
        g.rule_level,
        g.series,
        g.ceiling,
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
                return Span::styled(set.gap_glyph().to_string(), theme.chrome_style());
            }
            let pcts: Vec<f32> = cell.iter().map(|v| v.unwrap_or(0.0)).collect();
            let left = glyphs::level_in_row_scaled(pcts[0], row, rows, ceiling);
            let right =
                glyphs::level_in_row_scaled(*pcts.get(1).unwrap_or(&pcts[0]), row, rows, ceiling);
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
            let has_data = cell.iter().any(|v| v.is_some());
            let empty = left.max(right) == 0;
            match rule_level {
                Some(lvl) if has_data && empty && i % 2 == 0 => {
                    Span::styled(set.rule_glyph(lvl).to_string(), theme.chrome_style())
                }
                _ => Span::styled(
                    set.glyph(left, right).to_string(),
                    theme.series_style(series),
                ),
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
        100.0,
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
    ceiling: f32,
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
        unit.axis(ceiling)
    } else if row + 1 == rows {
        "0".to_string()
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
    let marker = app.glyphs.cursor_marker(spc == 2 && slot % spc == 1);

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
    let side = |n: usize, anchors: bool| -> Option<(usize, usize)> {
        let (l0, r1) = if anchors {
            (ANCHOR_L + 1, width.saturating_sub(ANCHOR_R))
        } else {
            (0, width)
        };
        let pad = if anchors { 2 } else { 1 };
        let left = (l0, cell);
        let right = (cell + 1, r1);
        let ok = |(a, b): (usize, usize)| room(a, b) >= n + pad;
        match (ok(left), ok(right)) {
            (true, true) => Some(if room(left.0, left.1) >= room(right.0, right.1) {
                left
            } else {
                right
            }),
            (true, false) => Some(left),
            (false, true) => Some(right),
            (false, false) => None,
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
            (n > 0 && n <= width).then(|| side(n, true).map(|s| (c.as_str(), n, s, true)))?
        })
        .or_else(|| {
            w.captions.iter().find_map(|c| {
                let n = cols(c);
                (n > 0 && n <= width).then(|| side(n, false).map(|s| (c.as_str(), n, s, false)))?
            })
        });

    match chosen {
        Some((caption, n, (a, b), anchors)) => {
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
            put(&mut row, a + (b - a - n) / 2, caption);
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
/// Everything to the left of the command: pid, user, cpu% and its bar, rss and
/// its bar, state, threads, history.
const FIXED_COLUMNS: u16 =
    7 + 10 + 6 + (BAR_W as u16 + 1) + 8 + BAR_W as u16 + 2 + 4 + SPARK_W as u16;

/// The narrowest terminal the IO columns will appear on.
///
/// The two IO columns, one space between each of the twelve, and enough left
/// for a command name to be worth reading.
///
/// Takes `show_user` for the same reason [`command_width`] does: when that
/// column has been folded into the title its ten columns are free, and the IO
/// columns were refusing to appear until the terminal was eleven columns wider
/// than they needed to be.
#[cfg(test)]
pub fn command_width_for_test(width: u16, show_io: bool, show_user: bool) -> usize {
    command_width(width, show_io, show_user, false, 0, 0)
}

#[cfg(test)]
pub fn min_width_for_io_for_test(show_user: bool) -> u16 {
    min_width_for_io(show_user)
}

fn min_width_for_io(show_user: bool) -> u16 {
    let user = if show_user { USER_W } else { 0 };
    FIXED_COLUMNS - USER_W + user + 9 + 9 + 11 + 16
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
const USER_W: u16 = 10;

/// How much of the line is left for the command name.
///
/// The identity column is the one that takes what nothing else claimed, so it
/// is the one that runs out — at 104 columns with the IO columns shown it gets
/// nineteen, one more than the two disk-rate columns together. Knowing the
/// figure is what lets the name be elided deliberately rather than clipped by
/// the terminal.
fn command_width(
    width: u16,
    show_io: bool,
    show_user: bool,
    show_cid: bool,
    dropped: u16,
    taken: u16,
) -> usize {
    let (io, columns) = if show_io { (18, 12) } else { (0, 10) };
    // The container column and its gap. Left out, the elision arithmetic is
    // thirteen columns too generous and the command is elided in the middle
    // *and then* chopped at the right edge — losing the tail with no marker,
    // which is the failure the comment below is about.
    let (cid, columns) = if show_cid {
        (CID_W + 1, columns + 1)
    } else {
        (0, columns)
    };
    let (user, columns) = if show_user {
        (USER_W, columns)
    } else {
        (0, columns - 1)
    };
    // Floored at the column's own `Min`, not at one. Below that width ratatui
    // stops honouring the fixed lengths and squeezes them instead, so the
    // command cell is *wider* than this arithmetic says — and eliding against
    // the arithmetic rendered `Google Chrome Helper (Renderer)` as the single
    // letter `G` on an eighty-column terminal.
    // `dropped` is the width a view has given back: the bars and the thread
    // count are not always drawn, and the command gets what they were using.
    // Left out, the elision is more cautious than it needs to be — a milder
    // failure than the other direction, but still a name cut for no reason.
    width
        .saturating_sub(FIXED_COLUMNS - USER_W + user + io + cid + taken + (columns - 1))
        .saturating_add(dropped)
        .max(MIN_COMMAND_W) as usize
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

fn draw_procs(f: &mut Frame, area: Rect, app: &App) {
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
    let show_user = one_user.is_none() && app.group != crate::app::Grouping::User;
    // In the disk view the throughput columns are the point, so they are not
    // subject to the width test that hides them elsewhere — which is the
    // concrete thing views fix: today those figures vanish on a narrow terminal
    // with nothing to bring them back, and this key is what brings them back.
    let show_io = app.show_io
        && app.view.wants_io()
        && (app.view == crate::app::View::Disk || area.width >= min_width_for_io(show_user));
    // What the disk columns are given room by. A view is a named list of
    // columns over one renderer, not a second renderer.
    let show_bars = app.view != crate::app::View::Disk;
    let show_thr = app.view == crate::app::View::Generic;
    // The memory view's own columns: what a process's memory actually costs,
    // what it has reserved, whether it is being paged in, and which way it is
    // going.
    let show_mem_cols = app.view == crate::app::View::Memory;
    // What the view has given back, in columns, for the command to use — less
    // what it has taken. The memory view drops the thread count and *adds* four
    // columns of its own, so counting only the drops left the arithmetic
    // thirty-five columns too generous, and the command was elided in the
    // middle and then chopped at the right edge with no marker: the exact
    // failure `command_width` exists to prevent.
    let given = i32::from(if show_bars { 0 } else { BAR_W as u16 * 2 + 3 })
        + if show_thr { 0 } else { 5 }
        - if show_mem_cols {
            8 + 8 + 7 + 8 + 4i32
        } else {
            0
        };
    let dropped = given.max(0) as u16;
    let taken = (-given).max(0) as u16;
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
    let show_cid = app.group != crate::app::Grouping::User
        && app.any_container()
        && command_width(area.width, show_io, show_user, true, dropped, taken) as u16
            > MIN_COMMAND_W;
    let cmd_w = command_width(area.width, show_io, show_user, show_cid, dropped, taken);
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
    // The whole retained buffer, not a slice of it. A per-row summary that
    // shifted every time the timeline zoomed would be a second, contradictory
    // reading of the same history; "what this process has been doing" is a
    // fixed question with a fixed answer.
    let series = history::series_for(&app.history, &keys, app.history.len());
    let spark_slots = SPARK_W * app.glyphs.samples_per_cell();
    let spark_zoom = app.history.len().div_ceil(spark_slots.max(1)).max(1);

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
    let spark_ceiling = glyphs::ceiling_for(
        app.history
            .iter()
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
            let mut style = Style::default();
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
                cells.push(num("—").style(app.theme.dim_style()));
                if show_bars {
                    cells.push(Cell::from(""));
                }
                cells.push(Cell::from(th.state.to_string()));
                if show_thr {
                    cells.push(num("—").style(app.theme.dim_style()));
                }
                if show_io {
                    cells.push(num("—").style(app.theme.dim_style()));
                    cells.push(num("—").style(app.theme.dim_style()));
                }
                if show_mem_cols {
                    // A thread has no memory of its own; it shares its
                    // process's, one row up.
                    for _ in 0..4 {
                        cells.push(num("—").style(app.theme.dim_style()));
                    }
                }
                // No sparkline. The retained history is per process, so the
                // only series available here is the parent's — drawing it on
                // every thread row would put the same shape beside forty
                // different numbers and invite reading it as each one's.
                cells.push(Cell::from(""));
                cells.push(num(th.tid.to_string()));
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
            let mut cells = vec![
                num(format!("{:.1}", p.cpu)).style(app.theme.heat_style(p.cpu)),
                // A bar beside the number turns a column that must be read
                // into one that can be scanned. htop does the same, for the
                // same reason.
                //
                // Both bars are neutral. Length already carries the magnitude,
                // and the number beside each one already carries its status
                // colour — colouring the bar too would spend a third channel
                // on the same fact. Using a series hue here was worse still:
                // that is an identity token, and a share of memory is not an
                // identity. The C6 test caught it.
            ];
            if show_bars {
                cells.push(Cell::from(cpu_bar(p.cpu)).style(app.theme.dim_style()));
            }
            cells.push(num(fmt_bytes(p.rss)));
            if show_bars {
                cells.push(
                    Cell::from(glyphs::micro_bar(mem_frac(p.rss, total_mem), BAR_W))
                        .style(app.theme.dim_style()),
                );
            }
            cells.push(Cell::from(p.state.to_string()));
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
            if show_mem_cols {
                // Never a zero for any of these: a share nobody measured, a
                // size the platform does not publish and a fault count that was
                // not collected are all "not known", and this table has one way
                // of saying that.
                cells.push(num(match p.pss {
                    Some(b) => fmt_bytes(b),
                    None => "—".into(),
                }));
                cells.push(num(match p.vsize {
                    Some(b) => fmt_bytes(b),
                    None => "—".into(),
                }));
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
            cells.push(
                Cell::from(sparkline(
                    p.key().and_then(|k| series.get(&k)).map(Vec::as_slice),
                    app.glyphs,
                    spark_zoom,
                    spark_ceiling,
                ))
                .style(app.theme.dim_style()),
            );
            // Identity, all of it together — see the note above `rows`.
            // A group has no pid — it is not a process. The column carries how
            // many were folded in instead, which is the fact that replaces it.
            // A group has no pid — it is not a process. The column carries how
            // many were folded in instead, which is the fact that replaces it.
            // A group of one keeps the pid: there is a single process there and
            // `×1` says less than its number does.
            cells.push(num(match r.members {
                Some(n) if n > 1 => format!("×{n}"),
                _ => p.pid.to_string(),
            }));
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

    // A header aligned against its column is a header for a different column.
    // `right` marks the numeric ones; the bars and the text columns stay left.
    let right = |s| num(s).style(app.theme.table_header_style());
    let left = |s: &str| Cell::from(s.to_string()).style(app.theme.table_header_style());
    // In the order the cells are pushed, which is what `Table` pairs them by.
    // With the IO columns shown these had drifted a place: `HISTORY` sat over
    // DISK R, `DISK R` over DISK W, and `DISK W` over the sparkline — every one
    // of the three naming the column beside it.
    let mut header_cells = vec![right("CPU%")];
    if show_bars {
        header_cells.push(left(""));
    }
    header_cells.push(right("RSS"));
    if show_bars {
        header_cells.push(left(""));
    }
    header_cells.push(left("S"));
    if show_thr {
        header_cells.push(right("THR"));
    }
    if show_io {
        header_cells.push(right("DISK R"));
        header_cells.push(right("DISK W"));
    }
    if show_mem_cols {
        header_cells.push(right("PSS"));
        header_cells.push(right("VSZ"));
        header_cells.push(right("MAJF/s"));
        header_cells.push(right("GROW"));
    }
    header_cells.push(left(&spark_header(spark_ceiling)));
    header_cells.push(right("PID"));
    if show_user {
        header_cells.push(left("USER"));
    }
    if show_cid {
        header_cells.push(left("CID"));
    }
    header_cells.push(left("COMMAND"));
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
    let all_one = one_user
        .as_deref()
        .map_or(String::new(), |u| format!(" · all {u}"));

    // Ranked, and given up from the least important end, because at eighty
    // columns not all of it fits and a clipped title reads as a message called
    // `io: panel too narr`. The ranks are the argument:
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
    //  30  the io status        — the message the `i` key looks broken without
    //  40  the sort column      — not otherwise stated anywhere
    //  50  `tree`               — visible in the rows themselves
    //  58  OOM kills            — an event, and the answer to "what happened
    //                             to my process"
    //  60  churn                — a nicety
    //  70  the history axis     — a nicety, and the ladder it was already at
    //                             the bottom of
    //
    // Display order and drop order are separate: the list below reads left to
    // right as it appears on screen, and the rank beside each says when it
    // goes. The sort clause reads better before the io status and is given up
    // first of the two.
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

    // A filter that could not be parsed is filtering nothing, which is a
    // surprising thing for the table to be doing silently once the filter box
    // has closed.
    let bad_filter = app
        .filter_error()
        .map_or(String::new(), |why| format!(" ! filter: {why}"));

    let (io_text, io_is_warning) = io_status(show_io, app, collected);
    let plain = app.theme.title_style();
    let parts = [
        (0u8, format!(" processes ({})", shown_procs), plain),
        (5, absent, plain),
        (7, bad_filter, app.theme.warning_style()),
        (10, all_one, plain),
        (20, hidden, plain),
        (22, afford, app.theme.warning_style()),
        (28, threads, plain),
        (
            40,
            // Both named, because `s` now cycles within the view and the two
            // can no longer disagree — so saying one without the other leaves
            // the reader guessing which columns the ordering is over.
            match app.view {
                crate::app::View::Generic => format!(" — sort: {}", app.sort.label()),
                v => format!(" — {} view, sort: {}", v.label(), app.sort.label()),
            },
            plain,
        ),
        // Just under the sort it is about, and above the modes: a suggestion a
        // narrow terminal drops is one nobody can act on, but it is still
        // advice rather than a fact about the data.
        (45, constraint, plain),
        (
            50,
            match (app.tree, app.group) {
                (true, _) => " · tree".into(),
                // Named, not just "grouped": the rows say what they fold only
                // if you already know which key is in force, and `g` now has
                // three states rather than two.
                (_, g) if g != crate::app::Grouping::Off => format!(" · {}", g.label()),
                _ => String::new(),
            },
            plain,
        ),
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

    let mut widths = vec![Constraint::Length(6)];
    if show_bars {
        // The bar, plus room for the over-100 mark.
        widths.push(Constraint::Length(BAR_W as u16 + 1));
    }
    widths.push(Constraint::Length(8));
    if show_bars {
        widths.push(Constraint::Length(BAR_W as u16));
    }
    widths.push(Constraint::Length(2));
    if show_thr {
        widths.push(Constraint::Length(4));
    }
    if show_io {
        widths.push(Constraint::Length(9));
        widths.push(Constraint::Length(9));
    }
    if show_mem_cols {
        widths.push(Constraint::Length(8));
        widths.push(Constraint::Length(8));
        widths.push(Constraint::Length(7));
        widths.push(Constraint::Length(8));
    }
    widths.push(Constraint::Length(SPARK_W as u16));
    widths.push(Constraint::Length(7));
    if show_user {
        widths.push(Constraint::Length(USER_W));
    }
    if show_cid {
        // Twelve characters, which is what `docker ps` shows.
        widths.push(Constraint::Length(CID_W));
    }
    widths.push(Constraint::Min(MIN_COMMAND_W));

    f.render_widget(
        Paragraph::new(divider_of(title, area.width, &app.theme)),
        Rect { height: 1, ..area },
    );
    let table = Table::new(rows, widths).header(header);
    f.render_widget(
        table,
        Rect {
            y: area.y + 1,
            height: area.height.saturating_sub(1),
            ..area
        },
    );
}

/// Width of the per-process history sparkline, in cells.
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
    let spc = set.samples_per_cell();
    let slots = SPARK_W * spc;
    let values: Vec<f32> = series.iter().map(|v| v.unwrap_or(0.0)).collect();
    let agg = history::peak_slots(&values, zoom.max(1), slots);
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
    if !app.show_io {
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

fn draw_help(f: &mut Frame, area: Rect, app: &App) {
    let line = if app.editing_filter {
        // The error, where the query is being typed. A one-line filter box has
        // nowhere else to teach the field names, so the message carries them.
        let tail = match app.filter_error() {
            Some(why) => Span::styled(format!("   {why}"), app.theme.warning_style()),
            None => Span::styled("   (Enter/Esc to finish)", app.theme.dim_style()),
        };
        Line::from(vec![
            Span::styled("filter: ", app.theme.cursor_style()),
            Span::raw(&app.filter),
            Span::styled("█", app.theme.cursor_style()),
            tail,
        ])
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
    "←/→ scrub",
    "+/- zoom",
    "Space live",
    "↑/↓ select",
    "s sort",
    "/ filter",
    "t tree",
    "i io",
    "v view",
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
fn fit_hints(hints: &[&str], width: u16) -> String {
    const SEP: &str = " · ";
    let width = width as usize;
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
    out
}
