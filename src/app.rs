//! Application state and input handling.

use crate::collect::Needs;
use crate::glyphs::GlyphSet;
use crate::history::History;
use crate::sample::{ProcSample, Sample};
use crate::theme::Theme;
use crate::tree::{self, TreeRow};
use std::cmp::Ordering;
use std::collections::HashSet;

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

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Sort {
    Cpu,
    Mem,
    Pid,
    Name,
}

impl Sort {
    pub fn label(self) -> &'static str {
        match self {
            Sort::Cpu => "CPU",
            Sort::Mem => "MEM",
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
            Sort::Pid => a.pid.cmp(&b.pid),
            Sort::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        }
    }

    pub fn next(self) -> Self {
        match self {
            Sort::Cpu => Sort::Mem,
            Sort::Mem => Sort::Pid,
            Sort::Pid => Sort::Name,
            Sort::Name => Sort::Cpu,
        }
    }
}

pub struct App {
    pub history: History,
    pub sort: Sort,
    /// Index into the *sorted* process list of the displayed sample.
    pub selected: usize,
    pub filter: String,
    pub editing_filter: bool,
    pub should_quit: bool,
    pub tree: bool,
    /// Whether the IO columns are shown.
    pub show_io: bool,
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
            selected: 0,
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

        if self.tree {
            // A filtered tree keeps matches plus their ancestors; `tree::build`
            // works out the ancestry, so it only needs the direct matches.
            let matched: Option<HashSet<i32>> = (!needle.is_empty()).then(|| {
                sample
                    .procs
                    .iter()
                    .filter(|p| matches(p, &needle))
                    .map(|p| p.pid)
                    .collect()
            });
            return tree::build(&sample.procs, self.sort, matched.as_ref());
        }

        let mut v: Vec<&ProcSample> = sample
            .procs
            .iter()
            .filter(|p| matches(p, &needle))
            .collect();
        v.sort_by(|a, b| self.sort.compare(a, b));
        v.into_iter()
            .map(|p| TreeRow {
                proc: p,
                prefix: String::new(),
                context_only: false,
            })
            .collect()
    }

    pub fn select_delta(&mut self, delta: isize) {
        let n = self.visible_rows().len();
        if n == 0 {
            self.selected = 0;
            return;
        }
        let next = self.selected as isize + delta;
        self.selected = next.clamp(0, n as isize - 1) as usize;
    }

    /// Keep the selection in range after the list shrinks (filter, or a process
    /// exiting between samples).
    pub fn clamp_selection(&mut self) {
        let n = self.visible_rows().len();
        if n == 0 {
            self.selected = 0;
        } else if self.selected >= n {
            self.selected = n - 1;
        }
    }
}

/// A process matches the filter by name or by pid. An empty filter matches all.
fn matches(p: &ProcSample, needle: &str) -> bool {
    needle.is_empty()
        || p.name.to_lowercase().contains(needle)
        || p.pid.to_string().contains(needle)
}
