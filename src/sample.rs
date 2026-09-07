//! Core data model.
//!
//! Everything the collectors produce is a `Sample`: a complete, self-contained
//! snapshot of the machine at one instant, including the full process list.
//! Keeping processes *inside* the sample is what makes scrubbing backwards
//! possible — the process table you see at t-40s is the real one from t-40s,
//! not an interpolation.

use std::sync::Arc;
use std::time::SystemTime;

/// Memory figures, all in bytes.
#[derive(Debug, Clone, Copy, Default)]
pub struct MemStat {
    pub total: u64,
    /// Memory actually in use (total minus available), the number people mean
    /// when they ask "how much RAM is this box using".
    pub used: u64,
    pub available: u64,
    /// Genuinely unused, where the platform can say. `available` minus this is
    /// reclaimable cache — memory the kernel is holding but will hand back
    /// under pressure.
    ///
    /// The distinction is the whole reason to draw memory as a composition
    /// rather than a percentage: "37% used" reads the same on a box with eight
    /// gigabytes free and on one whose only headroom is page cache it is about
    /// to have to drop.
    ///
    /// `None` on macOS, and that is not laziness. There, `used` and `available`
    /// come from overlapping `vm_stat` quantities and routinely sum to more
    /// than the machine has — 20.0G used plus 11.5G available on a 24G box —
    /// so there is no partition to draw. `free_memory()` is no help either: it
    /// is `free - speculative` with a saturating subtract, which on any warm
    /// machine is simply zero. A split derived from those would report "no free
    /// memory, all headroom is reclaimable cache" on a perfectly healthy box,
    /// which is the exact alarming misreading this figure exists to prevent.
    pub free: Option<u64>,
    pub swap_total: u64,
    pub swap_used: u64,
}

impl ProcSample {
    /// Whether this is a kernel thread rather than a program.
    ///
    /// Everything under `kthreadd` — `kworker/*`, `ksoftirqd`, `irq/*` — plus
    /// `kthreadd` itself. On a many-core box these outnumber the real
    /// processes, they are all owned by root, and none of them has a
    /// `/proc/<pid>/io` an ordinary user can read.
    ///
    /// That matters because counting them as "denied" would fire the IO probe
    /// on exactly the laptop it exists to protect: a machine where every
    /// process the user cares about is readable, and every process they do not
    /// is a kernel thread.
    ///
    /// A Linux notion. On macOS pid 2 is an ordinary process, so one process in
    /// several hundred is wrongly excluded from the IO ratio there — which
    /// changes no decision this figure is used for.
    /// How this process is followed from one sample to the next.
    ///
    /// `None` when the platform would not give a start time. A caller with no
    /// key must not fall back to the pid alone: pids are recycled, and a
    /// recycled pid is precisely the case that produces a graph made of two
    /// different programs. Better a process with no history than a history
    /// belonging to something else.
    pub fn key(&self) -> Option<(i32, u64)> {
        Some((self.pid, self.started?))
    }

    pub fn is_kernel_thread(&self) -> bool {
        self.pid == KTHREADD || self.ppid == KTHREADD
    }
}

/// `kthreadd`, the parent of every kernel thread, is always pid 2 on Linux.
const KTHREADD: i32 = 2;

impl MemStat {
    /// Reclaimable cache: counted as available, but not free. `None` wherever
    /// [`MemStat::free`] is.
    pub fn cache(&self) -> Option<u64> {
        Some(self.available.saturating_sub(self.free?))
    }

    /// The memory bar's segments — used, cache, free — and whether the middle
    /// one means anything.
    ///
    /// Two parts where the platform cannot separate cache from free, three
    /// where it can. Both partitions sum to `total`, so the bar and the
    /// percentage beside it can never disagree.
    pub fn composition(&self) -> ([u64; 3], bool) {
        match self.cache() {
            Some(cache) => (
                [
                    self.used,
                    cache,
                    self.total.saturating_sub(self.used).saturating_sub(cache),
                ],
                true,
            ),
            // Used against the rest of the machine. Less to say, but nothing
            // said that is not known.
            None => ([self.used, 0, self.total.saturating_sub(self.used)], false),
        }
    }

    pub fn used_pct(&self) -> f32 {
        if self.total == 0 {
            return 0.0;
        }
        (self.used as f32 / self.total as f32) * 100.0
    }

    pub fn swap_pct(&self) -> f32 {
        if self.swap_total == 0 {
            return 0.0;
        }
        (self.swap_used as f32 / self.swap_total as f32) * 100.0
    }
}

/// Disk throughput for one process over one interval, in bytes per second.
#[derive(Debug, Clone, Copy, Default)]
pub struct IoRates {
    pub read: u64,
    pub write: u64,
}

