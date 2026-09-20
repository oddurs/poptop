//! Application state and input handling.

use crate::collect::{Needs, SUPPORTED, Size, Source};
use crate::glyphs::GlyphSet;
use crate::history::History;
use crate::query::{self, Query};
use crate::sample::{IoRates, ProcSample, Sample};
use crate::theme::Theme;
use crate::tree::{self, TreeRow};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// Nominal time between samples.
///
/// Lives here rather than in `main` because the renderer needs it too: telling
/// a gap in the buffer from an ordinary interval is a question about the
/// nominal rate, and two copies of that number would drift the moment item
/// 0013 makes it configurable.
/// Samples the table averages over by default.
///
/// Five, which at the default one-second interval is Activity Monitor's own
/// refresh period — long enough that a row stops twitching and short enough
/// that a process starting is on screen before you have finished reading the
/// row above it.
pub const DEFAULT_SMOOTH: usize = 5;

pub const DEFAULT_INTERVAL: std::time::Duration = std::time::Duration::from_secs(1);

/// Samples per display slot.
///
/// Kept modest deliberately: a slot count of ~200 (braille on a normal
/// terminal) needs only 3-4 samples per slot to cover the whole ten-minute
/// buffer, and a narrow terminal needs ~8. Offering 30 would just give three
/// keypresses that visibly do nothing, since [`effective_zoom`] clamps to what
/// the buffer can actually fill.
pub const ZOOM_LEVELS: [usize; 4] = [1, 2, 4, 8];

/// The zoom actually used to draw, given how much history exists.
///
/// Zooming past the point where the whole buffer is on screen only shrinks the
/// data into a corner, so it is clamped. The empty region to the left of a
/// fully zoomed-out graph is meaningful — it is time from before the buffer
/// starts, not missing data.
pub fn effective_zoom(requested: usize, samples: usize, slots: usize) -> usize {
    if slots == 0 {
        return requested.max(1);
    }
    requested.max(1).min(samples.div_ceil(slots).max(1))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sort {
    Cpu,
    Mem,
    /// Per-process disk throughput, read plus write.
    ///
    /// Only reachable when those columns are being collected: a sort key every
    /// row answers `None` to is not an ordering, it is a shuffle.
    Disk,
    Pid,
    Name,
}

impl Sort {
    /// The name a config file writes, the inverse of [`Sort::label`].
    pub fn parse(name: &str) -> Option<Sort> {
        [Sort::Cpu, Sort::Mem, Sort::Disk, Sort::Pid, Sort::Name]
            .into_iter()
            .find(|s| s.label().eq_ignore_ascii_case(name))
    }

    pub fn label(self) -> &'static str {
        match self {
            Sort::Cpu => "CPU",
            Sort::Mem => "MEM",
            Sort::Disk => "DISK",
            Sort::Pid => "PID",
            Sort::Name => "NAME",
        }
    }

    /// Order two processes under this sort. Shared by the flat table and by
    /// sibling ordering inside the tree, so both agree.
    pub fn compare(self, a: &ProcSample, b: &ProcSample) -> Ordering {
        self.compare_with(&Smoothing::default(), a, b)
    }

    /// The same ordering, over averaged figures.
    ///
    /// Sorting on the smoothed value is the half of smoothing that matters. A
    /// row whose *number* twitches is mildly annoying; a row that swaps places
    /// with its neighbour while you are reading it is the thing that makes a
    /// table unreadable, and that happens on a single noisy sample unless the
    /// ordering is over the average too.
    pub fn compare_with(self, sm: &Smoothing, a: &ProcSample, b: &ProcSample) -> Ordering {
        match self {
            // Descending for resource columns: the interesting rows go top.
            // A process the kernel said nothing about goes after one it did,
            // not among the idle — its zeros were never measured. Ahead of the
            // figures rather than folded into them, so it holds whether the
            // figure compared is the raw one or the average.
            // The figure the row is *showing*, not a second reading of it.
            //
            // These sorted on a settled average for a few hours: the figure
            // ended at the cursor and the ordering on a beat, so the table
            // held still while the numbers stayed live. It put a column marked
            // `▾CPU%` on screen reading 13.4, 5.8, 3.5, 4.2, 3.9, 3.0, 6.6 —
            // sorted by a quantity that was not the one printed (0216).
            //
            // A table is monotonic in the column it says it is sorted by. That
            // contract outranks the calm: a reader who can see the sort is
            // wrong has no way left to tell which of the two numbers behind it
            // to believe, and the calm was bought with the trust that made the
            // ordering worth having.
            //
            // The calm now comes from the average itself, which is what a
            // longer `--smooth` lengthens. Two processes genuinely taking
            // turns will trade rows, and that is them taking turns.
            Sort::Cpu => a
                .unmeasured()
                .cmp(&b.unmeasured())
                .then(sm.cpu(b).total_cmp(&sm.cpu(a))),
            Sort::Mem => a
                .unmeasured()
                .cmp(&b.unmeasured())
                .then(sm.rss(b).cmp(&sm.rss(a))),
            // Unreadable sorts last, not as zero. A process whose IO could not
            // be read is not an idle one, and putting it among the idle ones
            // would be the fabricated zero this codebase refuses everywhere
            // else — here it would quietly hide the busiest process on the box
            // from someone who had just asked to see it.
            Sort::Disk => {
                let rate = |p: &ProcSample| p.io.map(|io| io.read + io.write);
                // Written as `a` against `b` throughout. The first version
                // matched on `(rate(b), rate(a))` to get the descending order
                // for free and then got the `None` arms backwards, sorting
                // unreadable processes to the top.
                match (rate(a), rate(b)) {
                    (Some(x), Some(y)) => y.cmp(&x),
                    (Some(_), None) => Ordering::Less,
                    (None, Some(_)) => Ordering::Greater,
                    (None, None) => Ordering::Equal,
                }
            }
            Sort::Pid => a.pid.cmp(&b.pid),
            // By what the column actually shows. Sorting on `name` while the
            // row renders `command()` produced a NAME column that looked
            // unsorted for exactly the processes this table is now good at
            // telling apart: four node services sort as four identical `node`s.
            Sort::Name => a.command().to_lowercase().cmp(&b.command().to_lowercase()),
        }
    }

    /// The next sort in the cycle, within the columns this view shows.
    ///
    /// Cycling within the view is what keeps the two from disagreeing: sorting
    /// by a column that is not on screen is an ordering the reader cannot see
    /// the reason for, and atop allows exactly that.
    ///
    /// `Disk` is skipped when its column is not being collected, because every
    /// row would answer `None` and the key would order nothing. It is still
    /// reachable there by accepting a suggestion, which only appears when the
    /// figures exist.
    pub fn next(self, io: bool, view: View) -> Self {
        let keys = view.sorts();
        let here = keys.iter().position(|k| *k == self).unwrap_or(0);
        for step in 1..=keys.len() {
            let n = keys[(here + step) % keys.len()];
            if n != Sort::Disk || io {
                return n;
            }
        }
        self
    }
}

/// Samples the thread ratchet keeps collecting after the view is turned off.
///
/// Sixty, so a minute of scrubbing back over what you were just looking at
/// still has threads in it, at the default interval.
const THREAD_GRACE: u32 = 60;

/// The share of one interval collection may take before poptop starts giving
/// things up.
///
/// A quarter. Past that the tool is a meaningful part of the load it is
/// measuring, which is the one thing a monitor must not become on the box that
/// is already in trouble — and that box is the whole reason poptop exists.
const BUDGET_SHARE: f32 = 0.25;

/// Consecutive over-budget samples before anything is given up.
const BUDGET_STRIKES: u32 = 3;

/// Which columns the table shows.
///
/// atop solves the same problem with seven of these — `g` generic, `m` memory,
/// `d` disk, `n` network — each a different column set over the same rows. The
/// table is already at its width on an eighty-column terminal, so more fields
/// cannot mean more columns.
///
/// # The mode axes, resolved rather than multiplied
///
/// Tree, grouping, thread expansion and now views looked like four exclusive
/// modes on four keys, which is where interfaces go wrong. They are two axes:
///
/// - **What the table is *of*** — processes flat, as a tree, folded by name or
///   by container, with a process expanded to its threads, or cgroups instead.
///   `t`, `g`, `y`, `C`.
/// - **What it *shows*** — this. `v`.
///
/// `d` is neither: it changes the *timeline* panel, not the table, and calling
/// it a table mode was the thing that made this look like four axes.
/// The ceiling each kind of graph is being drawn on, and when the data last
/// justified it.
///
/// A byte rate has no natural maximum, so its axis has to follow the data —
/// and an axis that follows the data exactly redraws every sample on screen
/// the moment a burst arrives or leaves. Nothing about the past has changed;
/// only the scale has, and a graph whose history redraws itself is one nobody
/// can read a trend off.
///
/// So the ceiling rises the instant the data needs it, because a clipped graph
/// is not a smaller graph but a wrong one, and falls only once the peak has
/// stayed under it for [`SETTLE`]. A burst no longer leaves a cliff behind it.
///
/// Three slots, one per [`crate::ui::Unit`], doubled: the machine's panels and
/// one process's panels are different subjects and must not inherit each
/// other's scale. Held in a `Cell` because drawing takes `&App` everywhere and
/// this is the one fact about a frame that has to outlive it — a ceiling
/// recomputed from scratch every frame is exactly the flicker being fixed.
#[derive(Clone, Debug, Default)]
pub struct HeldCeilings(std::cell::Cell<[(f32, Option<std::time::SystemTime>); 6]>);

/// How long a graph's ceiling stays up after the data stops needing it.
///
/// Long enough that a burst and the quiet after it are one picture, short
/// enough that a panel does not spend a minute mostly empty once traffic
/// genuinely drops.
pub const SETTLE: std::time::Duration = std::time::Duration::from_secs(15);

impl HeldCeilings {
    /// The ceiling to draw `unit` on, given what this frame's data wants.
    ///
    /// `now` is the newest sample's own timestamp rather than the wall clock,
    /// so the settling is a fact about the recording and a test can state it
    /// without sleeping.
    pub fn settle(
        &self,
        unit: crate::ui::Unit,
        subject: bool,
        want: f32,
        now: std::time::SystemTime,
    ) -> f32 {
        let at = unit.slot() * 2 + usize::from(subject);
        let mut held = self.0.get();
        let (ceiling, since) = held[at];
        let keep = match since {
            // Nothing held yet, or the data has caught up with what is held:
            // take it, and the clock starts again from here.
            _ if want >= ceiling => (want, Some(now)),
            None => (want, Some(now)),
            // Smaller than what is drawn. Hold it until it has been smaller
            // for long enough to be the new shape of things rather than a lull.
            Some(t) => match now.duration_since(t) {
                Ok(d) if d >= SETTLE => (want, Some(now)),
                // A clock that went backwards says nothing about how long
                // anything has been true. Hold rather than guess.
                _ => (ceiling, Some(t)),
            },
        };
        held[at] = keep;
        self.0.set(held);
        keep.0
    }

    /// Forget every held ceiling.
    ///
    /// Scrubbing to another part of the buffer is a deliberate move to a
    /// different span, and carrying the live view's scale into it would draw
    /// that span on an axis chosen by a moment the reader has left.
    pub fn forget(&self) {
        self.0.set(Default::default());
    }
}

/// Which of the memory tab's platform figures this machine actually publishes.
///
/// See [`App::mem_columns_available`]. Growth is not here: poptop computes it
/// from two of its own samples, so it is available wherever the tab is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MemColumns {
    pub pss: bool,
    pub vsize: bool,
    pub majflt: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum View {
    /// What poptop has always shown: CPU, memory, state, threads, history.
    #[default]
    Cpu,
    /// Memory, with the room the other columns were using.
    Memory,
    /// Disk throughput, always — not only when it happens to fit.
    ///
    /// This is the concrete thing views fix today: `DISK R`/`DISK W` are shown
    /// only when there is room, so on a narrow terminal the figures vanish with
    /// nothing to bring them back. This key is what brings them back.
    Disk,
}

impl View {
    pub fn next(self) -> Self {
        match self {
            View::Cpu => View::Memory,
            View::Memory => View::Disk,
            View::Disk => View::Cpu,
        }
    }

    /// Every tab, in the order they are drawn.
    ///
    /// The list rather than a `next` chain: a tab strip has to draw them all,
    /// and deriving the strip by walking `next` until it came back round would
    /// be a loop that only works because the cycle happens to be closed.
    pub const ALL: [View; 3] = [View::Cpu, View::Memory, View::Disk];

