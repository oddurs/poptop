//! Application state and input handling.

use crate::collect::Needs;
use crate::glyphs::GlyphSet;
use crate::history::History;
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
        match self {
            // Descending for resource columns: the interesting rows go top.
            Sort::Cpu => b.cpu.total_cmp(&a.cpu),
            Sort::Mem => b.rss.cmp(&a.rss),
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

    /// The next sort in the cycle.
    ///
    /// `Disk` is skipped when its column is not being collected, because every
    /// row would answer `None` and the key would order nothing. It is still
    /// reachable there by accepting a suggestion, which only appears when the
    /// figures exist.
    pub fn next(self, io: bool) -> Self {
        let n = match self {
            Sort::Cpu => Sort::Mem,
            Sort::Mem => Sort::Disk,
            Sort::Disk => Sort::Pid,
            Sort::Pid => Sort::Name,
            Sort::Name => Sort::Cpu,
        };
        if n == Sort::Disk && !io { Sort::Pid } else { n }
    }
}

pub struct App {
    pub history: History,
    pub sort: Sort,
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
    pub should_quit: bool,
    pub tree: bool,
    /// Whether the IO columns are shown.
    pub show_io: bool,
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
    pub group: bool,
    /// Whether IO is being collected. Deliberately a ratchet: hiding the
    /// columns does not stop collection, because resuming later would punch a
    /// hole in the middle of history. One clean boundary between "not collected
    /// yet" and "collected" is far easier to reason about while scrubbing than
    /// gaps wherever the column happened to be off.
    io_ratchet: bool,
    /// Index into [`ZOOM_LEVELS`].
    zoom_idx: usize,
    pub glyphs: GlyphSet,
    pub theme: Theme,
    /// Nominal time between samples, for spotting gaps in the buffer.
    pub interval: std::time::Duration,
}

impl App {
    pub fn new(history_len: usize) -> Self {
        Self {
            history: History::new(history_len),
            sort: Sort::Cpu,
            selected: None,
            last_row: std::cell::Cell::new(0),
            filter: String::new(),
            editing_filter: false,
            should_quit: false,
            tree: false,
            // On by default. The header may have just told the user their
            // machine is blocked on IO, and the table is where the culprit is
            // named — a default that hides it makes the default view unable to
            // answer the question the default view raised. Withdrawn after one
            // real sample where most of it turns out to be unreadable; see
            // `probe_io`.
            show_io: true,
            show_kernel: false,
            group: false,
            io_ratchet: true,
            zoom_idx: 0,
            glyphs: GlyphSet::default(),
            theme: Theme::default(),
            interval: DEFAULT_INTERVAL,
        }
    }

    /// What the collector should gather for the next sample.
    pub fn needs(&self) -> Needs {
        Needs {
            io: self.io_ratchet,
        }
    }

    /// Show or hide the IO columns. Showing them starts collection; hiding them
    /// does not stop it. See [`App::io_ratchet`].
    pub fn toggle_io(&mut self) {
        self.show_io = !self.show_io;
        self.io_ratchet |= self.show_io;
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

    /// Zoom out: more time on screen, coarser slots.
    pub fn zoom_out(&mut self) {
        self.zoom_idx = (self.zoom_idx + 1).min(ZOOM_LEVELS.len() - 1);
    }

    /// Zoom in: less time on screen, one slot per sample at the limit.
    pub fn zoom_in(&mut self) {
        self.zoom_idx = self.zoom_idx.saturating_sub(1);
    }

    pub fn push(&mut self, s: Sample) {
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
        let needle = self.filter.to_lowercase();
        let shown = |p: &&ProcSample| self.show_kernel || !p.is_kernel_thread();

        if self.tree {
            // A filtered tree keeps matches plus their ancestors; `tree::build`
            // works out the ancestry, so it only needs the direct matches.
            let matched: Option<HashSet<i32>> = (!needle.is_empty()).then(|| {
                sample
                    .procs
                    .iter()
                    .filter(shown)
                    .filter(|p| matches(p, &needle))
                    .map(|p| p.pid)
                    .collect()
            });
            // Withheld from the tree rather than filtered out of its rows: a
            // hidden kernel thread must not survive as somebody's visible
            // ancestor, and `kthreadd` is the ancestor of every one of them.
            let procs: Vec<&ProcSample> = sample.procs.iter().filter(shown).collect();
            return tree::build(&procs, self.sort, matched.as_ref());
        }

        let mut v: Vec<&ProcSample> = sample
            .procs
            .iter()
            .filter(shown)
            .filter(|p| matches(p, &needle))
            .collect();

        if self.group {
            let mut rows = grouped(&v);
            rows.sort_by(|a, b| self.sort.compare(&a.proc, &b.proc));
            return rows;
        }

        v.sort_by(|a, b| self.sort.compare(a, b));
        v.into_iter().map(TreeRow::of).collect()
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
        let needle = self.filter.to_lowercase();
        self.history.current().map_or(0, |s| {
            s.procs
                .iter()
                .filter(|p| p.is_kernel_thread() && matches(p, &needle))
                .count()
        })
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
            for p in s.procs.iter().filter(shown) {
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
        let i = from.clamp(0, rows.len() as isize - 1) as usize;
        self.selected = Some(Watched::of(&rows[i]));
    }

    /// Where the watched process is in these rows, if it is in them at all.
    pub fn row_of(&self, rows: &[TreeRow<'_>]) -> Option<usize> {
        let w = self.selected.as_ref()?;
        let i = rows.iter().position(|r| w.is(r))?;
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
            Watched::Group { name } => sample.procs.iter().any(|p| *p.name == **name),
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

    fn is(&self, row: &TreeRow<'_>) -> bool {
        match self {
            Watched::Process { pid, started, .. } => {
                !row.is_group() && *pid == row.proc.pid && *started == row.proc.started
            }
            Watched::Group { name } => row.is_group() && **name == *row.proc.name,
        }
    }
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
fn grouped<'a>(procs: &[&'a ProcSample]) -> Vec<TreeRow<'a>> {
    let mut order: Vec<&Arc<str>> = Vec::new();
    let mut by_name: HashMap<&str, Vec<&'a ProcSample>> = HashMap::new();
    for p in procs {
        if by_name.entry(&p.name).or_default().is_empty() {
            order.push(&p.name);
        }
        by_name.get_mut(&*p.name).expect("just inserted").push(p);
    }

    order
        .into_iter()
        .map(|name| {
            let members = &by_name[&**name];
            let first = members[0];
            if members.len() == 1 {
                return TreeRow::of(first);
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
                    pid: 0,
                    ppid: 0,
                    name: name.clone(),
                    user: first.user.clone(),
                    cpu: members.iter().map(|p| p.cpu).sum(),
                    rss: members.iter().map(|p| p.rss).sum(),
                    threads: members.iter().map(|p| p.threads).sum(),
                    state: '—',
                    started: None,
                    cmd: None,
                    io,
                }),
                prefix: String::new(),
                context_only: false,
                members: members.len(),
            }
        })
        .collect()
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

/// A process matches the filter by name, command line, or pid. An empty filter
/// matches all.
///
/// The command line is searched whether or not it is the thing on screen. The
/// question people arrive with is "which of these is the API server", and the
/// answer is in the arguments — so `/server.js` has to find it even when the
/// column is showing `node`.
fn matches(p: &ProcSample, needle: &str) -> bool {
    needle.is_empty()
        || p.name.to_lowercase().contains(needle)
        || p.cmd
            .as_ref()
            .is_some_and(|c| c.to_lowercase().contains(needle))
        || p.pid.to_string().contains(needle)
}