/// One process as it appeared in a single sample.
#[derive(Debug, Clone)]
pub struct ProcSample {
    pub pid: i32,
    /// Retained for the process-tree view; nothing reads it yet.
    #[allow(dead_code)]
    pub ppid: i32,
    /// Shared, like `user`. A process name is re-read every sample and almost
    /// never changes; at 400 processes over a 600-sample buffer, allocating it
    /// afresh each time cost 240,000 allocations and 3.8 MB for strings that
    /// were all identical.
    pub name: Arc<str>,
    /// Shared: a few distinct users repeat across every process in every
    /// retained sample, so the ring buffer holds one allocation each, not one
    /// per row per second.
    pub user: Arc<str>,
    /// Percent of one core. Can exceed 100 for threaded processes.
    pub cpu: f32,
    /// Resident set size in bytes.
    pub rss: u64,
    pub threads: u32,
    pub state: char,
    /// An opaque token, unique to one run of one process on this machine.
    ///
    /// Only ever compared for equality, never interpreted: the two backends
    /// count entirely different things, and nothing is gained by pretending
    /// otherwise.
    ///
    /// - Linux: clock ticks since boot, from `/proc/<pid>/stat` field 22.
    /// - macOS: microseconds since the epoch, from `sysctl(KERN_PROC_ALL)`.
    ///
    /// Because the units differ, a token means nothing outside the machine and
    /// boot that produced it — see [`crate::store::boot_time`], which refuses
    /// to restore a buffer across a reboot for exactly this reason.
    ///
    /// `None` where the platform will not say. Not zero: zero is a value that
    /// compares equal to another zero, so two unrelated processes sharing a
    /// recycled pid would be spliced into one line — the failure this field
    /// exists to prevent. See [`ProcSample::key`].
    pub started: Option<u64>,
    /// `None` means no figure is available — either extended collection was off
    /// when this sample was taken, or the process could not be read. The two
    /// cases are told apart by [`Sample::io_collected`], and neither is ever
    /// rendered as a zero: a fabricated zero is indistinguishable from a
    /// genuinely idle process.
    pub io: Option<IoRates>,
}

/// A complete snapshot of the machine at one instant.
#[derive(Debug, Clone)]
pub struct Sample {
    pub at: SystemTime,
    /// Aggregate CPU busy percentage, 0..100.
    pub cpu_total: f32,
    /// Per-core busy percentage, 0..100 each.
    pub cpu_per_core: Vec<f32>,
    /// Share of the interval the CPU spent idle *with I/O outstanding*.
    ///
    /// Separate from `cpu_total` on purpose. iowait is idle time — the CPU had
    /// nothing to run — so folding it into busy would overstate CPU on exactly
    /// the machine that needs reading most carefully. Reporting it alongside
    /// is what distinguishes "this box has nothing to do" from "this box
    /// cannot get on with anything".
    pub iowait: Option<f32>,
    /// Tasks runnable at the instant of the sample — vmstat's `r`.
    ///
    /// Load average smoothed; this is the unsmoothed truth, and poptop has a
    /// timeline for the smoothing.
    pub running: Option<u32>,
    /// Tasks in uninterruptible sleep — vmstat's `b`.
    ///
    /// The D-state count. Thirty processes blocked on one hung mount give a
    /// load average of thirty on a completely idle box, and this is the only
    /// figure that says so.
    pub blocked: Option<u32>,
    pub mem: MemStat,
    pub load: [f64; 3],
    pub procs: Vec<ProcSample>,
    pub uptime: std::time::Duration,
    /// Tasks the kernel has created since boot, or `None` where the platform
    /// will not say.
    ///
    /// The counter that makes short-lived processes *visible as an absence*.
    /// poptop reads `/proc` at an instant, so a process that lived 200ms never
    /// existed as far as the table is concerned — and a burst of them is one
    /// of the commonest causes of exactly the spike you scrubbed back to find.
    /// The difference between two samples is how many tasks were created in
    /// between, which the table can then be compared against.
    ///
    /// Cumulative rather than a per-interval delta because that is what the
    /// kernel exposes, and because a cumulative counter survives an uneven
    /// interval without needing to know how long it was.
    pub forks: Option<u64>,
    /// Whether extended per-process IO was being collected when this sample was
    /// taken. History predating the column being switched on has this false,
    /// and says so rather than pretending the machine was idle.
    /// Whether this kernel keeps per-process IO accounting at all.
    ///
    /// `CONFIG_TASK_IO_ACCOUNTING` is optional, and some hardened container
    /// runtimes hide the file. Then every read fails with `NotFound` — which is
    /// correctly *not* a permission problem, and so counts towards nothing, and
    /// so the probe that withdraws the columns never fires. The result was
    /// columns that stay on screen permanently empty while the collector keeps
    /// paying for them.
    ///
    /// A different question from `io_denied`, which is about this user rather
    /// than this kernel, and the two want different words on screen.
    pub io_supported: bool,
    pub io_collected: bool,
    /// Processes whose IO file could not be read at all, as opposed to those
    /// merely awaiting a second reading. Only the former is fixed by running as
    /// root, so conflating them produces advice that does not help.
    pub io_denied: usize,
}

impl Sample {
    /// A zeroed sample. Test fixture only — the real path always starts from
    /// a genuine collection.
    #[cfg(test)]
    pub fn empty() -> Self {
        Self {
            at: SystemTime::now(),
            cpu_total: 0.0,
            cpu_per_core: Vec::new(),
            iowait: None,
            running: None,
            blocked: None,
            mem: MemStat::default(),
            load: [0.0; 3],
            procs: Vec::new(),
            uptime: std::time::Duration::ZERO,
            forks: None,
            io_supported: true,
            io_collected: false,
            io_denied: 0,
        }
    }
}