    /// The name a config file writes, the inverse of [`View::label`].
    ///
    /// Over `ALL` rather than a list of its own: a fourth tab should not be
    /// addable in one place and unreadable from the config in another.
    pub fn parse(name: &str) -> Option<View> {
        View::ALL
            .into_iter()
            .find(|v| v.label().eq_ignore_ascii_case(name))
    }

    /// The name on the tab.
    ///
    /// Capitalised, because these are now proper nouns on a navigation strip
    /// rather than a word in a sentence in a panel title.
    pub fn label(self) -> &'static str {
        match self {
            View::Cpu => "CPU",
            View::Memory => "Memory",
            View::Disk => "Disk",
        }
    }

    pub fn prev(self) -> Self {
        // Two forward is one back in a cycle of three, and writing it this way
        // means the order lives in one place.
        self.next().next()
    }

    /// Whether the per-process disk columns belong in this view.
    ///
    /// In the disk view they are the point, so they are not subject to the
    /// width test that hides them elsewhere. On the CPU tab they are subject to
    /// it and still present by default, which `io_columns_are_there_before_anyone_asks`
    /// argues for and this deliberately did not disturb: the header can say the
    /// machine is blocked on IO, and the table under it is where the culprit is
    /// named. Making that answer a tab away would be a real loss to save a key.
    pub fn wants_io(self) -> bool {
        matches!(self, View::Cpu | View::Disk)
    }

    /// The sort keys reachable from this view.
    ///
    /// atop keeps sort and view independent, which allows sorting by a column
    /// the view does not show — an ordering the reader cannot see the reason
    /// for. Here `s` cycles within the view, so the two cannot disagree and the
    /// panel can name both without either contradicting the table.
    pub fn sorts(self) -> &'static [Sort] {
        match self {
            View::Cpu => &[Sort::Cpu, Sort::Mem, Sort::Disk, Sort::Pid, Sort::Name],
            View::Memory => &[Sort::Mem, Sort::Cpu, Sort::Pid, Sort::Name],
            View::Disk => &[Sort::Disk, Sort::Cpu, Sort::Pid, Sort::Name],
        }
    }

    /// The sort to fall back to when switching into this view leaves the
    /// current one unreachable.
    ///
    /// `io` is whether the per-process disk figures are being collected. The
    /// disk view's first choice is `Sort::Disk`, and without those figures every
    /// row answers `None` to it — "not an ordering, a shuffle", which is why
    /// [`Sort::next`] already refuses to cycle onto it. Switching views must
    /// not walk in the back door.
    pub fn default_sort_for(self, io: bool) -> Sort {
        let first = self.sorts()[0];
        if first == Sort::Disk && !io {
            return self
                .sorts()
                .iter()
                .copied()
                .find(|s| *s != Sort::Disk)
                .unwrap_or(Sort::Cpu);
        }
        first
    }
}

/// What a row stands for while the table is folding.
type GroupKey = fn(&ProcSample) -> Option<&Arc<str>>;

/// What the table folds rows by, if anything.
///
/// A cycle rather than a second mode. Folding by container is the same
/// machinery as folding by name with a different key, and giving it its own
/// toggle would add a fifth exclusive layout to a table that already has four.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Grouping {
    #[default]
    Off,
    /// Processes sharing a name, which is 0050's behaviour.
    Name,
    /// Processes belonging to one user.
    ///
    /// On a shared box "which user is eating the machine" is the first
    /// question, and the `USER` column cannot answer it: it is folded away
    /// precisely when it is constant, and one column of many rows when it is
    /// not.
    User,
    /// Processes in the same container. Processes in none are not shown: the
    /// question is what each container is doing.
    Container,
}

impl Grouping {
    /// The next state of the `g` cycle.
    pub fn next(self) -> Self {
        match self {
            Grouping::Off => Grouping::Name,
            Grouping::Name => Grouping::User,
            Grouping::User => Grouping::Container,
            Grouping::Container => Grouping::Off,
        }
    }

    /// What each row stands for, or `None` when the table is not folding.
    fn key(self) -> Option<GroupKey> {
        match self {
            Grouping::Off => None,
            Grouping::Name => Some(|p| Some(&p.name)),
            // Not the `?` an unresolvable uid falls back to on macOS. It is a
            // placeholder, not an identity, so folding on it heaps every
            // process whose owner could not be looked up into one row and
            // presents the total as one user's usage — the "heap called none"
            // this function refuses for containers, wearing a name.
            Grouping::User => Some(|p| (&*p.user != "?").then_some(&p.user)),
            Grouping::Container => Some(|p| p.container.as_ref()),
        }
    }

    /// The name a config file writes: `off`, `name`, `user`, `container`.
    pub fn parse(name: &str) -> Option<Grouping> {
        Some(match name {
            "off" | "none" => Grouping::Off,
            "name" => Grouping::Name,
            "user" => Grouping::User,
            "container" => Grouping::Container,
            _ => return None,
        })
    }

    /// The name [`Grouping::parse`] takes.
    pub fn name(self) -> &'static str {
        match self {
            Grouping::Off => "off",
            Grouping::Name => "name",
            Grouping::User => "user",
            Grouping::Container => "container",
        }
    }

    /// What to call it in the panel.
    pub fn label(self) -> &'static str {
        match self {
            Grouping::Off => "",
            Grouping::Name => "grouped by name",
            Grouping::User => "grouped by user",
            Grouping::Container => "grouped by container",
        }
    }
}

/// A column of the process table that a reader may not want.
///
/// Not every column: the command is what a row *is*, and the CPU figure is
/// what the table is sorted by and drawn for. These are the ones that can go
/// without leaving a row that cannot be read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Column {
    Bars,
    Rss,
    State,
    Thr,
    Io,
    Mem,
    Hist,
    Pid,
    User,
    Cid,
}

impl Column {
    /// Every column a config file can hide, in the order the table draws them.
    pub const ALL: [Column; 10] = [
        Column::Bars,
        Column::Rss,
        Column::State,
        Column::Thr,
        Column::Io,
        Column::Mem,
        Column::Hist,
        Column::Pid,
        Column::User,
        Column::Cid,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Column::Bars => "bars",
            Column::Rss => "rss",
            Column::State => "state",
            Column::Thr => "thr",
            Column::Io => "io",
            Column::Mem => "mem",
            Column::Hist => "hist",
            Column::Pid => "pid",
            Column::User => "user",
            Column::Cid => "cid",
        }
    }

    pub fn parse(name: &str) -> Option<Column> {
        Column::ALL
            .into_iter()
            .find(|c| c.name().eq_ignore_ascii_case(name))
    }
}

pub struct App {
    pub history: History,
    pub sort: Sort,
    /// Which columns the table shows. See [`View`].
    pub view: View,
    /// Index into the *sorted* process list of the displayed sample.
    /// The process being watched, not the row it happens to be on.
    ///
    /// A selection that tracks a row number is not a selection of anything. The
    /// table is re-sorted at every sample, so scrubbing back through a spike
    /// moved the highlight from row 3 to row 11 to off-screen while the reader
    /// sat still — and the one gesture this tool exists for, find the moment it
    /// went wrong and watch what that process was doing around it, was the
    /// gesture that lost your place.
    ///
    /// Held as `(pid, started)`, the identity [`ProcSample::key`] exists for.
    /// `started` is `Option` rather than required: a platform that cannot time
    /// a process would otherwise have no selection at all, and following the
    /// wrong process after a pid is recycled is a far smaller harm than never
    /// following one — it moves a highlight, where the same mistake in
    /// `ProcSample::key` would splice two processes' history into one line.
    pub selected: Option<Watched>,
    /// Where the highlight was last seen, as a hint for scrolling and for the
    /// next keypress. Never the selection — that is `selected`, and this is
    /// only ever a fallback for when the watched process is not on screen.
    ///
    /// Without it the viewport snapped home every time the process was
    /// momentarily absent: scrubbing back past the moment it started made the
    /// list jump to the top and back on every arrow key, and a process exiting
    /// cost the reader their place sixty rows down.
    last_row: std::cell::Cell<usize>,
    pub filter: String,
    pub editing_filter: bool,
    /// What the filter was before editing began, so Escape can put it back.
    pub filter_before: String,
    /// Whether the jump box is open, and what has been typed into it.
    ///
    /// An incident has a time. Scrubbing to it by pressing the left arrow six
    /// hundred times is not a workflow, and it is the one thing atop's `-b`
    /// does that poptop had no answer for.
    pub editing_jump: bool,
    /// Whether the buffer holds a recorded day rather than live history.
    ///
    /// Read by the jump: a replayed buffer never receives a live sample, so
    /// resuming the live tail there would leave the header claiming `LIVE` over
    /// a week-old day. In replay the honest landing is the newest *recorded*
    /// sample, pinned.
    pub replaying: bool,
    pub jump: String,
    /// What the last jump did, or why it could not.
    ///
    /// Kept after the box closes: the answer to "was there anything recorded at
    /// 03:00" is the whole point of asking, and a message that vanished with
    /// the prompt would be one nobody read.
    pub jump_note: Option<String>,
    /// Whether poptop may send a signal at all. See `config::Settings::signals`.
    pub signals: bool,
    /// A signal asked for and not yet confirmed.
    pub pending: Option<crate::signal::Pending>,
    /// What the last signal did, or why it did not.
    pub signal_note: Option<String>,
    pub should_quit: bool,
    /// Whether the `?` list of every key is open. See `ui::draw_key_list`.
    pub show_help: bool,
    /// Which key asks for what. The default map is the keys poptop has always
    /// had; a config file can move any of them. See [`crate::keys`].
    pub keys: crate::keys::Keymap,
    pub tree: bool,
    /// Whether the IO columns are shown.
    pub show_io: bool,
    /// Whether the selected process expands into its threads.
    ///
    /// The selected one, not every one: a box has eight times as many threads
    /// as processes, and a table that grew ninefold on a keypress would answer
    /// "which thread is spinning" by making it harder to find anything at all.
    pub show_threads: bool,
    /// Whether the table shows cgroups instead of processes.
    ///
    /// A view, not a column: it is a table of different things. atop makes the
    /// same call with `G`.
    pub show_cgroups: bool,
    /// Show kernel threads — `kworker/*`, `ksoftirqd/*`, `irq/*` — in the
    /// table.
    ///
    /// Off. On a many-core Linux box they outnumber the real processes several
    /// times over, and none of them is what anyone opened a monitor to find.
    /// poptop already excludes them from the IO ratio on the grounds that
    /// including them distorts a figure; the same argument applies to the panel
    /// they were crowding out.
    ///
    /// A no-op on macOS, where [`ProcSample::is_kernel_thread`] is never true.
    pub show_kernel: bool,
    /// Fold processes sharing a name into one row.
    ///
    /// Twelve of the fifteen rows on screen being the same program is the same
    /// waste the folded `USER` column is, one axis over: repetition that costs
    /// space and conveys nothing. Six `ruby` rows are individually unremarkable
    /// and together are 21% of a core and 2.3GB, which is the fact worth
    /// knowing and the one a list of six cannot state.
    ///
    /// A mode, not the default, and mutually exclusive with the tree — bottom
    /// makes the same two choices. Grouping destroys parentage by construction,
    /// so a grouped tree would be a tree of things that are not processes.
    pub group: Grouping,
    /// The theme's name and where it was read from, so `R` and the file
    /// watcher can read it again. `None` for a built-in, which cannot change
    /// under the program.
    pub theme_file: Option<(String, std::time::SystemTime)>,
    /// What to say about the last reload, in the footer's note line.
    pub theme_note: Option<String>,
    /// Columns the reader asked not to see. The table drops them before it
    /// decides what else it has room for, so the width they took goes to the
    /// command rather than to the next column along.
    pub hidden_columns: Vec<Column>,
    /// Show the selected process's own history in place of the machine's.
    ///
    /// The buffer already holds every retained sample's whole process table, so
    /// "what has *this* process been doing" is a question the data can answer
    /// and the interface could not: it was a ten-column sparkline in a table
    /// row, about one percent of the screen for the thing the tool is built
    /// around.
    ///
    /// It replaces the timeline rather than crowding beside it. The timeline is
    /// the panel about time; this is the same question asked of one process, so
    /// it is the same panel with a different subject — and it inherits
    /// scrubbing, zoom, the cursor and the caption by being that rather than by
    /// reimplementing them.
    pub detail: bool,
    /// Whether IO is being collected. Deliberately a ratchet: hiding the
    /// columns does not stop collection, because resuming later would punch a
    /// hole in the middle of history. One clean boundary between "not collected
    /// yet" and "collected" is far easier to reason about while scrubbing than
    /// gaps wherever the column happened to be off.
    io_ratchet: bool,
    /// The same ratchet for threads, but one that eventually lets go.
    ///
    /// Turning the view on starts collecting; turning it off keeps collecting
    /// for [`THREAD_GRACE`] more samples, so scrubbing back over the last
    /// minute still has threads to show.
    ///
    /// The IO ratchet never releases, and that is right for it: one extra read
    /// per process. This costs a directory read per multi-threaded process plus
    /// a file read per thread — 534us to 3.63ms, measured — and about 68 KB of
    /// every retained sample at four thousand threads. Holding that for the
    /// rest of a session because somebody once pressed `y` is a worse bargain
    /// than a gap in history that the panel names.
    thread_ratchet: bool,
    /// Whether this machine has ever reported NUMA nodes.
    ///
    /// Sticky, and read by the layout rather than the sample under the cursor.
    /// Header height derived per-sample looked right and scrubbed badly: on a
    /// NUMA box whose restored history predates this field, every keypress
    /// across the boundary moved the timeline and the whole process table up
    /// and down a row. A tool built around rewinding cannot have the rewind
    /// shift the thing you are reading.
    pub numa: bool,
    /// Samples since the thread view was turned off. Counts only while the
    /// ratchet is still holding.
    thread_idle: u32,
    /// How many samples have been taken, for the sources that are read on a
    /// cadence rather than every time.
    sample_count: u64,
    /// Sources the budget gave up because collection was taking too long.
    ///
    /// Named rather than silent, and not restored automatically: a source that
    /// went over budget once will go over it again, so quietly retrying would
    /// flap between a figure and an em dash. Asking for it again by name clears
    /// it, because that is a decision the reader is entitled to make.
    /// Why the log stopped, if it has.
    ///
    /// On the panel while it is true, not only in the lines printed at exit: a
    /// disk that filled at 10:00 is something the reader needs at 10:00, and a
    /// message they see when they quit is one they see after it stopped
    /// mattering. Same ladder as the withheld sources beside it — an omission
    /// that does not state itself is the one thing this panel never does.
    pub log_note: Option<String>,
    withheld: Vec<Source>,
    /// Consecutive samples that ran over budget. One slow sample is a hiccup;
    /// three in a row is the machine telling you something.
    over_budget: u32,
    /// Sampling is over budget and nothing optional is big enough to be why —
    /// so the interval is too short for this machine, which is a different
    /// message and a different remedy.
    baseline_over: bool,
    /// Index into [`ZOOM_LEVELS`].
    zoom_idx: usize,
    pub glyphs: GlyphSet,
    pub axis: crate::glyphs::Axis,
    /// How much air the layout is given. See `ui::Density`.
    pub density: crate::ui::Density,
    /// Which dropdown is open, if any. See `menu.rs`.
    pub menu: crate::menu::MenuState,
    /// Whether the inspector is open on the selected process. See `ui::draw_inspector`.
    pub inspecting: bool,
    pub theme: Theme,
    /// Nominal time between samples, for spotting gaps in the buffer.
    pub interval: std::time::Duration,
    /// Samples the table's figures are averaged over. 1 is off. See `Smoothing`.
    pub smooth: usize,
    /// The scale each graph is being drawn on. See [`HeldCeilings`].
    pub ceilings: HeldCeilings,
}

