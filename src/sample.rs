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
    /// Pages modified and not yet written back.
    ///
    /// A box with gigabytes dirty is about to stall on IO, and every other
    /// figure here looks fine until it does.
    pub dirty: Option<u64>,
    /// Kernel memory, and the part of it the kernel can hand back under
    /// pressure.
    ///
    /// A leak here presents as "used" memory belonging to no process, which is
    /// precisely the case where the process table cannot explain the header.
    pub slab: Option<u64>,
    pub slab_reclaimable: Option<u64>,
    /// Shared memory, counted once by the kernel — which is why per-process RSS
    /// sums to more than the machine has.
    pub shmem: Option<u64>,
    /// Page tables. Large and invisible on a database box.
    pub page_tables: Option<u64>,
    /// Huge pages reserved, and how much of that is in use. Also large and
    /// invisible where they are configured at all.
    pub huge_total: Option<u64>,
    pub huge_used: Option<u64>,
}

/// A process this reader knows nothing about, as the base a schema merge fills
/// in. Never a real process: `pid` 0 belongs to no task and `state` is the `?`
/// an unrecognised state already renders as.
impl Default for ProcSample {
    fn default() -> Self {
        Self {
            pid: 0,
            ppid: 0,
            name: Arc::from(""),
            user: Arc::from(""),
            cpu: 0.0,
            rss: 0,
            threads: None,
            state: '?',
            started: None,
            cmd: None,
            io: None,
            container: None,
            minflt: None,
            majflt: None,
            vsize: None,
            nice: None,
            pss: None,
        }
    }
}

/// The blank a schema merge starts from — see [`Sample::unknown`], which is the
/// same thing said for collectors.
impl Default for Sample {
    fn default() -> Self {
        Self::unknown()
    }
}

/// One thread of a process.
///
/// Deliberately not a `ProcSample`. A thread shares its process's memory, user,
/// command line and parent, so repeating them per thread would multiply the
/// retained table by the fields that are identical across it. What differs —
/// and what the reader came to find out — is which thread is running, which is
/// blocked, and how much of the process's CPU each one accounts for.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ThreadSample {
    /// The process this thread belongs to, so a row can be filed under it.
    pub pid: i32,
    /// The kernel's task id. Equal to `pid` for a process's main thread.
    pub tid: i32,
    /// A thread's own name, which is frequently the useful part: a pool of
    /// forty threads called `tokio-runtime-w` with one called `blocking-1` is a
    /// different picture than forty anonymous ones.
    pub name: Arc<str>,
    pub state: char,
    /// Percent of one core over the interval, on the same scale as
    /// [`ProcSample::cpu`], so the threads of a process sum to about its total.
    pub cpu: f32,
}

/// One cgroup, in the unified hierarchy.
///
/// `cpu` and `mem` are **subtree totals**, because that is what cgroup v2
/// publishes — a parent reads higher than any one child rather than equal to
/// the sum of the rows beneath it. The rollup is the kernel's arithmetic, not
/// poptop's, which is the only version of it that can be right about a cgroup
/// holding both processes and children.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CgroupStat {
    /// Path below the hierarchy root; `/` for the root itself.
    pub path: Arc<str>,
    /// Levels below the root, so the table can indent without re-parsing.
    pub depth: u32,
    /// Percent of one core over the interval. `None` on the first sighting:
    /// a rate needs two readings, and the alternative is reporting a cgroup's
    /// whole lifetime of CPU as this second's.
    pub cpu: Option<f32>,
    /// `cpu.max` as a percentage of one core. `None` means unlimited, which is
    /// a different answer from a limit of zero.
    pub cpu_max: Option<f32>,
    pub mem: Option<u64>,
    pub mem_max: Option<u64>,
    pub read: Option<u64>,
    pub write: Option<u64>,
    /// The reason this exists. Machine-wide PSI says something is stalled;
    /// this says which cgroup is stalled, which is the question.
    pub pressure: Option<Pressure>,
    pub procs: Option<u32>,
}

/// One NUMA node.
///
/// On a two-socket machine a single memory figure averages a node that is
/// exhausted with one that is idle, and reads as half full — while the process
/// pinned to the exhausted node stalls on allocation with the header saying
/// there is plenty.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NodeStat {
    pub id: u32,
    pub total: u64,
    pub free: u64,
    /// Page cache on this node.
    pub file: Option<u64>,
    pub dirty: Option<u64>,
    pub shmem: Option<u64>,
    /// The mean of this node's cores, from the per-core figures poptop already
    /// has. Free: no read, just the arithmetic the whole-machine figure was
    /// already hiding.
    ///
    /// `None` where the node's CPU list could not be read, which is the only
    /// thing that maps cores onto nodes.
    pub cpu: Option<f32>,
}

// Every record whose schema the file carries. A record reachable from `Sample`
// but missing here has no schema in the file and cannot be read back, which
// `every_reachable_record_has_a_schema` asserts rather than assumes.
crate::persist::records! {
    MemStat, Stall, Pressure, FsStat, Link, NetStat, DiskStat, IoRates, ThreadSample,
    CgroupStat, NodeStat, ProcSample, Sample
}