impl App {
    pub fn new(history_len: usize) -> Self {
        Self {
            history: History::new(history_len),
            sort: Sort::Cpu,
            view: View::default(),
            selected: None,
            last_row: std::cell::Cell::new(0),
            filter: String::new(),
            editing_filter: false,
            filter_before: String::new(),
            editing_jump: false,
            replaying: false,
            jump: String::new(),
            jump_note: None,
            signals: false,
            pending: None,
            signal_note: None,
            should_quit: false,
            show_help: false,
            keys: crate::keys::Keymap::default(),
            tree: false,
            // On by default. The header may have just told the user their
            // machine is blocked on IO, and the table is where the culprit is
            // named — a default that hides it makes the default view unable to
            // answer the question the default view raised. Withdrawn after one
            // real sample where most of it turns out to be unreadable; see
            // `probe_io`.
            show_io: true,
            show_threads: false,
            show_cgroups: false,
            show_kernel: false,
            group: Grouping::Off,
            detail: false,
            io_ratchet: true,
            thread_ratchet: false,
            numa: false,
            thread_idle: 0,
            sample_count: 0,
            log_note: None,
            withheld: Vec::new(),
            over_budget: 0,
            baseline_over: false,
            zoom_idx: 0,
            hidden_columns: Vec::new(),
            theme_file: None,
            theme_note: None,
            glyphs: GlyphSet::default(),
            axis: crate::glyphs::Axis::default(),
            density: crate::ui::Density::default(),
            menu: crate::menu::MenuState::default(),
            inspecting: false,
            theme: Theme::default(),
            interval: DEFAULT_INTERVAL,
            smooth: DEFAULT_SMOOTH,
            ceilings: HeldCeilings::default(),
        }
    }

    /// What the collector should gather for the next sample.
    pub fn needs(&self) -> Needs {
        let mut n = Needs::at(self.sample_count)
            .with(Source::ClockPolicies)
            // Always asked for, never behind a key. Draining a socket the
            // kernel has already filled costs almost nothing, and the whole
            // value is that the record is there when you scrub back to the
            // spike — a ratchet would mean the burst you are looking for
            // happened before you thought to ask.
            .with(Source::Exited);
        if self.io_ratchet {
            n = n.with(Source::Io);
        }
        if self.thread_ratchet {
            n = n.with(Source::Threads);
        }
        // Only while the view is open. Six files a node against a thousand
        // nodes is not something to collect for a panel nobody is looking at,
        // and unlike threads there is no per-row history to keep continuous —
        // a cgroup's figures are the cgroup's, whenever you ask.
        if self.show_cgroups {
            n = n.with(Source::Cgroups);
        }
        // Only while the memory view is open, for the same reason: one extra
        // read per process is not a cost to pay for a column nobody is looking
        // at. Unlike the thread ratchet there is nothing to keep continuous —
        // a process's share of memory is its share whenever you ask.
        if self.view == View::Memory {
            n = n.with(Source::Pss);
        }
        // Nothing this backend does not read. Otherwise `y` on macOS starts a
        // collection that will never produce a row, and the budget can spend
        // three strikes giving up a source that was costing nothing.
        for s in Source::ALL {
            if !SUPPORTED.contains(&s) {
                n = n.without(s);
            }
        }
        // What the budget gave up. Applied last, so a source poptop stopped
        // reading stays stopped until the reader asks for it again by name.
        for s in &self.withheld {
            n = n.without(*s);
        }
        n
    }

    /// Act on what is in the jump box.
    ///
    /// The whole of it, so the outcome can be tested without a terminal: parse
    /// what was typed, move the cursor, and leave behind a sentence saying what
    /// happened. Landing in a gap says so rather than showing the nearest
    /// sample as though it were the one asked for — poptop draws seams for
    /// intervals it did not observe, and answering a question with the nearest
    /// thing to it is the same lie the seam exists to prevent.
    pub fn jump_to(&mut self, now: std::time::SystemTime) {
        let text = self.jump.trim().to_string();
        // A relative jump is measured from the end of what is retained, not
        // from the wall clock. In a live buffer those are the same moment; in
        // a day opened with `--read` they are a week apart, and `-2h` there
        // means two hours before the end of the day being read rather than two
        // hours before lunchtime today.
        let origin = self.history.newest().map_or(now, |s| s.at);
        self.jump_note = match crate::log::parse_when_noting(&text, origin) {
            Err(why) => Some(why),
            // The clocks changing is said first, and survives whatever the
            // landing has to add: it is about which moment this is at all.
            Ok((at, Some(shift))) => Some(match self.history.seek(at, self.interval) {
                crate::history::Landing::Empty => format!("{shift} — nothing is retained yet"),
                crate::history::Landing::Live if self.replaying => {
                    self.history.goto_newest();
                    format!("{shift} — the end of this day")
                }
                crate::history::Landing::Live => format!("{shift} — live"),
                crate::history::Landing::On => shift,
                crate::history::Landing::Nearest(off) => format!(
                    "{shift} — nothing recorded then, nearest sample is {} away",
                    crate::ui::fmt_lag(off)
                ),
            }),
            Ok((at, None)) => match self.history.seek(at, self.interval) {
                crate::history::Landing::Empty => Some("nothing is retained yet".into()),
                // In a recorded day there is no live tail to resume: the
                // buffer never receives a sample, and `LIVE` over a week-old
                // day would be the worst thing this header could say.
                crate::history::Landing::Live if self.replaying => {
                    self.history.goto_newest();
                    Some(format!("{text} is the end of this day"))
                }
                crate::history::Landing::Live => Some(format!("{text} is now — live")),
                crate::history::Landing::On => None,
                crate::history::Landing::Nearest(off) => Some(format!(
                    "nothing recorded at {text} — nearest sample is {} away",
                    crate::ui::fmt_lag(off)
                )),
            },
        };
    }

    /// Let go of the selected process, and of the view that was about it.
    /// `false` if nothing was selected.
    ///
    /// The one way back to following nothing. The arrow keys set a selection
    /// and nothing cleared it, so a process picked once was followed for the
    /// rest of the run (0102). The per-process history goes with it: it is a
    /// view *of* the selection, and left on it would show the machine's
    /// timeline captioned with an instruction to pick a process.
    ///
    /// Threads stay as they are. `show_threads` is a way of looking at
    /// whichever process is selected next, not a fact about this one.
    pub fn deselect(&mut self) -> bool {
        if self.selected.take().is_none() {
            return false;
        }
        self.detail = false;
        true
    }

    /// Ask to signal the selected process, if signalling is allowed at all.
    ///
    /// Opens a question rather than acting: the number is the part that gets
    /// misread, and poptop knows the name and the command line, so it can name
    /// what it is about to stop.
    ///
    /// Every reason poptop would refuse is checked here as well as at the
    /// moment of sending, so the reader is told before typing `y` rather than
    /// after. A prompt that can only be answered "no" is worse than the key
    /// saying why.
    /// Why a signal would be refused right now, if it would.
    ///
    /// Split out so the affordance and the attempt cannot disagree. An action
    /// bar offering `x quit` on a recorded day, which then refuses it, is worse
    /// than one that never offered it: the reader has already decided before
    /// they find out.
    ///
    /// Returns the reason rather than a sentence, because the two callers need
    /// different lengths of it — a footer shared with the key hints cannot
    /// carry the hundred and twenty characters the note line is written for.
    pub fn signal_refusal(&self) -> Option<Blocked> {
        if !self.signals {
            return Some(Blocked::Off);
        }
        // `replaying` before `is_live`: that method means "the cursor is
        // untethered", which in a day opened with `--read` is true the moment
        // somebody presses `End` — and every check below would then be made
        // against last Tuesday's process table.
        if self.replaying {
            return Some(Blocked::Recorded);
        }
        if !self.history.is_live() {
            return Some(Blocked::Scrubbing);
        }
        None
    }

    pub fn ask_to_signal(&mut self, signal: crate::signal::Signal) {
        self.signal_note = None;
        if !self.signals {
            self.signal_note =
                Some("signals are off — `signals = on` in the config, or --signals=on".into());
            return;
        }
        if let Some(why) = self.signal_refusal() {
            self.signal_note = Some(why.why());
            return;
        }
        let Some(p) = self.selected_process() else {
            // Distinguished, because the remedies differ and one of them is not
            // "press the arrow keys": a folded row *is* selected, and moving to
            // another folded row would say the same thing again.
            self.signal_note = Some(match self.selected {
                Some(Watched::Group { .. }) => {
                    "that row is several processes — `g` again to unfold them".to_string()
                }
                _ => "nothing selected — the arrow keys pick a process".to_string(),
            });
            return;
        };
        let pending = crate::signal::Pending::new(&p, signal);
        // A process poptop cannot identify is one it will never signal, so the
        // question is not worth asking.
        if pending.started.is_none() {
            self.signal_note = Some(crate::signal::Refused::Unidentifiable.why(Some(&pending)));
            return;
        }
        self.pending = Some(pending);
    }

    /// The process under the cursor in the sample under the cursor.
    #[cfg(test)]
    pub fn selected_name_for_test(&self) -> Arc<str> {
        self.selected_process()
            .map(|p| p.name)
            .expect("nothing selected")
    }