// The wire order for each retained struct, listed beside it. The list cannot
// fall behind the struct: both halves are generated from it, so a field added
// above and left out here is a compile error naming the field — the reader
// cannot build the struct, and the writer cannot destructure it. Neither is a
// silent stop-retaining-this. See `crate::persist`.
crate::persist::codec! { MemStat { total: u64, used: u64, available: u64, free: Option<u64>, swap_total: u64, swap_used: u64, dirty: Option<u64>, slab: Option<u64>, slab_reclaimable: Option<u64>, shmem: Option<u64>, page_tables: Option<u64>, huge_total: Option<u64>, huge_used: Option<u64> } }

impl ProcSample {
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
    /// A Linux notion, and stated only there. macOS has no `kthreadd`; pid 2 is
    /// either absent or an ordinary process, and answering `true` for it would
    /// hide a real row from the table and drop a real process from the IO
    /// ratio. The `cfg!` is what keeps the claim on the platform where it is a
    /// fact.
    pub fn is_kernel_thread(&self) -> bool {
        cfg!(target_os = "linux") && (self.pid == KTHREADD || self.ppid == KTHREADD)
    }

    /// What to write in the identity column: the command line if there is one,
    /// and `comm` if there is not.
    ///
    /// A kernel thread has no command line and `[kworker/3:1]` is a real name,
    /// so the fallback is a name rather than a blank.
    pub fn command(&self) -> &str {
        self.cmd.as_deref().unwrap_or(&self.name)
    }
}

/// The longest command line poptop keeps.
///
/// Chrome's renderer runs to 1.4KB of flags — seatbelt handles, shared-memory
/// descriptors, a variations seed. None of it identifies anything to a person,
/// all of it is retained in every sample in the buffer, and the column it goes
/// in is a few dozen characters wide. Cut with an ellipsis so a truncated line
/// says it was truncated.
const CMD_MAX: usize = 200;

/// The command line poptop stores for a process, built from its `argv`.
///
/// The rule, and the reasoning, because a heuristic nobody can state is one
/// nobody can fix:
///
/// 1. **`argv[0]` is reduced to its basename.** `/usr/bin/python3` and
///    `/opt/homebrew/bin/python3` are both `python3`: the directory is the
///    longest part of the string and the part every row shares. htop shows the
///    same by default.
/// 2. **Every argument after it is kept verbatim.** The tempting next step is
///    to shorten path *arguments* the same way, and it is wrong — `node
///    /srv/api/server.js` and `node /srv/web/server.js` both reduce to `node
///    server.js`, destroying exactly the distinction the column exists to draw.
///    The arguments are where the identity lives, so they are not touched.
/// 3. **Reduced here, where `argv` is still a list, rather than at render.**
///    Splitting a joined command line back on its first space finds the wrong
///    boundary the moment a path contains one, and on macOS they nearly all do:
///    `Google Chrome.app/Contents/…/Google Chrome Helper` has four. Doing it
///    here means the boundary is known rather than guessed.
///
/// The cost of that choice is that the full path is not recoverable from a
/// stored sample, so there is no htop-style toggle back to it. That is the
/// trade taken deliberately: the alternative is a second copy of every command
/// line in every retained sample, and the buffer is what poptop spends its
/// memory on.
///
/// `None` for an empty `argv` — see [`ProcSample::cmd`].
pub fn command_from_argv<'a>(argv: impl IntoIterator<Item = &'a str>) -> Option<String> {
    let mut argv = argv.into_iter();
    let argv0 = argv.next()?;
    // `rsplit('/').next()` is never `None`, and on a path with no separator it
    // is the whole string — so this is a no-op rather than a special case.
    let mut out = String::new();
    // Control characters are replaced, not passed through. An argument
    // containing a newline is routine — an `awk` program, a `sed` script, a
    // multi-line `grep -e` pattern — and one of them turns one row of `--once`
    // into several, breaking the line-oriented output that mode exists to give.
    // An ESC sequence in `argv` would otherwise reach the terminal directly.
    // The TUI is safe either way because ratatui drops control characters when
    // it writes a cell, but that is ratatui's guarantee and not this one's.
    let push = |s: &str, out: &mut String| {
        out.extend(s.chars().map(|c| if c.is_control() { ' ' } else { c }))
    };
    push(argv0.rsplit('/').next().unwrap_or(argv0), &mut out);
    for arg in argv {
        out.push(' ');
        push(arg, &mut out);
    }
    if out.is_empty() {
        return None;
    }
    if out.chars().count() > CMD_MAX {
        out = out.chars().take(CMD_MAX - 1).collect::<String>() + "…";
    }
    Some(out)
}

/// `kthreadd`, the parent of every kernel thread, is always pid 2 on Linux.
const KTHREADD: i32 = 2;

/// Time lost waiting for a resource, as a percentage of the last ten seconds.
///
/// The two halves answer different questions and the difference is the whole
/// point. `some` is "at least one task was stalled", which a busy machine does
/// constantly and healthily. `full` is "every runnable task was stalled" —
/// nothing was getting done, by anyone, and there is no benign reading of it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stall {
    pub some: f32,
    pub full: f32,
}

crate::persist::codec! { Stall { some: f32, full: f32 } }

/// Pressure Stall Information, where the kernel publishes it.
///
/// The most direct answer to "why is this slow" that Linux offers, and it says
/// something no other figure here can. `iowait` is a *CPU-side* view — the CPU
/// was idle with IO outstanding — so a box with plenty of other work to do
/// shows a calm `iowait` while every task that matters is stuck behind the
/// disk. `io.full` catches exactly that case.
///
/// Utilisation figures answer "how much is happening". This answers "how much
/// stopped happening", which is the question the tool is opened to settle.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pressure {
    pub cpu: Stall,
    pub io: Stall,
    pub memory: Stall,
}

crate::persist::codec! { Pressure { cpu: Stall, io: Stall, memory: Stall } }

impl Pressure {
    /// The resource that stopped the machine most, and by how much.
    ///
    /// Over IO and memory only. The kernel documents `full` as undefined for
    /// CPU — a task waiting for CPU is by definition not stalled on anything
    /// the CPU could be doing instead — and it reports zero there, which would
    /// win every tie and name the wrong thing.
    ///
    /// IO wins an exact tie because it is the commoner cause by a wide margin,
    /// and because a tie at anything above zero is two resources jammed at once,
    /// where naming either is equally true.
    pub fn worst(&self) -> (&'static str, f32) {
        if self.memory.full > self.io.full {
            ("mem", self.memory.full)
        } else {
            ("io", self.io.full)
        }
    }
}

/// How full one filesystem is.
///
/// The only figure in this tool that describes a hard failure rather than a
/// slowdown: a machine out of disk space does not get slower, it stops. It is
/// also the only one that is a *threshold* rather than a rate — nobody scrubs
/// back forty seconds to see the disk was a fifth of a percent emptier — which
/// is why it earns a header figure and no graph row.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FsStat {
    /// Where it is mounted, which is what a reader recognises it by.
    pub mount: Arc<str>,
    pub total: u64,
    /// Bytes available to an unprivileged writer, which is the number that runs
    /// out. On most filesystems this is below the true free figure, because
    /// some is reserved for root — reporting the larger one would say there is
    /// room when writes have already started failing.
    pub avail: u64,
}

crate::persist::codec! { FsStat { mount: Arc<str>, total: u64, avail: u64 } }

impl FsStat {
    pub fn used_pct(&self) -> f32 {
        if self.total == 0 {
            return 0.0;
        }
        let used = self.total.saturating_sub(self.avail);
        used as f32 / self.total as f32 * 100.0
    }
}

/// One network interface's traffic over the interval.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Link {
    /// Kernel name — `en0`, `eth0`, `wlan0`.
    pub name: Arc<str>,
    /// Bytes per second.
    pub rx: u64,
    pub tx: u64,
    /// Packets per second.
    pub rx_packets: u64,
    pub tx_packets: u64,
}

crate::persist::codec! { Link { name: Arc<str>, rx: u64, tx: u64, rx_packets: u64, tx_packets: u64 } }

impl Link {
    /// Bytes per second in both directions, which is what "busiest" means here.
    pub fn bytes(&self) -> u64 {
        self.rx.saturating_add(self.tx)
    }
}

/// What the network did during the interval, and whether it was healthy.
///
/// Throughput is the figure every monitor draws and the one that least often
/// explains a slow machine: a link at 3% of its capacity dropping 2% of its
/// packets is slow, and one at 90% is usually fine. So the counters that say
/// something is *wrong* are collected alongside, and they are what the header
/// gives its scarce space to.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NetStat {
    /// Interfaces that have ever carried a byte. A laptop publishes twenty-odd
    /// of them, almost all idle tunnels.
    pub links: Vec<Link>,
    /// Packets the interface itself reported as bad, over the interval.
    pub errors: Option<u64>,
    /// Packets the kernel threw away because it could not keep up. `None`
    /// where the platform does not count them separately from errors.
    pub drops: Option<u64>,
    /// TCP segments sent a second time, over the interval.
    ///
    /// The single best "the network path is unhealthy" number on a server, and
    /// the one that most often explains a slow machine whose interfaces look
    /// quiet. `None` where the platform does not publish it.
    pub retrans: Option<u64>,
    /// Connections dropped because the accept queue was full — a service
    /// failing to keep up, which no other figure here would show.
    pub listen_drops: Option<u64>,
}

crate::persist::codec! { NetStat { links: Vec<Link>, errors: Option<u64>, drops: Option<u64>, retrans: Option<u64>, listen_drops: Option<u64> } }

impl NetStat {
    /// The interface carrying the most traffic, if any is known.
    ///
    /// Ties go to the earlier one, as with [`Sample::busiest_disk`]: on an idle
    /// machine every interface is at zero, and naming whichever sorted last
    /// reads as a claim about which one poptop is watching.
    pub fn busiest(&self) -> Option<&Link> {
        let mut it = self.links.iter();
        let mut best = it.next()?;
        for l in it {
            if l.bytes() > best.bytes() {
                best = l;
            }
        }
        Some(best)
    }

    /// The worst thing that happened to the network this interval, if anything
    /// did.
    ///
    /// Ordered by how much each narrows the problem down rather than by size.
    /// Retransmits point at the path between here and elsewhere; listen drops
    /// point at a service on this machine that is not accepting fast enough;
    /// plain drops point at the kernel or the ring buffer; errors point at the
    /// link or the cable. A machine with one retransmit and four hundred
    /// errors is telling you about the cable, but the retransmit is the figure
    /// that changes what you do next.
    ///
    /// `None` when nothing went wrong, so the header spends no space saying so.
    pub fn trouble(&self) -> Option<(&'static str, u64)> {
        [
            ("retrans", self.retrans),
            ("listen drops", self.listen_drops),
            ("drops", self.drops),
            ("errors", self.errors),
        ]
        .into_iter()
        .find_map(|(what, n)| match n {
            Some(n) if n > 0 => Some((what, n)),
            _ => None,
        })
    }
}