    fn selected_process(&self) -> Option<ProcSample> {
        // A *process*, not a group. `g` folds rows together and a folded row is
        // several processes; signalling "the one under the cursor" there would
        // mean picking one of them, and which is not a decision a confirmation
        // could describe.
        let Watched::Process { pid, started, .. } = self.selected.as_ref()? else {
            return None;
        };
        let now = self.history.current()?;
        now.procs
            .iter()
            .find(|p| p.pid == *pid && p.started == *started)
            .cloned()
    }

    /// Answer the question. Anything but `y` cancels.
    ///
    /// Checked again against the *newest* sample rather than the one on
    /// screen: the identity poptop uses everywhere is `(pid, started)`, and it
    /// is what stops a recycled pid being signalled by number.
    pub fn confirm_signal(&mut self, yes: bool) {
        let Some(p) = self.pending.take() else {
            return;
        };
        if !yes {
            self.signal_note = Some(format!("nothing sent to {}", p.name));
            return;
        }
        let live = self.history.newest();
        let checked = crate::signal::check(&p, live, !self.history.is_live(), self.replaying);
        self.signal_note = Some(match checked {
            Err(why) => why.why(Some(&p)),
            Ok(()) => match crate::signal::send(&p) {
                Err(why) => why.why(&p),
                Ok(()) => format!("sent {} to {} (pid {})", p.signal.name(), p.name, p.pid),
            },
        });
    }

    /// Show or hide the IO columns. Showing them starts collection; hiding them
    /// does not stop it. See [`App::io_ratchet`].
    pub fn toggle_io(&mut self) {
        if self.show_io {
            self.show_io = false;
        } else {
            self.reveal_io();
        }
    }

    /// Show the IO columns, and collect what they show.
    pub fn reveal_io(&mut self) {
        self.show_io = true;
        self.io_ratchet = true;
        self.insist(Source::Io);
    }

    /// How much a process's memory grew since the previous sample.
    ///
    /// Derived rather than stored: a growth figure in every retained sample is
    /// a field poptop would carry forever to describe one interval, and it is
    /// already implied by two adjacent samples.
    ///
    /// **Absent across a seam.** The previous sample is the one before the
    /// cursor in the buffer, which is not the same as the one before this
    /// moment in time: a laptop that slept leaves two samples twenty minutes
    /// apart sitting next to each other. "Grew 400 MB since the last sample" is
    /// a rate, and a rate over an unknown interval is not a figure — so it is
    /// an em dash there rather than a number nobody can scale.
    pub fn growth(&self, pid: i32, started: Option<u64>) -> Option<i64> {
        let now = self.history.current()?;
        let before = self.history.previous()?;
        // An interval of zero has no notion of a missed tick, and `gaps_in`
        // says so explicitly — 0013 makes the interval configurable. Without
        // the same guard the timeline would draw no seam while this column
        // showed an em dash on every row, which is the divergence the comment
        // below is about.
        if self.interval.is_zero() {
            return None;
        }
        let gap = now.at.duration_since(before.at).ok()?;
        // The timeline's definition of adjacent, not a second one. `gap_limit`
        // is exposed for exactly this: two definitions would eventually
        // disagree about the same pair of samples, and then the graph would
        // draw a seam where the column showed a number.
        if gap >= crate::history::gap_limit(self.interval) {
            return None;
        }
        let key = |p: &&ProcSample| p.pid == pid && p.started == started;
        let a = now.procs.iter().find(key)?.rss;
        let b = before.procs.iter().find(key)?.rss;
        Some(a as i64 - b as i64)
    }

    /// Ask again for whatever the current view needs collected.
    ///
    /// The budget names what it withholds until somebody asks for it by name,
    /// and for a view's columns the key that asks is the one that opens it.
    /// Ask for whatever this tab needs to answer its own question.
    ///
    /// This is where the `i` key went. It gated the disk columns, which made a
    /// collection decision wear a display key: the figures are expensive to
    /// read, not optional to see, and the tab that exists to show them is the
    /// one that should be paying for them. Choosing Disk *is* asking for disk.
    pub fn insist_for_view(&mut self) {
        match self.view {
            View::Memory => self.insist(Source::Pss),
            View::Disk => self.reveal_io(),
            View::Cpu => {}
        }
    }

    /// Order the table by the resource the tab is named after.
    ///
    /// A tab that says Disk and lists the busiest processes by CPU, each with
    /// a pair of zeroes under DISK R and DISK W, has answered a question
    /// nobody asked. The tab *is* the question; the ordering is half of the
    /// answer, and `s` cycles within the view when it is the wrong half.
    ///
    /// The disk key is reachable on the strength of `show_io`, which
    /// `insist_for_view` has just set, rather than of `io_collected`, which
    /// describes the sample already taken. Otherwise the first press of the
    /// key that exists to ask for disk figures would find none collected yet,
    /// fall back to CPU, and stay there once they arrived.
    pub fn adopt_view_sort(&mut self) {
        self.sort = self
            .view
            .default_sort_for(self.show_io || self.io_collected());
    }

    /// Show cgroups instead of processes, or stop.
    ///
    /// No ratchet, unlike [`App::toggle_threads`]: this genuinely stops
    /// collecting. Six files a node against a thousand nodes is not a cost to
    /// keep paying for a view nobody is looking at, and a cgroup's figures are
    /// the cgroup's whenever you ask — there is no per-row history to keep
    /// continuous.
    pub fn toggle_cgroups(&mut self) {
        self.show_cgroups = !self.show_cgroups;
        if self.show_cgroups {
            self.insist(Source::Cgroups);
        }
    }

    /// Expand the selected process into its threads, or stop.
    ///
    /// Starts collection the first time and keeps it for [`THREAD_GRACE`] more
    /// samples, for the same reason as [`App::toggle_io`]: a reader who turns
    /// the view off, scrubs back, and turns it on again should find the threads
    /// that were there, not a gap shaped like the moment they lost interest.
    pub fn toggle_threads(&mut self) {
        self.show_threads = !self.show_threads;
        self.thread_ratchet |= self.show_threads;
        self.thread_idle = 0;
        if self.show_threads {
            self.insist(Source::Threads);
        }
    }

    /// The share of a sample's processes whose IO could not be read, above
    /// which the columns are not worth showing by default.
    ///
    /// `/proc/<pid>/io` is mode 0400 and owned by the process owner, so reading
    /// other users' processes needs `CAP_SYS_PTRACE`. On a laptop almost every
    /// process is yours and the columns are useful; on a box running its
    /// services as root while you are not, they are a wall of em dashes. Half
    /// is the line because a column that is mostly unreadable is worse than no
    /// column: it costs width, and it invites the reading that those processes
    /// are doing no IO.
    const IO_MOSTLY_DENIED: f32 = 0.5;

    /// Decide from one real sample whether the IO columns earn their place.
    ///
    /// A probe rather than a guess: whether `/proc/<pid>/io` is readable
    /// depends on who is running poptop and who owns the processes, which nothing
    /// short of trying it can answer.
    ///
    /// If they do not, collection stops as well. The ratchet exists so history
    /// has one clean boundary between "not collected" and "collected", and a
    /// probe that answered "no" never really crossed it — continuing to pay
    /// half a millisecond a sample for a column nobody can read would be the
    /// worse trade.
    pub fn probe_io(&mut self, s: &Sample) {
        // A kernel with no per-process IO accounting at all. Every read fails
        // with `NotFound`, which is correctly not a permission problem, and so
        // counts towards nothing — leaving the columns on screen permanently
        // empty with the collector still paying for them.
        if !s.io_supported {
            self.show_io = false;
            self.io_ratchet = false;
            return;
        }

        // Against the processes IO was actually attempted for. Kernel threads
        // are excluded on both sides — they are root-owned and unreadable to
        // an ordinary user, and on a many-core box they outnumber everything
        // else, so including them would fire this on the very laptop it exists
        // to protect.
        let eligible = s.procs.iter().filter(|p| !p.is_kernel_thread()).count();
        if eligible == 0 {
            return;
        }
        let denied = s.io_denied as f32 / eligible as f32;
        if denied > Self::IO_MOSTLY_DENIED {
            self.show_io = false;
            self.io_ratchet = false;
        }
    }

    /// Samples per display slot.
    pub fn zoom(&self) -> usize {
        ZOOM_LEVELS[self.zoom_idx]
    }

    /// Start zoomed to `samples` per slot, for the config file's `zoom`.
    /// A level that is not one of [`ZOOM_LEVELS`] is refused by the setting.
    pub fn set_zoom(&mut self, samples: usize) -> bool {
        match ZOOM_LEVELS.iter().position(|z| *z == samples) {
            Some(i) => {
                self.zoom_idx = i;
                true
            }
            None => false,
        }
    }

    /// Zoom out: more time on screen, coarser slots.
    pub fn zoom_out(&mut self) {
        self.zoom_idx = (self.zoom_idx + 1).min(ZOOM_LEVELS.len() - 1);
    }

    /// Zoom in: less time on screen, one slot per sample at the limit.
    pub fn zoom_in(&mut self) {
        self.zoom_idx = self.zoom_idx.saturating_sub(1);
    }

    pub fn push(&mut self, s: Sample) {
        // Let the thread ratchet go once the view has been off long enough.
        // Counted in samples rather than seconds because the cost is per
        // sample, and because the interval is configurable.
        if self.thread_ratchet && !self.show_threads {
            self.thread_idle = self.thread_idle.saturating_add(1);
            if self.thread_idle > THREAD_GRACE {
                self.thread_ratchet = false;
            }
        }
        self.numa |= s.nodes.is_some();
        self.sample_count = self.sample_count.wrapping_add(1);
        self.history.push(s);
    }

    /// Rows of the displayed sample, filtered and ordered for the table.
    ///
    /// Flat and tree modes return the same row type so the renderer has one
    /// path; a flat row is simply one with an empty prefix.
    pub fn visible_rows(&self) -> Vec<TreeRow<'_>> {
        let Some(sample) = self.history.current() else {
            return Vec::new();
        };
        let (query, _) = filter_of(&self.filter);
        let shown = |p: &&ProcSample| self.show_kernel || !p.is_kernel_thread();
        // Processes that lived and died inside this interval, in the interval
        // that contains them. They are rows like any other — filtered, sorted
        // and searched the same way — and `state` is what marks them.
        let exited: &[ProcSample] = sample.exited.as_deref().unwrap_or(&[]);

        if self.tree {
            // A filtered tree keeps matches plus their ancestors; `tree::build`
            // works out the ancestry, so it only needs the direct matches.
            let matched: Option<HashSet<i32>> = (!query.is_empty()).then(|| {
                sample
                    .procs
                    .iter()
                    .filter(shown)
                    .filter(|p| query.matches_in(p, sample.tasks.as_deref()))
                    .map(|p| p.pid)
                    .collect()
            });
            // Withheld from the tree rather than filtered out of its rows: a
            // hidden kernel thread must not survive as somebody's visible
            // ancestor, and `kthreadd` is the ancestor of every one of them.
            let procs: Vec<&ProcSample> = sample.procs.iter().chain(exited).filter(shown).collect();
            return tree::build(&procs, self.sort, &self.smoothing(), matched.as_ref());
        }

        let mut v: Vec<&ProcSample> = sample
            .procs
            .iter()
            .chain(exited)
            .filter(shown)
            .filter(|p| query.matches_in(p, sample.tasks.as_deref()))
            .collect();

        if self.group != Grouping::Off {
            let mut rows = grouped(&v, self.group);
            let sm = self.smoothing();
            rows.sort_by(|a, b| self.sort.compare_with(&sm, &a.proc, &b.proc));
            // Not spliced: a group row stands for a name, and the threads of
            // one of its members belong under a process, not under a heading
            // that folds several.
            return rows;
        }