/// One block device's activity over the interval that produced this sample.
///
/// Rates, not counters: `/proc/diskstats` publishes cumulative totals and every
/// figure here is a delta, which is why the collector holds the previous read.
///
/// The reason this exists at all is that the header can already say `WAIT 26.7%`
/// and `BLOCKED 30` and then strand you — the next question is always *which
/// device, and how badly*, and the per-process columns answer a different one.
/// A device at 100% utilisation with 40ms service times is slow for everyone on
/// it, including processes issuing almost no IO of their own.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DiskStat {
    /// Kernel name — `nvme0n1`, `vda`, `dm-0`. Whole devices only; partitions
    /// are excluded because their IO is already counted in their disk's.
    pub name: Arc<str>,
    /// Bytes per second. Sectors in `/proc/diskstats` are 512 bytes by
    /// convention, whatever the device's physical block size.
    pub read: u64,
    pub write: u64,
    /// Completed operations per second, which is what "IOPS" means.
    pub reads: u64,
    pub writes: u64,
    /// Percent of the interval the device had at least one request in flight.
    ///
    /// The saturation figure, and the one worth reading first. Throughput says
    /// how much work went through; this says how close the device is to having
    /// no idle time left. A disk can sit at 100% here moving 2 MB/s of random
    /// reads, which is exactly the case throughput alone reports as quiet.
    ///
    /// Not a hard ceiling on modern hardware: an SSD that serves requests in
    /// parallel can be at 100% and still have capacity, which is why `queue`
    /// and `await` sit beside it rather than behind it.
    pub util: f32,
    /// Mean milliseconds a completed operation spent in the device, or `None`
    /// when none completed.
    ///
    /// `None` rather than zero, because a mean of no samples is not zero — and
    /// zero here would read as an infinitely fast disk, the most flattering
    /// possible lie about the figure most worth trusting.
    pub await_ms: Option<f32>,
    /// Mean requests in flight across the interval.
    pub queue: f32,
}

crate::persist::codec! { DiskStat { name: Arc<str>, read: u64, write: u64, reads: u64, writes: u64, util: f32, await_ms: Option<f32>, queue: f32 } }

impl Sample {
    /// The filesystem closest to full, if any is known.
    ///
    /// One figure, because the header has room for one and the question it
    /// answers — "is this machine about to stop" — is settled by the worst.
    /// Ties go to the earlier one, as everywhere else here.
    pub fn fullest(&self) -> Option<&FsStat> {
        let mut it = self.filesystems.as_ref()?.iter();
        let mut worst = it.next()?;
        for f in it {
            if f.used_pct() > worst.used_pct() {
                worst = f;
            }
        }
        Some(worst)
    }
}

impl Sample {
    /// The device closest to having no idle time left, if any is known.
    ///
    /// One device rather than a table, because the header has room for a figure
    /// and not for a panel — and because the question the header answers is
    /// "is storage the problem", which the worst device settles. The others are
    /// a device table's job, if one is ever built.
    /// Ties go to the earlier device, which `max_by` would not do — it returns
    /// the last of equal maxima. On an idle machine every device is at 0.0, and
    /// the header would name whichever one happened to sort last: `loop3 0.0%`
    /// where the collector meant `nvme0n1`. A figure that names a device is
    /// read as "this is the disk poptop is watching", so which one it picks
    /// matters even when the number does not.
    pub fn busiest_disk(&self) -> Option<&DiskStat> {
        let mut it = self.disks.as_ref()?.iter();
        let mut best = it.next()?;
        for d in it {
            if d.util > best.util {
                best = d;
            }
        }
        Some(best)
    }
}

impl Sample {
    /// The most CPU any one process on this machine can have used, in percent
    /// of one core.
    ///
    /// A claim about what the hardware can deliver, so it lives with the model
    /// rather than in a backend: `/proc` has nothing to do with it, and the
    /// next backend should not have to rediscover it. Applied by
    /// [`crate::collect::Collector::sample`] to every sample, whoever produced
    /// it.
    ///
    /// It matters because a per-process figure is a delta between two counters,
    /// and a pid recycled between samples diffs the new process against the old
    /// one's total — which reads as thousands of percent. htop guards the same
    /// way. See [`ProcSample::key`] for the identity that makes the recycle
    /// visible in the first place; this is what keeps the number sane in the
    /// window before it is.
    ///
    /// `None` when the core count is unknown, which is the only honest answer:
    /// clamping to a ceiling of zero would report every process as idle, and a
    /// fabricated zero is the one thing this tool must not produce.
    pub fn cpu_ceiling(&self) -> Option<f32> {
        (!self.cpu_per_core.is_empty()).then_some(self.cpu_per_core.len() as f32 * 100.0)
    }
}

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

crate::persist::codec! { IoRates { read: u64, write: u64 } }

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
    /// Threads in this process, or `None` where the platform will not say.
    ///
    /// Not `1`. A flat `1` is not a missing figure but a wrong one, and it is
    /// visibly wrong next to the column beside it: a single thread cannot use
    /// three cores, yet macOS reported exactly that for every virtual machine
    /// on the box. `None` renders as an em dash and claims nothing.
    ///
    /// Always known on Linux, where `/proc/<pid>/stat` publishes it for every
    /// process. Known on macOS for processes this user owns, which in practice
    /// is every process busy enough for the figure to matter.
    pub threads: Option<u32>,
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
    /// The command line, as the process was invoked, arguments joined by
    /// spaces.
    ///
    /// `comm` — the name one field up — is what `/proc/<pid>/stat` publishes,
    /// and the kernel truncates it to fifteen characters. For anything under an
    /// interpreter or a runtime it is the interpreter's name, so four services
    /// are four rows reading `node` and the table says nothing about any of
    /// them. This is the field that tells them apart.
    ///
    /// `None` where there is no command line to read: a kernel thread has an
    /// empty `cmdline`, and its bracketed `comm` is the only identity it has.
    /// Not an empty string — that would render as a blank row where a name
    /// belongs.
    pub cmd: Option<Arc<str>>,
    /// `None` means no figure is available — either extended collection was off
    /// when this sample was taken, or the process could not be read. The two
    /// cases are told apart by [`Sample::io_collected`], and neither is ever
    /// rendered as a zero: a fabricated zero is indistinguishable from a
    /// genuinely idle process.
    pub io: Option<IoRates>,
    /// The container this process is in, as a twelve-character id.
    ///
    /// `None` means it is in no container — not that poptop could not tell.
    /// The pod *name* is not in the cgroup path at all: atop reads it from the
    /// runtime with superuser, and an id is what poptop can know without asking
    /// anybody's permission.
    pub container: Option<Arc<str>>,
    /// Minor faults in the interval — pages found in memory. Common and cheap.
    pub minflt: Option<u32>,
    /// **Major** faults in the interval: pages fetched from disk.
    ///
    /// The one that answers "why is this slow". A process taking major faults
    /// is being paged in, and nothing else on screen says so — its CPU looks
    /// low and its state looks ordinary.
    ///
    /// A rate over the interval like every other counter here, not the
    /// lifetime total `/proc` publishes.
    pub majflt: Option<u32>,
    /// Virtual size. Against `rss` it is how much of what a process has
    /// reserved it is actually touching.
    pub vsize: Option<u64>,
    /// Scheduling niceness, -20 to 19.
    pub nice: Option<i32>,
    /// Proportional set size: the process's share of the pages it holds, with
    /// shared pages divided among the processes sharing them.
    ///
    /// The honest answer to "how much memory is this actually costing", and the
    /// fix for the caveat grouped RSS carries — summing RSS across six Chrome
    /// renderers counts their shared pages six times, so the total is an upper
    /// bound and says so. PSS sums correctly.
    ///
    /// `None` unless asked for: it needs `smaps_rollup`, a second read per
    /// process, which is why atop gates its own behind a key.
    pub pss: Option<u64>,
}

crate::persist::codec! { ThreadSample { pid: i32, tid: i32, name: Arc<str>, state: char, cpu: f32 } }

crate::persist::codec! { CgroupStat { path: Arc<str>, depth: u32, cpu: Option<f32>, cpu_max: Option<f32>, mem: Option<u64>, mem_max: Option<u64>, read: Option<u64>, write: Option<u64>, pressure: Option<Pressure>, procs: Option<u32> } }

crate::persist::codec! { NodeStat { id: u32, total: u64, free: u64, file: Option<u64>, dirty: Option<u64>, shmem: Option<u64>, cpu: Option<f32> } }