        let sm = self.smoothing();
        v.sort_by(|a, b| self.sort.compare_with(&sm, a, b));
        self.with_threads(sample, v.into_iter().map(TreeRow::of).collect())
    }

    /// Expand the selected process into its threads.
    ///
    /// The selected one only. A box has roughly eight times as many threads as
    /// processes, so expanding every row would answer "which thread is
    /// spinning" by making the spinning one harder to find — and would cost the
    /// vertical space the tree and the groups are already competing for.
    ///
    /// Sorted by CPU rather than by the table's sort column, because the column
    /// sorts by things a thread does not have its own copy of. Ties break on
    /// tid so the order is stable frame to frame.
    ///
    /// Flat rows only. The tree orders rows by parentage and draws a spine
    /// through them, so threads spliced between a process and its children
    /// would make the children read as children of the last thread; a group row
    /// stands for a name that folds several processes, and the threads of one
    /// of them belong under a process. [`App::thread_note`] says so on screen.
    fn with_threads<'a>(&self, sample: &'a Sample, mut rows: Vec<TreeRow<'a>>) -> Vec<TreeRow<'a>> {
        if !self.show_threads {
            return rows;
        }
        let Some(Watched::Process { pid, .. }) = self.selected.as_ref() else {
            return rows;
        };
        let pid = *pid;
        // `None` means this sample predates the view being turned on. Nothing
        // is drawn and nothing is invented; the panel title says so.
        let Some(tasks) = sample.tasks.as_ref() else {
            return rows;
        };
        let Some(at) = rows.iter().position(|r| !r.is_group() && r.proc.pid == pid) else {
            return rows;
        };
        let mut mine: Vec<&crate::sample::ThreadSample> =
            tasks.iter().filter(|t| t.pid == pid).collect();
        mine.sort_by(|a, b| b.cpu.total_cmp(&a.cpu).then(a.tid.cmp(&b.tid)));

        let base = rows[at].prefix.clone();
        let proc = rows[at].proc.clone();
        let last = mine.len().saturating_sub(1);
        for (i, t) in mine.iter().enumerate() {
            rows.insert(
                at + 1 + i,
                TreeRow {
                    proc: proc.clone(),
                    prefix: format!("{base}{} ", if i == last { "└─" } else { "├─" }),
                    context_only: false,
                    members: None,
                    thread: Some((*t).clone()),
                },
            );
        }
        rows
    }

    /// Why an expanded process is showing no threads, if it is.
    ///
    /// A process expanded to nothing looks exactly like a process with one
    /// thread. The two are different answers, and this is which.
    pub fn thread_note(&self) -> Option<&'static str> {
        if !self.show_threads {
            return None;
        }
        // Both modes claim the same vertical space and both order rows by
        // something other than "this process, then its threads" — the tree by
        // parentage, groups by a name that folds several processes. Saying so
        // rather than doing nothing: a key that silently has no effect is the
        // ambiguity this whole note exists to remove.
        if self.group != Grouping::Off {
            return Some("threads: not shown while grouped");
        }
        if self.tree {
            return Some("threads: not shown in the tree");
        }
        match self.history.current() {
            None => None,
            Some(s) if s.tasks.is_some() => None,
            // Before the two below, because on this platform the next sample
            // will not have them either and neither will any sample further
            // back — so both of those messages would be promises poptop cannot
            // keep here.
            //
            // Not "macOS cannot": the mach `task_threads` call would answer
            // this, and sysinfo — the backend poptop uses here — simply does
            // not expose it. Saying what is true rather than what is
            // convenient, so nobody reads this as a kernel limitation.
            Some(_) if !SUPPORTED.contains(&Source::Threads) => {
                Some("threads: not read on this platform")
            }
            // Collection starts with the *next* sample, so for one interval
            // after the key the newest sample has no threads. Distinguished
            // from a scrub, because "not collected this far back" sends a
            // reader who is already at the live edge scrolling forward, and
            // nothing they can do there will help.
            Some(_) if self.history.is_live() => Some("threads: from the next sample"),
            // Scrubbed back past the moment the view was turned on. The
            // ratchet means this can only ever be a prefix of the buffer.
            Some(_) => Some("threads: not collected this far back"),
        }
    }

    /// What collection cost, and whether to stop doing some of it.
    ///
    /// The item offers two shapes: the panel that needs a source asks for it
    /// (pull), or a budget turns things off when the sample runs long (push).
    /// This is both, because they answer different questions — pull decides
    /// what is *worth* gathering and push decides what the machine can *afford*
    /// — and the objection to a budget is not that it is wrong but that it can
    /// silently drop a figure. So it never does: everything it gives up is
    /// named, in the panel, until the reader asks for it again.
    ///
    /// Dropped one at a time, most expensive first, and only after three
    /// consecutive over-budget samples. A single slow sample is a hiccup — a
    /// page fault, a scheduler decision, another process finishing — and
    /// withdrawing a column for one of those would be its own kind of noise.
    pub fn spent(&mut self, took: std::time::Duration, interval: std::time::Duration) {
        let budget = interval.mul_f32(BUDGET_SHARE);
        if took <= budget {
            self.over_budget = 0;
            self.baseline_over = false;
            return;
        }
        self.over_budget += 1;
        if self.over_budget < BUDGET_STRIKES {
            return;
        }
        self.over_budget = 0;

        // What the optional sources could plausibly account for, against what
        // has to be explained. Everything mandatory — the process table walk,
        // `/proc/stat`, the net and disk counters — is in `took` too, and none
        // of it can be given up.
        //
        // Without this check a box whose *baseline* exceeds the budget drops
        // IO, stays over, drops threads, stays over, and is left permanently
        // missing both with a banner blaming them for something neither did.
        let excess = took.saturating_sub(budget).as_nanos() as u64;
        let size = self.size();
        match self
            .needs()
            .costliest(size)
            .filter(|s| s.total_nanos(size) * 2 >= excess)
        {
            Some(worst) => {
                // The view goes with it. Leaving it on would put "not collected
                // here" — the wording for a platform that never collects it —
                // beside a note saying poptop stopped, and would make the first
                // press of the key turn the view *off* rather than ask for the
                // source back.
                match worst {
                    Source::Io => self.show_io = false,
                    Source::Threads => self.show_threads = false,
                    Source::Cgroups => self.show_cgroups = false,
                    // Both views fall back to the generic one, so the state
                    // stays consistent: a view whose defining column is no
                    // longer collected would be a panel of em dashes.
                    Source::Pss => self.view = View::Cpu,
                    // Neither has a view to turn off: exit records go into the
                    // table beside live rows, and the clock ceiling is a header
                    // figure. The withheld clause is what says they stopped.
                    Source::Exited | Source::ClockPolicies => {}
                }
                self.withheld.push(worst);
            }
            // Nothing optional is big enough to be the reason. Saying so, with
            // the thing that would actually help, rather than dismantling the
            // tool one column at a time in pursuit of a target it cannot reach.
            None => self.baseline_over = true,
        }
    }

    /// Roughly how big this machine is, from the last sample.
    fn size(&self) -> Size {
        let s = self.history.iter().last();
        Size {
            procs: s.map_or(0, |s| s.procs.len() as u64),
            tasks: s.map_or(0, |s| s.tasks.as_ref().map_or(0, Vec::len) as u64),
            exited: s.map_or(0, |s| s.exited.as_ref().map_or(0, Vec::len) as u64),
            cgroups: s.map_or(0, |s| s.cgroups.as_ref().map_or(0, Vec::len) as u64),
        }
    }

    /// Whether sampling is over budget for reasons nothing optional explains.
    pub fn baseline_over_budget(&self) -> bool {
        self.baseline_over
    }

    /// Sources the budget gave up, for the panel to say so.
    pub fn withheld(&self) -> &[Source] {
        &self.withheld
    }

    /// Ask for a source again after the budget gave it up.
    ///
    /// The reader insisting. If it goes over budget again it will be given up
    /// again, which is the honest outcome: poptop is telling them the machine
    /// cannot afford it at this interval, and `--interval` is the answer.
    fn insist(&mut self, s: Source) {
        self.withheld.retain(|w| *w != s);
    }

    /// The container id to show, if the column is worth its width.
    ///
    /// Dropped when no process on screen is in one — on a box running no
    /// containers the column would be nine columns of em dash. The same rule
    /// as `one_user`, and the reason a process in no container says nothing
    /// rather than showing a blank.
    pub fn any_container(&self) -> bool {
        self.history
            .current()
            .is_some_and(|s| s.procs.iter().any(|p| p.container.is_some()))
    }

    /// Which of the memory tab's platform figures anybody actually reports.
    ///
    /// The same rule as [`App::any_container`] and [`App::one_user`], applied
    /// to the three columns whose contents come from the kernel rather than
    /// from poptop. macOS publishes none of them and Linux publishes
    /// proportional memory only where the process is allowed to read another's
    /// `smaps_rollup`, so on most machines this is three columns of em dash —
    /// twenty-five columns spent saying nothing, next to an RSS figure the
    /// table was truncating for want of them.
    pub fn mem_columns_available(&self) -> MemColumns {
        let Some(s) = self.history.current() else {
            return MemColumns::default();
        };
        MemColumns {
            pss: s.procs.iter().any(|p| p.pss.is_some()),
            vsize: s.procs.iter().any(|p| p.vsize.is_some()),
            majflt: s.procs.iter().any(|p| p.majflt.is_some()),
        }
    }

    /// Kernel threads withheld from the table right now.
    ///
    /// Said out loud in the panel title. Every other omission in poptop states
    /// itself — an idle interface, a device that has done no IO, a filesystem
    /// with no blocks — and a table quietly two hundred rows shorter than the
    /// process count above it would be the one that did not.
    pub fn hidden_kernel_threads(&self) -> usize {
        if self.show_kernel {
            return 0;
        }
        // Filtered, like the count it sits beside. Counting every kernel
        // thread in the sample made `processes (1) · 250 kernel hidden` while a
        // filter for `nginx` was active, implying two hundred and fifty rows
        // were withheld from a list that had one candidate.
        let (query, _) = filter_of(&self.filter);
        self.history.current().map_or(0, |s| {
            s.procs
                .iter()
                .filter(|p| p.is_kernel_thread() && query.matches(p))
                .count()
        })
    }

    /// The watched process's own history over a window of samples.
    ///
    /// `None` when nothing is selected, or when the process appears nowhere in
    /// the window — a panel about a process that was never here has nothing to
    /// draw, and an empty graph would say it was idle.
    ///
    /// Every row scales to its own peak, which the graph already does per row,
    /// so a thread count and a percentage sit in the same panel without either
    /// pretending to be the other. Disk is in MB/s for the same reason: the
    /// axis prints its ceiling as a bare number, and `4` is a scale a reader
    /// can hold where `4194304` is not.
    pub fn watched_series(&self, window: &[&Sample]) -> Option<WatchedSeries> {
        let w = self.selected.as_ref()?;
        let total_mem = window.last()?.mem.total.max(1) as f32;

        // Summed inline rather than through `grouped`, which builds a hash map
        // and an order vector to answer a question about one name. This runs
        // once per sample in the window — up to the whole buffer — on every
        // frame, and a codebase that interns `Arc<str>` to save far less should
        // not allocate six hundred hash maps to redraw a panel.
        let find = |s: &Sample| -> Option<Member> {
            match w {
                Watched::Process { pid, started, .. } => s
                    .procs
                    .iter()
                    .find(|p| p.pid == *pid && p.started == *started)
                    .map(Member::of),
                Watched::Group { name } => {
                    let mut it = s.procs.iter().filter(|p| *p.name == **name);
                    let mut m = Member::of(it.next()?);
                    for p in it {
                        m.add(p);
                    }
                    Some(m)
                }
            }
        };

        let seen: Vec<Option<Member>> = window.iter().map(|s| find(s)).collect();
        if seen.iter().all(Option::is_none) {
            return None;
        }
        let absent: Vec<bool> = seen.iter().map(Option::is_none).collect();

        // A figure the platform would not give is a gap in that row, never a
        // zero. `threads` is `None` on macOS for processes this user does not
        // own, and *every* process has no `io` in the first sample it appears
        // in — there is no previous counter to diff against — so plotting zero
        // would put a false floor under the leftmost cell of every panel.
        let row = |name: &'static str,
                   unit: crate::ui::Unit,
                   f: &dyn Fn(&Member) -> Option<f32>|
         -> DetailRow {
            DetailRow {
                name,
                unit,
                values: seen
                    .iter()
                    .map(|m| m.as_ref().and_then(f).unwrap_or(0.0))
                    .collect(),
                unknown: seen
                    .iter()
                    .map(|m| m.as_ref().is_some_and(|m| f(m).is_none()))
                    .collect(),
            }
        };

        use crate::ui::Unit;
        let mut rows = vec![
            row("CPU", Unit::Percent, &|m| Some(m.cpu)),
            row("MEM", Unit::Percent, &|m| {
                Some(m.rss as f32 / total_mem * 100.0)
            }),
        ];
        // Only where some sample could answer at all. A row of pure gap is
        // worse than a shorter panel.
        if seen.iter().flatten().any(|m| m.threads.is_some()) {
            rows.push(row("THR", Unit::Count, &|m| m.threads.map(|n| n as f32)));
        }
        if seen.iter().flatten().any(|m| m.io.is_some()) {
            // Bytes a second, in its own unit. It used to be pre-divided into
            // megabytes so that a bare ceiling would read as a scale a person
            // could hold; a series that carries its unit does not need the
            // trick.
            rows.push(row("DISK", Unit::Rate, &|m| m.io.map(|b| b as f32)));
        }
        Some(WatchedSeries { rows, absent })
    }

    /// What is wrong with the filter, if anything.
    ///
    /// Surfaced where the filter is typed. A query language nobody knows the
    /// keywords for is worse than a substring match, and on a one-line filter
    /// box the error is the only place discovery can happen.
    pub fn filter_error(&self) -> Option<String> {
        filter_of(&self.filter).1
    }

    /// Whether the displayed sample carries per-process disk figures.
    pub fn io_collected(&self) -> bool {
        self.history.current().is_some_and(|s| s.io_collected)
    }

    /// The resource stopping work on the machine at the cursor, if one is.
    ///
    /// atop sorts its list by whichever resource is currently constrained —
    /// "automatic sort on the most utilized resource" — so that when the disk
    /// is the bottleneck the table is already sorted by disk. Saturation over
    /// utilisation is already this tool's layout thesis; the header is ordered
    /// by how much each figure answers "why is this slow", and this is the same
    /// judgement applied one panel down.
    ///
    /// Read at the cursor, not live. Scrubbing back to a spike to find out what
    /// was constrained *then* is the whole reason the buffer exists, and a
    /// suggestion about the present moment would be answering a question nobody
    /// asked.
    ///
    /// Stall pressure first where the kernel publishes it, because it is a
    /// direct measure of *which* resource is stopping work rather than which is
    /// merely busy — a disk at 100% utilisation that nothing is waiting on is
    /// not a constraint. Utilisation is the fallback, and on macOS the only
    /// thing available.
    ///
    /// Held over a window for the same reason the folded column is: a
    /// constraint that flickers between two resources would flicker the
    /// suggestion, and a panel that changes its advice twice a second is worse
    /// than one that gives none.
    pub fn constraint(&self) -> Option<Constraint> {
        let window: Vec<&Sample> = self.history.around(Self::CONSTANT_FOR).collect();
        // A hold that shrinks near the start of the buffer is not a hold: one
        // sample agrees with itself, so a single spike a second after launch
        // named a constraint and the advice changed on every frame for the
        // first four seconds — the flicker this window exists to prevent.
        if window.len() < Self::CONSTANT_FOR {
            return None;
        }

        // Swap that is *growing* is memory pressure being paid for, and unlike
        // a headroom figure it is a measurement. It is the only memory signal
        // macOS can stand behind: `MemStat::available` there comes from
        // overlapping `vm_stat` quantities that routinely sum to more than the
        // machine has, so a headroom rule reads as roomy on a box that is
        // thrashing — the one case it exists for. Checked across the window
        // rather than in one sample because a constant two gigabytes of swap is
        // an idle Mac and says nothing.
        let (first, last) = (window.first()?, window.last()?);
        // The rate, where the platform gives one — but held across the window
        // like everything else here, not read off the last sample.
        //
        // Any Linux box with a non-zero swappiness pages an idle daemon out now
        // and then, so a single frame of ordinary reclaim would flip the
        // suggestion to memory, re-sort the table, and flip back on the next
        // frame. That is the flicker this whole function is built to prevent,
        // and it would also have let four kilobytes a second preempt a
        // sustained, genuine CPU constraint.
        if window.iter().all(|s| s.swout.is_some_and(|v| v > 0)) {
            return Some(Constraint::Memory);
        }
        // The inference, still, for a platform that publishes no rate. Across a
        // window rather than in one sample, because a constant two gigabytes of
        // swap is an idle Mac and says nothing.
        if last.mem.swap_total > 0 && last.mem.swap_used > first.mem.swap_used {
            return Some(Constraint::Memory);
        }

        let mut agreed: Option<Constraint> = None;
        for s in &window {
            match (constraint_of(s), agreed) {
                (Some(c), None) => agreed = Some(c),
                (Some(c), Some(prev)) if c == prev => {}
                // Two samples in the window disagree, or one had no constraint
                // at all. Either way there is nothing steady to suggest.
                _ => return None,
            }
        }
        // A sort that orders nothing is not an answer. On a box where most of
        // `/proc/<pid>/io` is unreadable the probe withdraws those columns for
        // good, and PSI goes on reporting io stall — so the panel offered
        // `disk is the constraint`, `S` set a sort where every figure is
        // `None`, the table did not move, and the title named a column that
        // was not even drawn.
        agreed.filter(|c| *c != Constraint::Disk || self.io_collected())
    }

    /// The interface the header and the NET graph follow: the one that carried
    /// the most over the last [`Self::NET_WINDOW`] samples, never loopback.
    ///
    /// Chosen over a window, not per sample. Per sample, the header named
    /// `lo0` one second and `en0` the next, and the graph was worse — one line
    /// spliced from whichever interface won each sample, so a spike could be
    /// one interface's and the trough beside it another's (0108). Over a
    /// minute, the choice moves only when another interface has really taken
    /// over. Ending at the cursor, like every other window here, so scrubbing
    /// back follows the interface that was busy then.
    ///
    /// Ties go to the interface listed first in the newest sample: on an idle
    /// machine every interface is at zero, and naming whichever sorted last
    /// reads as a claim about which one poptop is watching.
    pub fn headline_link(&self) -> Option<std::sync::Arc<str>> {
        let mut totals: Vec<(&std::sync::Arc<str>, u64)> = Vec::new();
        for s in self.history.window(Self::NET_WINDOW) {
            for l in s.net.iter().flat_map(|n| &n.links) {
                if l.is_loopback() {
                    continue;
                }
                match totals.iter_mut().find(|(n, _)| **n == l.name) {
                    Some((_, t)) => *t = t.saturating_add(l.bytes()),
                    None => totals.push((&l.name, l.bytes())),
                }
            }
        }
        let mut best: Option<(&std::sync::Arc<str>, u64)> = None;
        for (n, t) in totals {
            if best.is_none_or(|(_, b)| t > b) {
                best = Some((n, t));
            }
        }
        best.map(|(n, _)| n.clone())
    }

    /// Samples the headline interface is chosen over: a minute at the default
    /// interval — long enough not to flicker, short enough to follow a change.
    pub const NET_WINDOW: usize = 60;

    /// The program crowding the table, if one is: the name with the most
    /// processes in the displayed sample, when there are at least
    /// [`Self::CROWD`] of them and the table is neither grouped nor a tree.
    ///
    /// For offering `g`, never for acting on it (0112). Across the sample
    /// rather than the rows on screen, so scrolling does not make the offer
    /// come and go.
    pub fn crowding(&self) -> Option<(std::sync::Arc<str>, usize)> {
        if self.tree || self.group != Grouping::Off {
            return None;
        }
        let s = self.history.current()?;
        let mut counts: Vec<(&std::sync::Arc<str>, usize)> = Vec::new();
        for p in s
            .procs
            .iter()
            .filter(|p| self.show_kernel || !p.is_kernel_thread())
        {
            match counts.iter_mut().find(|(n, _)| **n == p.name) {
                Some((_, c)) => *c += 1,
                None => counts.push((&p.name, 1)),
            }
        }
        let (name, n) = counts.into_iter().max_by_key(|(_, c)| *c)?;
        (n >= Self::CROWD).then(|| (name.clone(), n))
    }

    /// How many processes of one name make a table crowded: five, which is a
    /// fifth of a normal terminal's rows.
    pub const CROWD: usize = 5;

    /// The one user every process belongs to, if there is only one.
    ///
    /// `USER` was measured at ten columns — more than `CPU%` — to repeat the
    /// word `oddurs` twelve times, while `COMMAND`, which differs on every row
    /// and is how a reader identifies anything, took what was left and elided.
    /// poptop already filters *rows* by measurement rather than by name: a
    /// device appears once it has done IO, an interface once it has carried a
    /// byte. The same test applies to columns. A column whose values are all
    /// identical is telling you one fact, and a fact belongs in a sentence.
    ///
    /// Read over a window of samples, not just the displayed one. One
    /// short-lived `root` process would otherwise take the column away and give
    /// it back a second later, and a layout that moves under the reader is
    /// worse than the waste it saves. The window makes the two directions
    /// asymmetric, which is the useful shape: a second user expands the column
    /// on the frame they appear, and it takes [`Self::CONSTANT_FOR`] quiet
    /// samples to collapse again.
    ///
    /// Unfiltered on purpose. The filter changes with every keystroke, and
    /// relaying out the table as someone types into it is the same flicker one
    /// step removed.
    pub fn one_user(&self) -> Option<std::sync::Arc<str>> {
        let mut only: Option<&std::sync::Arc<str>> = None;
        let mut any = false;
        // The same rows the table draws. Scanning only user processes while `K`
        // is showing two hundred root-owned kworkers put `· all alice` above
        // rows that were not alice's — the panel making a claim that is false
        // about the lines directly under it.
        let shown = |p: &&ProcSample| self.show_kernel || !p.is_kernel_thread();
        for s in self.history.window(Self::CONSTANT_FOR) {
            // An owner the kernel would not name is not a second user. On
            // macOS every process this user may not read comes back as `?` —
            // two hundred of them on an ordinary Mac — so the column never
            // folded, and ten columns said `oddurs` twenty times beside a
            // command elided for want of them (0111). Those rows already say
            // they could not be read, in every figure; the title says how many.
            for p in s.procs.iter().filter(shown).filter(|p| !p.owner_unknown()) {
                any = true;
                match only {
                    None => only = Some(&p.user),
                    Some(u) if **u == *p.user => {}
                    Some(_) => return None,
                }
            }
        }
        any.then(|| only.cloned()).flatten()
    }

    /// Processes in the displayed sample whose owner the kernel would not name,
    /// among those the table shows. See [`Self::one_user`].
    pub fn unknown_owners(&self) -> usize {
        self.history.current().map_or(0, |s| {
            s.procs
                .iter()
                .filter(|p| self.show_kernel || !p.is_kernel_thread())
                .filter(|p| p.owner_unknown())
                .count()
        })
    }

    /// How many samples a column must have been constant over before its width
    /// is taken away.
    ///
    /// Five, which is five seconds at the default interval — long enough that a
    /// process starting and exiting does not move the layout, short enough that
    /// the width arrives while it is still wanted.
    pub const CONSTANT_FOR: usize = 5;

    pub fn select_delta(&mut self, delta: isize) {
        let rows = self.visible_rows();
        // A filter matching nothing says nothing about the watched process —
        // it is still running. Clearing the selection here meant one reflexive
        // arrow key during a mistyped filter destroyed it, and clearing the
        // filter came back with nothing selected.
        if rows.is_empty() {
            return;
        }
        // From where the watched process is *now*. If it is not in this sample
        // there is no row to move relative to, so a keypress starts at the top
        // rather than jumping to wherever the highlight was last seen.
        // With something selected, move relative to where it is. With nothing
        // selected, the first keypress lands on a row rather than one step past
        // it — otherwise a walk down the list can never reach row zero, and a
        // loop looking for the process there never terminates.
        let from = match self.row_of(&rows) {
            Some(i) => i as isize + delta,
            None if self.selected.is_none() => 0,
            // Selected but off screen: resume from where it was last seen.
            None => self.resume_row() as isize + delta,
        };
        let mut i = from.clamp(0, rows.len() as isize - 1) as usize;
        // A thread row carries its process's identity, so selecting one would
        // re-select the process and leave the cursor exactly where it started —
        // an arrow key that visibly does nothing. Step past them in the
        // direction of travel.
        let step = if delta < 0 { -1 } else { 1 };
        while rows[i].is_thread() {
            let next = i as isize + step;
            if next < 0 || next >= rows.len() as isize {
                break;
            }
            i = next as usize;
        }
        self.selected = Some(Watched::of(&rows[i]));
    }

    /// Select the row at `i`, if there is one.
    ///
    /// The mouse's version of `select_delta`: it points at a row rather than a
    /// direction. Thread rows are skipped the same way and for the same reason
    /// — a thread carries its process's identity, so selecting one would select
    /// the process and leave the highlight where it started.
    pub fn select_row(&mut self, i: usize) {
        let rows = self.visible_rows();
        if rows.is_empty() {
            return;
        }
        let mut i = i.min(rows.len() - 1);
        while rows[i].is_thread() && i > 0 {
            i -= 1;
        }
        self.selected = Some(Watched::of(&rows[i]));
    }

    /// Average every process's figures over the last `smooth` samples ending at
    /// the cursor.
    ///
    /// Computed per frame rather than cached. It is a fold over at most a few
    /// hundred processes across a handful of samples, and a cache would have to
    /// be invalidated by every one of `History`'s six cursor movements — which
    /// is exactly the kind of fact-derived-twice this interface keeps being
    /// bitten by.
    ///
    /// Ends *at the cursor*, not at the live edge: scrubbed to 14:32, the table
    /// shows what those processes were doing around 14:32, which is the only
    /// reading that agrees with the timeline beside it.
    /// Where the averaging window ends: at the cursor, always.
    ///
    /// It used to end on a *boundary* while live — `at - (at % smooth)` — so
    /// that the figures, and therefore the ordering, held still between one
    /// boundary and the next. The calm was real and the price was not worth
    /// it. With a five-second window a process that ran at 90% for three
    /// seconds showed `5.0` throughout, because the block it was averaging had
    /// ended before the spike began; then, once the spike was over, the table
    /// read `56.0` for five seconds — a figure the process had at no point.
    /// Five rows of a number that is not true, under a graph drawing the spike
    /// at the second it happened.
    ///
    /// Staleness is the wrong price for calm, because calm is not what a stale
    /// figure buys: what has to hold still is the *order*, and the order is
    /// held still by averaging the value the ordering is over — which
    /// `Sort::compare_with` does, on a key coarse enough that two rows a
    /// breath apart do not trade places. The number itself is then free to be
    /// live, and it is the number the reader is reading.
    ///
    /// The rows themselves have always come from the newest sample. Quantising
    /// those too was tried and is wrong for the same reason twice over: a
    /// process that had just started would not be listed for five seconds, and
    /// "what is running now" is the question the table exists to answer.
    fn table_index(&self) -> Option<usize> {
        if self.history.len() == 0 {
            return None;
        }
        Some(self.history.cursor_index())
    }

    /// The averaging window in samples, for a span asked for in seconds.
    ///
    /// The buffer is counted in samples and the setting is stated in seconds,
    /// because the interval is itself a setting: "five seconds" has to mean the
    /// same thing at either end of it. One sample is the floor, and one sample
    /// is what "off" is — an average of a single reading is that reading.
    pub fn smooth_samples(&self, span: std::time::Duration) -> usize {
        if self.interval.is_zero() {
            return 1;
        }
        (span.as_secs_f64() / self.interval.as_secs_f64())
            .round()
            .max(1.0) as usize
    }

    /// Average the table's figures over this much time.
    pub fn set_smooth(&mut self, span: std::time::Duration) {
        self.smooth = self.smooth_samples(span);
    }

    pub fn smoothing(&self) -> Smoothing {
        let window = self.smooth;
        if window <= 1 {
            return Smoothing::default();
        }
        // One window, ending at the cursor — live or scrubbed, the figures
        // describe the moment on screen and agree with the graph beside them.
        //
        // It was briefly two: this one, and a second ending on a beat that the
        // ordering was done over, so the rows held still while the numbers
        // stayed live. That is a real thing to want and this was the wrong way
        // to buy it, because it put two readings of one quantity on screen at
        // once — a column headed `▾CPU%` that did not descend (0216). What
        // calms the order is the averaging itself, and how much of it there is
        // is already a setting.
        let at = self.table_index().unwrap_or(0);
        let first = (at + 1).saturating_sub(window);
        let len = at + 1 - first;

        // Weighted towards now, not a flat mean over the window.
        //
        // A flat mean has two edges and both are visible. It answers late — a
        // spike is a fifth of the figure on the second it starts — and it
        // answers *again* when the spike falls off the far end of the window,
        // a whole window later, as a step down to a number nothing caused. The
        // reader sees a change with no event under it.
        //
        // A weight that halves with age has neither edge. The newest sample is
        // about a third of the figure, so a spike moves the number at once and
        // visibly; the oldest is about a tenth, so its leaving moves it by
        // almost nothing. The same span, spent on a curve instead of a wall:
        // "the last five seconds, mostly the last two".
        //
        // Half-life is half the window, which is the only choice here that
        // does not need a setting of its own: it keeps the span the reader
        // asked for as the span that matters, and puts the weight where the
        // question is.
        let half_life = (window as f32 / 2.0).max(0.5);
        let mut sums: HashMap<(i32, Option<u64>), Weighted> = HashMap::new();
        for (i, s) in self.history.iter().skip(first).take(len).enumerate() {
            // Age in samples, counted back from the newest in the window.
            let age = (len - 1 - i) as f32;
            let w = 0.5_f32.powf(age / half_life);
            for p in s.procs.iter().chain(s.exited.as_deref().unwrap_or(&[])) {
                sums.entry((p.pid, p.started)).or_default().add(p, w);
            }
        }
        Smoothing {
            by_key: sums
                .into_iter()
                .filter_map(|(k, acc)| Some((k, acc.finish()?)))
                .collect(),
        }
    }

    /// Where the watched process is in these rows, if it is in them at all.
    pub fn row_of(&self, rows: &[TreeRow<'_>]) -> Option<usize> {
        let w = self.selected.as_ref()?;
        // Skipping thread rows: they carry their process's `proc` so they sort
        // and file under it, which would otherwise make the first *thread* of
        // the selected process match before the process itself.
        let i = rows.iter().position(|r| !r.is_thread() && w.is(r))?;
        self.last_row.set(i);
        Some(i)
    }

    /// Where to look when the watched process is not on screen.
    ///
    /// A scroll position, not a selection: nothing is highlighted at this row,
    /// and moving from it re-keys onto whatever process is actually there.
    pub fn resume_row(&self) -> usize {
        self.last_row.get()
    }

    /// The watched process, when it is not in the rows on screen.
    ///
    /// A held place would be a lie about which row is which, and silently
    /// selecting whatever is at that index is the bug this replaced. Absence is
    /// information — often *the* information: a process that appears partway
    /// through the buffer is what the reader scrubbed back to find out about.
    pub fn watched_but_absent(&self, rows: &[TreeRow<'_>]) -> Option<&Watched> {
        let w = self.selected.as_ref()?;
        if rows.iter().any(|r| w.is(r)) {
            return None;
        }
        // Not in the rows is not the same as not in the sample. A filter or the
        // kernel-thread toggle takes rows away too, and `nginx not running
        // here` above a running nginx is the panel making a claim that is false
        // about the machine — the one thing it is careful never to do. Those
        // cases need no message anyway: the reader typed the filter, and it is
        // on screen.
        let sample = self.history.current()?;
        // A group is present whenever any of its members is: it has no identity
        // of its own beyond the name they share.
        let here = match w {
            Watched::Process { pid, started, .. } => sample
                .procs
                .iter()
                .any(|p| p.pid == *pid && p.started == *started),
            // Through the *grouping's* key, not the process name. A group's
            // name is whatever it folds on — a username, a container id — so
            // matching it against `p.name` finds nothing the moment the key is
            // not the name, and the panel says `alice not running here` about a
            // user who is running plenty. Precisely the false claim about the
            // machine this function exists to avoid.
            Watched::Group { name } => match self.group.key() {
                Some(key) => sample
                    .procs
                    .iter()
                    .any(|p| key(p).is_some_and(|k| k == name)),
                // Not grouping any more, so there is no group to be absent.
                None => true,
            },
        };
        (!here).then_some(w)
    }
}

/// What the table is following.
///
/// A group is followed by name rather than by `(pid, started)`, because it has
/// neither: its figures are a sum and its membership changes as processes come
/// and go. Following the name is the only thing that stays true across that,
/// and it is what the reader picked — they selected `ruby`, not one of six
/// interchangeable rubies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Watched {
    Process {
        pid: i32,
        started: Option<u64>,
        /// What the table called it when it was chosen, so the panel can say
        /// who is missing when it is missing.
        ///
        /// [`ProcSample::command`], not `name`: on Linux `name` is the kernel's
        /// fifteen-character `comm`, so a reader who selected the row
        /// `node /srv/api/server.js` was told `node not running here` — which
        /// on a box with four node services identifies nothing. That is exactly
        /// the failure the command-line column was added to fix, reintroduced
        /// one line over.
        name: Arc<str>,
    },
    Group {
        name: Arc<str>,
    },
}