crate::persist::codec! { ProcSample { pid: i32, ppid: i32, name: Arc<str>, user: Arc<str>, cpu: f32, rss: u64, threads: Option<u32>, state: char, started: Option<u64>, cmd: Option<Arc<str>>, io: Option<IoRates>, container: Option<Arc<str>>, minflt: Option<u32>, majflt: Option<u32>, vsize: Option<u64>, nice: Option<i32>, pss: Option<u64> } }

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
    /// Time the hypervisor took for something else, as a share of the interval.
    ///
    /// The figure that separates "the box is busy" from "the box is not being
    /// given a box". On a cloud instance nothing else on screen can say it:
    /// every other number looks healthy while the machine gets less done.
    pub steal: Option<f32>,
    /// Time given to guests. On a hypervisor this is the work, not the
    /// overhead.
    pub guest: Option<f32>,
    /// Hard and soft interrupt time, kept apart. A network-heavy box's softirq
    /// share is the answer to why user time looks low while nothing is idle.
    pub irq: Option<f32>,
    pub softirq: Option<f32>,
    /// Context switches and interrupts a second. A box thrashing between
    /// threads looks identical to a busy one without them.
    pub ctxt: Option<u64>,
    pub intr: Option<u64>,
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
    /// Whether extended per-process IO was being collected when this sample was
    /// taken. History predating the column being switched on has this false,
    /// and says so rather than pretending the machine was idle.
    pub io_collected: bool,
    /// Processes whose IO file could not be read at all, as opposed to those
    /// merely awaiting a second reading. Only the former is fixed by running as
    /// root, so conflating them produces advice that does not help.
    pub io_denied: usize,
    /// Per-device disk activity, or `None` where the platform will not say.
    ///
    /// `None` on macOS: `sysinfo::Disks::refresh` costs 12.5ms steady state,
    /// measured, against a whole sample budget of about 4ms. An em dash is the
    /// honest answer until there is a cheaper route to the same counters.
    ///
    /// An empty list is a different statement from `None` — it means the
    /// platform looked and found no device that has ever done any IO.
    pub disks: Option<Vec<DiskStat>>,
    /// Stall pressure, or `None` where the kernel does not publish it.
    ///
    /// `/proc/pressure` needs `CONFIG_PSI=y`, and some distributions ship it
    /// behind `psi=1` on the kernel command line. Present on every kernel
    /// checked here, absent on plenty that are still in service — so nothing in
    /// the default view is allowed to depend on it, and it renders as an em
    /// dash rather than a zero when it is missing.
    pub pressure: Option<Pressure>,
    /// How much of the processor's nominal clock the kernel is currently
    /// allowing, as a percentage. `None` where the platform will not say.
    ///
    /// Not a temperature. Temperature is a proxy and an inconsistent one: a
    /// figure reading `84°C` makes the reader infer, and on hardware whose
    /// nominal is 85°C it makes them infer wrongly. The machine knows whether
    /// it is allowed to run at full speed and says so directly.
    ///
    /// This is the one cause of "why is this slow" that nothing else here can
    /// show. A capped machine reports 100% busy and gets less work done than it
    /// did an hour ago, while `STALL`, `WAIT` and disk saturation all read
    /// normal — because nothing is *waiting*, the work is simply being done
    /// more slowly.
    ///
    /// The *ceiling*, not the current frequency. Current frequency drops when a
    /// core is idle, which is a healthy machine doing nothing and reads
    /// identically to a throttled one; a ceiling is a statement about what the
    /// machine is permitted to do, so an idle box reads 100%. That catches any
    /// capping the driver reports by lowering its policy maximum — thermal,
    /// power, or a limit somebody set by hand — and does not catch hardware
    /// capping that leaves the policy ceiling alone and reports through
    /// counters instead. Named after what is measured rather than after what is
    /// suspected, which is the same choice `stall` makes.
    pub clock_ceiling: Option<f32>,
    /// Network traffic and health, or `None` where the platform will not say.
    pub net: Option<NetStat>,
    /// Mounted filesystems worth watching, or `None` where the platform will
    /// not say.
    ///
    /// Pseudo-filesystems are excluded by measurement — `proc`, `sysfs`,
    /// `cgroup2` and friends all report zero blocks — and RAM-backed ones by
    /// name, because a full `tmpfs` is a memory problem the header already
    /// reports and counting it here would say the same bytes twice.
    pub filesystems: Option<Vec<FsStat>>,
    /// Every thread of every multi-threaded process, flat, when the thread view
    /// asked for them.
    ///
    /// `None` means nobody asked — not that the box is single-threaded. Flat
    /// rather than nested under each `ProcSample` so a sample carries one
    /// optional list instead of four hundred, and so the task-level figures in
    /// the header can be itemised without walking the process table.
    pub tasks: Option<Vec<ThreadSample>>,
    /// **Bytes a second** read from and written to disk — the file-backed
    /// traffic, not swap.
    ///
    /// Bytes rather than pages, and a rate rather than a total, because that is
    /// what the collector produces and what the panel renders. The kernel
    /// publishes `pgpgin`/`pgpgout` in kilobytes and the swap pair in pages;
    /// both are converted where the page size is known.
    pub pgin: Option<u64>,
    pub pgout: Option<u64>,
    /// **Bytes a second** swapped in and out.
    ///
    /// The figure the swap *level* cannot give: a machine that swapped four
    /// gigabytes in and out during the interval and one sitting on four idle
    /// gigabytes report the same level, and only one of them is in trouble.
    pub swin: Option<u64>,
    pub swout: Option<u64>,
    /// Processes the OOM killer ended during the interval.
    ///
    /// An event, not a level, and the sharpest thing on this list: a process
    /// that was killed is gone from the next sample with nothing anywhere
    /// saying why. Scrubbing back to the moment and seeing both the count and
    /// the process table from the instant before is a thing no live-only
    /// monitor can do.
    pub oom_kills: Option<u64>,
    /// Processes that lived and died inside this interval.
    ///
    /// The gap this closes is the substantive one against atop: poptop reads
    /// `/proc` at an instant, so a process that lived 200ms never existed —
    /// and a burst of them is one of the commonest causes of exactly the spike
    /// somebody opens poptop to explain.
    ///
    /// `None` means nobody could ask: the kernel refuses exit listeners outside
    /// the initial namespace, and macOS has no equivalent. Not an empty list,
    /// which means the interval genuinely had none.
    pub exited: Option<Vec<ProcSample>>,
    /// Per-cgroup utilisation and pressure, when asked for.
    ///
    /// `None` means nobody asked, or this machine has no unified hierarchy —
    /// not that it has no cgroups.
    pub cgroups: Option<Vec<CgroupStat>>,
    /// The machine's NUMA nodes, when it has more than one.
    ///
    /// `None` on a single-node machine as well as on a platform that will not
    /// say: a box with one node spends no space announcing that it has one, and
    /// the figures for it are the whole-machine figures already on screen.
    pub nodes: Option<Vec<NodeStat>>,
}

impl Sample {
    /// A sample that knows nothing, as the base for struct-update syntax.
    ///
    /// A collector states what its platform *can* answer and lets the rest
    /// default:
    ///
    /// ```ignore
    /// Sample { at, cpu_total, mem, procs, ..Sample::unknown() }
    /// ```
    ///
    /// Adding a metric used to mean editing every collector, including the ones
    /// whose only contribution was a `None` meaning "not on this platform".
    /// That edit is now the default, so a new metric costs a line only where
    /// somebody can actually read it.
    ///
    /// **The contract this rests on:** a metric a platform might not have must
    /// be an `Option`. The non-optional fields below are placeholders, not
    /// answers — a required metric is by definition one every backend supplies,
    /// so overwriting them is not optional. Add a non-optional field that some
    /// platform cannot fill and this base will hand it a fabricated zero, which
    /// is the one thing this codebase does not do.
    // Only macOS reaches this today. `linux.rs` answers every field it
    // declares, so on a Linux build nothing constructs a partial `Sample` and
    // the compiler sees an unused function. That is a fact about what Linux can
    // currently answer, not a property of the design: the first metric only
    // macOS can supply puts a `..Sample::unknown()` in `linux.rs` too.
    #[allow(dead_code)]
    pub fn unknown() -> Self {
        Self {
            at: std::time::UNIX_EPOCH,
            cpu_total: 0.0,
            cpu_per_core: Vec::new(),
            iowait: None,
            steal: None,
            guest: None,
            irq: None,
            softirq: None,
            ctxt: None,
            intr: None,
            running: None,
            blocked: None,
            mem: MemStat::default(),
            load: [0.0; 3],
            procs: Vec::new(),
            uptime: std::time::Duration::ZERO,
            forks: None,
            io_supported: false,
            io_collected: false,
            io_denied: 0,
            disks: None,
            pressure: None,
            clock_ceiling: None,
            pgin: None,
            pgout: None,
            swin: None,
            swout: None,
            oom_kills: None,
            net: None,
            filesystems: None,
            tasks: None,
            exited: None,
            cgroups: None,
            nodes: None,
        }
    }
}

crate::persist::codec! { Sample { at: SystemTime, cpu_total: f32, cpu_per_core: Vec<f32>, iowait: Option<f32>, steal: Option<f32>, guest: Option<f32>, irq: Option<f32>, softirq: Option<f32>, ctxt: Option<u64>, intr: Option<u64>, running: Option<u32>, blocked: Option<u32>, mem: MemStat, load: [f64; 3], procs: Vec<ProcSample>, uptime: std::time::Duration, forks: Option<u64>, io_supported: bool, io_collected: bool, io_denied: usize, disks: Option<Vec<DiskStat>>, pressure: Option<Pressure>, clock_ceiling: Option<f32>, pgin: Option<u64>, pgout: Option<u64>, swin: Option<u64>, swout: Option<u64>, oom_kills: Option<u64>, net: Option<NetStat>, filesystems: Option<Vec<FsStat>>, tasks: Option<Vec<ThreadSample>>, exited: Option<Vec<ProcSample>>, cgroups: Option<Vec<CgroupStat>>, nodes: Option<Vec<NodeStat>> } }