/// Why signalling is refused right now.
///
/// The reason rather than a sentence: the footer says `while scrubbing` beside
/// what else can be done, and the note line says the whole thing when somebody
/// tries anyway. Two lengths, one decision about whether.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Blocked {
    Off,
    Recorded,
    Scrubbing,
}

impl Blocked {
    /// Four words, for a row that is shared with the key hints.
    pub fn short(self) -> &'static str {
        match self {
            Blocked::Off => "signals off",
            Blocked::Recorded => "a recorded day",
            Blocked::Scrubbing => "while scrubbing",
        }
    }

    /// The whole reason, for the row that has nothing else on it.
    pub fn why(self) -> String {
        match self {
            Blocked::Off => {
                "signals are off — `signals = on` in the config, or --signals=on".to_string()
            }
            Blocked::Recorded => crate::signal::Refused::Recorded.why(None),
            Blocked::Scrubbing => crate::signal::Refused::Scrubbing.why(None),
        }
    }
}

/// Each process's figures, averaged over the last few samples.
///
/// A process table read at one sample a second is mostly noise: a row's CPU
/// figure swings from 3 to 40 and back, and — far worse for reading it — the
/// rows swap places while your eye is on them. Activity Monitor answers this by
/// refreshing every five seconds. poptop keeps every second and averages what
/// it shows, which is the same calm with none of the delay: a spike still
/// happens at the second it happened, and the timeline still draws it.
///
/// **Mean here, and peak in the timeline.** That looks like a contradiction and
/// is the opposite: the timeline is where a spike must be found, so averaging
/// it away would be a lie; the table is a thing you *read*, and the spike is on
/// the graph directly above it. Each aggregation matches the question its panel
/// answers.
///
/// Absences are skipped rather than counted as zero. A process that was not
/// running for three of the five samples was not idle for them, and dividing by
/// five would report a figure it never had.
#[derive(Default)]
pub struct Smoothing {
    by_key: HashMap<(i32, Option<u64>), Averaged>,
}

/// One process's running total while [`App::smoothing`] folds the window.
#[derive(Clone, Copy, Default)]
struct Weighted {
    cpu: f32,
    rss: f64,
    weight: f32,
}

impl Weighted {
    fn add(&mut self, p: &ProcSample, w: f32) {
        self.cpu += p.cpu * w;
        self.rss += p.rss as f64 * f64::from(w);
        self.weight += w;
    }

    /// The averages, or `None` for a process that collected no weight at all.
    ///
    /// Divided by the weight this process actually collected, not by the
    /// window's. A process that was not running for three of the five samples
    /// was not idle for them, and dividing by all five would report a figure
    /// it never had.
    fn finish(self) -> Option<Averaged> {
        if self.weight <= 0.0 {
            return None;
        }
        Some(Averaged {
            cpu: self.cpu / self.weight,
            rss: (self.rss / f64::from(self.weight)) as u64,
        })
    }
}

#[derive(Clone, Copy, Default)]
struct Averaged {
    cpu: f32,
    rss: u64,
}

impl Smoothing {
    /// The averaged CPU of this process, or its own figure when there is no
    /// average to use.
    pub fn cpu(&self, p: &ProcSample) -> f32 {
        self.by_key
            .get(&(p.pid, p.started))
            .map_or(p.cpu, |a| a.cpu)
    }

    pub fn rss(&self, p: &ProcSample) -> u64 {
        self.by_key
            .get(&(p.pid, p.started))
            .map_or(p.rss, |a| a.rss)
    }

    /// Whether anything is being averaged, for the panel to say so.
    pub fn is_on(&self) -> bool {
        !self.by_key.is_empty()
    }
}

impl Watched {
    fn of(row: &TreeRow<'_>) -> Self {
        if row.is_group() {
            return Watched::Group {
                name: row.proc.name.clone(),
            };
        }
        Watched::Process {
            pid: row.proc.pid,
            started: row.proc.started,
            name: Arc::from(row.proc.command()),
        }
    }

    /// What to call it in a sentence.
    pub fn name(&self) -> &Arc<str> {
        match self {
            Watched::Process { name, .. } | Watched::Group { name } => name,
        }
    }

    /// Whether this sample's process is the one being watched.
    ///
    /// Keyed on pid *and* start time, like `is` above and like `series_for`: on
    /// pid alone a number reused after an exit splices two unrelated processes
    /// into one.
    pub fn matches(&self, p: &crate::sample::ProcSample) -> bool {
        match self {
            Watched::Process { pid, started, .. } => p.pid == *pid && p.started == *started,
            Watched::Group { name } => **name == *p.name,
        }
    }

    fn is(&self, row: &TreeRow<'_>) -> bool {
        match self {
            Watched::Process { pid, started, .. } => {
                !row.is_group() && *pid == row.proc.pid && *started == row.proc.started
            }
            Watched::Group { name } => row.is_group() && **name == *row.proc.name,
        }
    }
}