impl Sample {
    /// A zeroed sample. Test fixture only — the real path always starts from
    /// a genuine collection.
    #[cfg(test)]
    pub fn empty() -> Self {
        Self {
            at: SystemTime::now(),
            // The fixture's machine keeps per-process IO accounting; tests that
            // want the other answer say so.
            io_supported: true,
            ..Self::unknown()
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn a_sample_that_knows_nothing_claims_nothing() {
        // `unknown()` is the base every collector defaults through, so a value
        // fabricated here is fabricated on every platform that stays quiet
        // about that field — the widest possible blast radius for exactly the
        // mistake this codebase refuses to make.
        let s = Sample::unknown();
        let claims: Vec<&str> = [
            ("iowait", s.iowait.is_some()),
            ("steal", s.steal.is_some()),
            ("guest", s.guest.is_some()),
            ("irq", s.irq.is_some()),
            ("softirq", s.softirq.is_some()),
            ("ctxt", s.ctxt.is_some()),
            ("intr", s.intr.is_some()),
            ("running", s.running.is_some()),
            ("blocked", s.blocked.is_some()),
            ("forks", s.forks.is_some()),
            ("disks", s.disks.is_some()),
            ("pressure", s.pressure.is_some()),
            ("clock_ceiling", s.clock_ceiling.is_some()),
            ("pgin", s.pgin.is_some()),
            ("pgout", s.pgout.is_some()),
            ("swin", s.swin.is_some()),
            ("swout", s.swout.is_some()),
            ("oom_kills", s.oom_kills.is_some()),
            ("net", s.net.is_some()),
            ("filesystems", s.filesystems.is_some()),
            ("mem.free", s.mem.free.is_some()),
            // Not an `Option`, but it is the flag that decides whether the IO
            // columns render at all. Defaulting it to `true` would put a zero
            // where a platform has said nothing.
            ("io_supported", s.io_supported),
            ("io_collected", s.io_collected),
        ]
        .into_iter()
        .filter(|(_, claimed)| *claimed)
        .map(|(name, _)| name)
        .collect();
        assert!(
            claims.is_empty(),
            "an unknown sample claims to know {claims:?}"
        );
    }

    fn label(argv: &[&str]) -> Option<String> {
        command_from_argv(argv.iter().copied())
    }

    #[test]
    fn the_program_is_named_without_its_directory() {
        assert_eq!(label(&["/usr/bin/python3"]).unwrap(), "python3");
        // No separator: the whole string, not a special case.
        assert_eq!(label(&["node"]).unwrap(), "node");
    }

    #[test]
    fn processes_differing_only_in_arguments_are_told_apart() {
        // The whole point of the item. Four rows reading `node` become four
        // services.
        let rows = [
            label(&["/usr/local/bin/node", "/srv/api/server.js"]).unwrap(),
            label(&["/usr/local/bin/node", "/srv/web/bundler.js", "--watch"]).unwrap(),
            label(&["/usr/local/bin/node", "/srv/api/worker.js"]).unwrap(),
        ];
        let distinct: std::collections::HashSet<&String> = rows.iter().collect();
        assert_eq!(distinct.len(), 3, "rows are not distinguishable: {rows:?}");
        assert_eq!(rows[1], "node /srv/web/bundler.js --watch");
    }

    #[test]
    fn a_path_argument_keeps_its_directory() {
        // The rejected heuristic, kept as a test because it is the tempting one.
        // Shortening path *arguments* the way `argv[0]` is shortened collapses
        // these two to `node server.js` and destroys the distinction the column
        // exists to draw.
        let api = label(&["node", "/srv/api/server.js"]).unwrap();
        let web = label(&["node", "/srv/web/server.js"]).unwrap();
        assert_ne!(api, web, "the argument's directory was stripped");
    }

    #[test]
    fn a_program_path_containing_spaces_is_still_reduced() {
        // macOS bundles put spaces in nearly every path, and splitting a joined
        // command line back on its first space finds a boundary inside the
        // directory rather than at the end of it. Reducing here, where argv is
        // still a list, is what makes this work — and it did not, when the
        // split happened at render time.
        let got = label(&[
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome Helper (Renderer)",
            "--type=renderer",
        ])
        .unwrap();
        assert_eq!(got, "Google Chrome Helper (Renderer) --type=renderer");
    }

    #[test]
    fn an_empty_argv_has_no_command_line() {
        // Not an empty string: a kernel thread has no command line, and a blank
        // renders as a row with no name where `[kworker/3:1]` belongs.
        assert_eq!(label(&[]), None);
        assert_eq!(label(&[""]), None);
    }

    #[test]
    fn a_process_without_a_command_line_falls_back_to_its_name() {
        let mut p = ProcSample {
            pid: 2,
            ppid: 0,
            name: Arc::from("[kworker/3:1]"),
            user: Arc::from("root"),
            cpu: 0.0,
            rss: 0,
            threads: Some(1),
            state: 'S',
            started: Some(1),
            cmd: None,
            io: None,

            container: None,
            minflt: None,
            majflt: None,
            vsize: None,
            nice: None,
            pss: None,
        };
        assert_eq!(p.command(), "[kworker/3:1]");
        p.cmd = Some(Arc::from("node server.js"));
        assert_eq!(p.command(), "node server.js");
    }

    #[test]
    fn an_enormous_command_line_is_cut_and_says_so() {
        // A Chrome renderer runs to 1.4KB of seatbelt handles and shared-memory
        // descriptors, retained in every sample in the buffer, for a column a
        // few dozen characters wide.
        let long = "x".repeat(4000);
        let got = label(&["chrome", &long]).unwrap();
        assert_eq!(got.chars().count(), CMD_MAX);
        assert!(got.ends_with('…'), "a cut line does not say it was cut");
        assert!(
            got.starts_with("chrome "),
            "the cut took the identifying end"
        );
    }

    #[test]
    fn a_newline_in_an_argument_stays_on_one_line() {
        // `awk` programs, `sed` scripts and multi-line `grep -e` patterns all
        // carry newlines routinely, and one of them turns one row of `--once`
        // into several — breaking the line-oriented output that mode exists to
        // give.
        let got = label(&["awk", "BEGIN {\n  print 1\n}", "file"]).unwrap();
        assert!(!got.contains('\n'), "the argument broke the row: {got:?}");
        assert!(got.starts_with("awk BEGIN"), "{got:?}");
    }

    #[test]
    fn an_escape_sequence_in_an_argument_does_not_reach_the_terminal() {
        // `argv` is attacker-controlled by anyone who can start a process, and
        // `--once` writes straight to stdout. The TUI is safe either way
        // because ratatui drops control characters as it writes a cell, but
        // that is ratatui's guarantee rather than this one's.
        let got = label(&["sh", "-c", "\x1b[2J\x1b[1;31mred"]).unwrap();
        assert!(!got.contains('\x1b'), "an escape survived: {got:?}");
    }

    #[test]
    fn an_empty_argument_is_kept() {
        // `sh -c ''` really did run with an empty argument, and dropping it
        // would show a different command from the one that is running.
        assert_eq!(label(&["sh", "-c", "", "x"]).unwrap(), "sh -c  x");
    }
}