/// Sum a field across a group, or `None` if any member cannot answer.
///
/// Not a sum of what happens to be there: a total missing one member's
/// contribution is a smaller number presented as a complete one, which is the
/// shape of a wrong answer rather than an absent one.
/// The same, for a narrow counter that could overflow.
///
/// `u32` of faults a second is enough for any one process and not for a group
/// of hundreds. Saturating rather than wrapping: a number pinned at the top of
/// its range is visibly wrong, and a wrapped one is quietly small.
fn sum_rate(members: &[&ProcSample], f: impl Fn(&ProcSample) -> Option<u32>) -> Option<u32> {
    members
        .iter()
        .map(|p| f(p))
        .try_fold(0u32, |acc, v| Some(acc.saturating_add(v?)))
}

fn sum_of<T: std::iter::Sum<T> + Copy>(
    members: &[&ProcSample],
    f: impl Fn(&ProcSample) -> Option<T>,
) -> Option<T> {
    members
        .iter()
        .map(|p| f(p))
        .collect::<Option<Vec<T>>>()
        .map(|v| v.into_iter().sum())
}

/// Fold processes sharing a name into one row each.
///
/// What sums and what does not is the whole design:
///
/// - **CPU, RSS and threads sum.** They are quantities of the same thing, and
///   the sum is the fact the six separate rows could not state.
/// - **State does not.** Four sleeping and two running is not a state, so a
///   group has none. `—`, the same mark every other unknowable figure here
///   uses.
/// - **IO sums, unless any member could not be read.** A group whose total
///   omits an unreadable member is a smaller number presented as a complete
///   one, which is the fabricated zero this codebase refuses everywhere else.
/// - **Start time and history do not exist.** A group's history is not the sum
///   of its members': membership changes as processes come and go, so a line
///   drawn through it would be continuity that never happened. `started` is
///   `None`, which is also what stops [`ProcSample::key`] from producing a
///   sparkline for it.
/// - **The command line does not.** Six rubies were started six different ways;
///   the shared `comm` is the only thing true of all of them. Grouping on the
///   name rather than the command line is deliberate for the same reason —
///   grouping by command line would fold nothing, because the arguments are
///   what differ.
fn grouped<'a>(procs: &[&'a ProcSample], by: Grouping) -> Vec<TreeRow<'a>> {
    let Some(key) = by.key() else {
        return Vec::new();
    };
    let mut order: Vec<&Arc<str>> = Vec::new();
    let mut by_name: HashMap<&str, Vec<&'a ProcSample>> = HashMap::new();
    for p in procs {
        // A process the key does not apply to is dropped, not folded into a
        // heap called "none". Grouping by container asks "what is each
        // container doing", and a bucket holding every process on the box that
        // is not in one answers a different question loudly.
        let Some(k) = key(p) else { continue };
        if by_name.entry(k).or_default().is_empty() {
            order.push(k);
        }
        by_name.get_mut(&**k).expect("just inserted").push(p);
    }

    order
        .into_iter()
        .map(|name| {
            let members = &by_name[&**name];
            let first = members[0];
            // A lone process keeps its own figures — its state, its command
            // line, its history are all real and there is nothing to reconcile
            // — but it is still a *group row*, standing for a name. Returning
            // an ordinary row here made the selection vanish the moment a pool
            // shrank to one: the row stopped being a group, and a group
            // selection stopped matching it.
            // Only when the key *is* the process's own name. Grouping by
            // container, the key is the container — so borrowing the single
            // member's row labels it `node` and puts it in a table beside rows
            // labelled `9a1f0e4c2b7d`, where the one container holding one
            // process cannot be told from anything else. `Watched::Group`
            // follows that label too, so the selection would jump to an
            // unrelated row the moment membership changed.
            if members.len() == 1 && by == Grouping::Name {
                return TreeRow {
                    members: Some(1),
                    ..TreeRow::of(first)
                };
            }
            let io = members
                .iter()
                .map(|p| p.io)
                .try_fold(IoRates::default(), |acc, io| {
                    io.map(|io| IoRates {
                        read: acc.read + io.read,
                        write: acc.write + io.write,
                    })
                });
            TreeRow {
                proc: std::borrow::Cow::Owned(ProcSample {
                    // For ordering only, and never drawn — the column shows the
                    // count instead. The lowest member's, so sorting by PID
                    // puts a group where its oldest process would be; a
                    // placeholder zero sorted every group above every process
                    // regardless of what was in it.
                    pid: members.iter().map(|p| p.pid).min().unwrap_or(0),
                    ppid: 0,
                    name: name.clone(),
                    // Not `first.user`. Three rubies owned by alice and three by
                    // bob are not alice's, and taking whichever sorted first
                    // renders a fact the group does not have — the same reason
                    // `state` is an em dash.
                    // Grouping by user, every member shares it by definition —
                    // and it is the row's identity, so the check below would
                    // reach the same answer the long way round.
                    user: match by == Grouping::User || members.iter().all(|p| p.user == first.user)
                    {
                        true => first.user.clone(),
                        false => Arc::from("—"),
                    },
                    cpu: members.iter().map(|p| p.cpu).sum(),
                    // An upper bound, not a measurement. Forked workers share
                    // an interpreter heap copy-on-write, and this counts those
                    // pages once per member, so six workers reading 2.3GB is
                    // more than the kernel has committed for them. Summed
                    // anyway because the shape of the answer — this pool is
                    // large — is what the six separate rows could not give.
                    //
                    // The `PSS` column beside it is the measurement: a shared
                    // page divided among the processes sharing it, so the same
                    // six workers sum to what they actually cost. It needs
                    // `smaps_rollup`, a read per process and Linux only, which
                    // is why it is the memory view's column and not this one.
                    rss: members.iter().map(|p| p.rss).sum(),
                    threads: members.iter().map(|p| p.threads).sum(),
                    state: '—',
                    started: None,
                    cmd: None,
                    io,
                    // Grouping by name, the members can be in different
                    // containers — or none — so there is no one answer and the
                    // column shows none. Grouping by container, every member
                    // shares it by definition, and blanking it would empty the
                    // column on exactly the rows whose container is known.
                    container: match by {
                        Grouping::Container => Some(name.clone()),
                        _ => None,
                    },
                    // Faults and virtual size sum across the group the way RSS
                    // does; `nice` does not, because a group of processes with
                    // different niceness has no one niceness — the same reason
                    // `state` is an em dash here.
                    // Saturating, unlike the others: these are per-second
                    // rates in a `u32`, and a name-group of hundreds of
                    // heavily-faulting processes overflows it — a panic in
                    // debug and a small wrong number in release.
                    minflt: sum_rate(members, |p| p.minflt),
                    majflt: sum_rate(members, |p| p.majflt),
                    vsize: sum_of(members, |p| p.vsize),
                    nice: match members.iter().all(|p| p.nice == first.nice) {
                        true => first.nice,
                        false => None,
                    },
                    // Unlike RSS, this one sums *correctly*: a shared page is
                    // divided among the processes sharing it, so six renderers
                    // do not count it six times. It is the fix for the caveat
                    // the grouped RSS carries.
                    pss: sum_of(members, |p| p.pss),
                }),
                prefix: String::new(),
                context_only: false,
                members: Some(members.len()),
                thread: None,
            }
        })
        .collect()
}

/// One row of a process's history.
pub struct DetailRow {
    pub name: &'static str,
    pub values: Vec<f32>,
    /// What the figures are measured in, which decides the axis and whether the
    /// machine's warn and critical percentages mean anything to them.
    pub unit: crate::ui::Unit,
    /// Samples where the process was there and this figure was not readable.
    /// Drawn as a gap, never as a zero.
    pub unknown: Vec<bool>,
}

/// One process's history across a window of samples.
pub struct WatchedSeries {
    pub rows: Vec<DetailRow>,
    /// Which samples the process was not in. Drawn as a gap, never as zero.
    pub absent: Vec<bool>,
}

/// What one row of the detail panel needs from a process, or from every process
/// sharing a name.
///
/// A flattened `ProcSample` rather than the thing itself, so a group is summed
/// once as it is walked instead of being rebuilt through `grouped`.
struct Member {
    cpu: f32,
    rss: u64,
    threads: Option<u32>,
    /// Read plus write. `None` if any member could not be read — the same
    /// refusal `grouped` makes, for the same reason.
    io: Option<u64>,
}

impl Member {
    fn of(p: &ProcSample) -> Self {
        Self {
            cpu: p.cpu,
            rss: p.rss,
            threads: p.threads,
            io: p.io.map(|io| io.read + io.write),
        }
    }

    fn add(&mut self, p: &ProcSample) {
        self.cpu += p.cpu;
        self.rss += p.rss;
        self.threads = self.threads.zip(p.threads).map(|(a, b)| a + b);
        self.io = self.io.zip(p.io).map(|(a, b)| a + b.read + b.write);
    }
}

/// A resource that is stopping work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Constraint {
    Cpu,
    Memory,
    Disk,
}

impl Constraint {
    /// What to call it in a sentence.
    pub fn name(self) -> &'static str {
        match self {
            Constraint::Cpu => "cpu",
            Constraint::Memory => "memory",
            Constraint::Disk => "disk",
        }
    }

    /// The sort that answers it.
    pub fn sort(self) -> Sort {
        match self {
            Constraint::Cpu => Sort::Cpu,
            Constraint::Memory => Sort::Mem,
            Constraint::Disk => Sort::Disk,
        }
    }
}

/// How much of the last ten seconds *every* runnable task spent stalled before
/// a resource counts as the constraint.
///
/// `full`, not `some`: a busy machine has something waiting on something
/// constantly and healthily, and `some` above zero is the normal state of a
/// working box. `full` means nothing could proceed, which is the thing worth
/// naming. Five percent is half a second in every ten — the same threshold the
/// stall figures are coloured against.
const STALL_CONSTRAINED: f32 = 5.0;

/// The resource stopping work in one sample.
///
/// Split from the window so it can be tested against a fixture rather than
/// against whatever this machine happens to be doing, the same split
/// `parse_meminfo` and the rest have.
pub fn constraint_of(s: &Sample) -> Option<Constraint> {
    // Where the kernel publishes stall pressure, it answers the question
    // directly and the utilisation figures are not consulted at all.
    if let Some(p) = &s.pressure {
        // Least important first: `max_by` keeps the *last* of equal maxima, so
        // this order is what makes a tie fall the way the fallback below ranks
        // them — a disk with no idle time outranks a busy CPU, because a busy
        // CPU is often the machine working. Written the other way round it
        // silently reported CPU whenever two stalls matched.
        let worst = [
            (p.cpu.full, Constraint::Cpu),
            (p.memory.full, Constraint::Memory),
            (p.io.full, Constraint::Disk),
        ]
        .into_iter()
        .filter(|(v, _)| *v >= STALL_CONSTRAINED)
        .max_by(|a, b| a.0.total_cmp(&b.0));
        return worst.map(|(_, c)| c);
    }

    // No pressure figures: fall back to saturation. Ordered by how much each
    // answers "why is this slow" rather than by size, which is why a disk with
    // no idle time outranks a busy CPU — the CPU being busy is often the
    // machine working, and the disk having nothing left is not.
    if s.busiest_disk().is_some_and(|d| d.util >= DISK_CONSTRAINED) {
        return Some(Constraint::Disk);
    }
    // Memory is deliberately not decided here. The only platform that reaches
    // this path is the one whose `available` this codebase documents as not
    // being a partition of `total`, so a headroom test on it would read as
    // roomy on a thrashing box. It is answered from the window instead, by
    // whether swap is growing — see `App::constraint`.
    if s.cpu_total >= CPU_CONSTRAINED {
        return Some(Constraint::Cpu);
    }
    None
}

/// Utilisation at which a device has effectively no idle time left.
const DISK_CONSTRAINED: f32 = 90.0;

/// Aggregate CPU at which the processor is the thing in the way.
///
/// Higher than the header's `critical`, because "busy" and "constrained" are
/// different claims: a box at 80% is working, and one at 95% has nothing left
/// to give.
const CPU_CONSTRAINED: f32 = 95.0;

/// The parsed filter, and the reason it could not be parsed.
///
/// A malformed query filters *nothing* away and says what is wrong. Hiding rows
/// because a query was mistyped is the worst of both outcomes: the reader
/// cannot see what they were looking for, and cannot see why.
fn filter_of(text: &str) -> (Query, Option<String>) {
    match query::parse(text) {
        Ok(q) => (q, None),
        Err(why) => (Query::default(), Some(why)),
    }
}
