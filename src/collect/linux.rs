//! Linux backend: reads `/proc` directly, no dependencies.
//!
//! The kernel exposes cumulative counters, not rates. Every CPU number here is
//! a delta between the previous read and this one, which is why the collector
//! is stateful and why the very first sample reports zero busy time.

use super::{Collector, Needs};
use crate::sample::{DiskStat, IoRates, MemStat, ProcSample, Sample};
use std::collections::HashMap;
use std::fmt::Write as _;
use std::fs;
use std::fs::File;
use std::io;
use std::io::Read as _;
use std::os::unix::fs::MetadataExt;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

/// Raw jiffy counters from one `/proc/stat` CPU line.
#[derive(Clone, Copy, Default)]
struct CpuTimes {
    idle: u64,
    total: u64,
    /// Time the CPU was idle *with at least one I/O request outstanding*.
    ///
    /// Kept rather than folded away. It is already inside `idle` — correctly,
    /// since the CPU genuinely had nothing to run — but that makes poptop right
    /// about the CPU being quiet and silent about the reason, which is the
    /// single most common confusing case there is: high load, idle CPU.
    iowait: u64,
}

impl CpuTimes {
    /// Parse the numbers after the `cpuN` label.
    ///
    /// Fields are: user nice system idle iowait irq softirq steal guest
    /// guest_nice. iowait counts as idle — the CPU genuinely had nothing to run.
    fn parse(fields: &str) -> Option<Self> {
        let v: Vec<u64> = fields
            .split_whitespace()
            .filter_map(|f| f.parse().ok())
            .collect();
        if v.len() < 4 {
            return None;
        }
        // guest and guest_nice are already counted inside user and nice, so
        // summing every field as-is would double-count them.
        let total: u64 = v.iter().take(8).sum();
        let iowait = v.get(4).copied().unwrap_or(0);
        Some(Self {
            idle: v[3] + iowait,
            total,
            iowait,
        })
    }

    /// Share of the interval the CPU spent waiting on I/O.
    ///
    /// A percentage of wall clock across all cores, the same denominator
    /// `busy_pct_since` uses, so the two can be read against each other: 12%
    /// busy and 61% waiting is a machine doing nothing while being unable to
    /// get on with anything.
    fn iowait_pct_since(&self, prev: &Self) -> f32 {
        let dt = self.total.saturating_sub(prev.total);
        if dt == 0 {
            return 0.0;
        }
        let dw = self.iowait.saturating_sub(prev.iowait);
        ((dw as f64 / dt as f64) * 100.0) as f32
    }

    /// Busy percentage between two reads.
    fn busy_pct_since(&self, prev: &Self) -> f32 {
        let dt = self.total.saturating_sub(prev.total);
        if dt == 0 {
            return 0.0;
        }
        let di = self.idle.saturating_sub(prev.idle);
        let busy = dt.saturating_sub(di);
        ((busy as f64 / dt as f64) * 100.0) as f32
    }
}

/// The fastest `/proc` can be walked without the monitor becoming the load.
///
/// A cost argument, and the number is the argument: a pass is about 1ms at 400
/// processes, so 50ms spends 2% of a core and 10ms would spend 10%. A monitor
/// that is itself the load is not measuring the machine, it is measuring
/// itself.
pub const MIN_INTERVAL: std::time::Duration = std::time::Duration::from_millis(50);
pub const MIN_INTERVAL_WHY: &str = "a /proc pass costs about 1ms at 400 processes, so anything faster spends \
     more of the machine on watching it than is worth knowing";

pub struct ProcFs {
    prev_total: Option<CpuTimes>,
    prev_cores: Vec<CpuTimes>,
    /// Raw `/proc/diskstats` counters from the previous read, per device.
    prev_disks: HashMap<Arc<str>, DiskTimes>,
    /// Which names in `/proc/diskstats` are whole devices rather than
    /// partitions, from `/sys/block`.
    ///
    /// Cached because it is a directory listing and the answer changes only
    /// when hardware is plugged in — see [`ProcFs::is_whole_device`], which
    /// refreshes it exactly when a name it has never seen shows up.
    block_devices: std::collections::HashSet<Arc<str>>,
    /// Names already established not to be whole devices. See
    /// [`ProcFs::is_whole_device`]: without this, every mounted partition costs
    /// a directory enumeration on every sample.
    partitions: std::collections::HashSet<Arc<str>>,
    /// pid -> cumulative (utime + stime) jiffies at the previous sample.
    prev_proc_jiffies: HashMap<i32, u64>,
    /// pid -> cumulative (read_bytes, write_bytes) at the previous sample.
    prev_proc_io: HashMap<i32, (u64, u64)>,
    prev_at: Option<SystemTime>,
    /// uid -> username, parsed once from /etc/passwd.
    users: HashMap<u32, Arc<str>>,
    /// pid -> (start time, name). The start time is what makes this safe: a
    /// recycled pid would otherwise inherit the dead process's name, which is
    /// a correctness bug wearing an optimisation's clothes.
    names: HashMap<i32, (u64, Arc<str>)>,
    /// Reused across every file read, so a sample allocates no buffers.
    buf: Vec<u8>,
    /// Reused path, so `/proc/<pid>/stat` costs no allocation either.
    path: String,
    ticks_per_sec: f64,
    page_size: u64,
    /// What this backend had to assume rather than read.
    notes: Vec<String>,
    /// Whether this kernel has `/proc/<pid>/io` at all. Asked once, of our own
    /// process, which is always readable if the file exists — so a `NotFound`
    /// here is the kernel saying it does not keep the accounting, not a
    /// permission problem and not a process that exited.
    io_supported: bool,
}

/// Everything one `/proc/stat` read yields.
///
/// A struct rather than a widening tuple: the file carries six unrelated facts
/// and a six-tuple at the call site says nothing about which is which.
struct StatRead {
    busy: f32,
    per_core: Vec<f32>,
    /// Share of the interval spent idle with I/O outstanding.
    iowait: f32,
    /// Tasks created since boot.
    forks: Option<u64>,
    /// Tasks runnable right now — vmstat's `r`.
    running: Option<u32>,
    /// Tasks in uninterruptible sleep — vmstat's `b`, and the D-state count
    /// that answers "why is load high when nothing is running".
    blocked: Option<u32>,
}

impl ProcFs {
    pub fn new() -> io::Result<Self> {
        let (page_size, notes) = read_page_size();
        let io_supported = std::fs::File::open("/proc/self/io").is_ok();
        Ok(Self {
            io_supported,
            prev_total: None,
            prev_cores: Vec::new(),
            prev_disks: HashMap::new(),
            block_devices: read_block_devices(),
            partitions: std::collections::HashSet::new(),
            prev_proc_jiffies: HashMap::new(),
            prev_proc_io: HashMap::new(),
            prev_at: None,
            users: parse_passwd(),
            names: HashMap::new(),
            buf: vec![0; READ_BUF],
            path: String::with_capacity(32),
            // USER_HZ is fixed at 100 on effectively every Linux build. The
            // honest way is sysconf(_SC_CLK_TCK), but that needs libc, and the
            // point of this backend is to need nothing.
            ticks_per_sec: 100.0,
            page_size,
            notes,
        })
    }

    fn read_stat_file(&mut self) -> io::Result<StatRead> {
        let Self {
            buf,
            prev_total,
            prev_cores,
            ..
        } = self;
        let stat = read_into("/proc/stat", buf)?;
        let mut total_now = CpuTimes::default();
        let mut cores_now = Vec::new();

        // `processes` is the count of tasks the kernel has created since boot.
        // It comes after the cpu lines, so the loop can no longer stop at the
        // first non-cpu line — but it is one integer parse on a file already
        // being read, so the cost is a rounding error against the per-process
        // work that dominates a sample.
        let mut forks = None;
        let mut running = None;
        let mut blocked = None;
        for line in stat.lines() {
            if let Some(n) = line.strip_prefix("processes ") {
                forks = n.trim().parse().ok();
                continue;
            }
            if let Some(n) = line.strip_prefix("procs_running ") {
                running = n.trim().parse().ok();
                continue;
            }
            if let Some(n) = line.strip_prefix("procs_blocked ") {
                blocked = n.trim().parse().ok();
                continue;
            }
            let Some(rest) = line.strip_prefix("cpu") else {
                continue;
            };
            match rest.split_once(char::is_whitespace) {
                // "cpu  ..." — the aggregate line has no digit after "cpu"
                Some(("", fields)) => {
                    if let Some(t) = CpuTimes::parse(fields) {
                        total_now = t;
                    }
                }
                // "cpu0 ...", "cpu1 ..." — per-core lines
                Some((_n, fields)) => {
                    if let Some(t) = CpuTimes::parse(fields) {
                        cores_now.push(t);
                    }
                }
                None => {}
            }
        }

        let (total_pct, iowait_pct) = match *prev_total {
            Some(prev) => (
                total_now.busy_pct_since(&prev),
                total_now.iowait_pct_since(&prev),
            ),
            None => (0.0, 0.0),
        };
        let core_pcts = cores_now
            .iter()
            .enumerate()
            .map(|(i, now)| match prev_cores.get(i) {
                Some(prev) => now.busy_pct_since(prev),
                None => 0.0,
            })
            .collect();

        *prev_total = Some(total_now);
        *prev_cores = cores_now;
        Ok(StatRead {
            busy: total_pct,
            per_core: core_pcts,
            iowait: iowait_pct,
            forks,
            running,
            blocked,
        })
    }

    /// Whether `name` is a whole device rather than a partition.
    ///
    /// Partitions are excluded because their IO is already inside their disk's
    /// counters — showing `vda` and `vda1` beside each other double-counts every
    /// byte and invites the reader to add them up.
    ///
    /// Both answers are cached, and the negative one especially. A mounted
    /// partition has nonzero reads from its superblock alone, so it reaches
    /// here on every sample — and without remembering the miss, each one costs
    /// a full `/sys/block` enumeration. Three mounted partitions at the 50ms
    /// floor is sixty directory reads a second, against a per-sample budget of
    /// about a millisecond.
    ///
    /// So the directory is re-read only for a name neither set has seen, which
    /// is what picks up a disk plugged in mid-run. A machine with no `sysfs`
    /// gets an empty set and keeps everything: showing a partition is a smaller
    /// error than showing nothing.
    fn is_whole_device(&mut self, name: &str) -> bool {
        if self.block_devices.is_empty() || self.block_devices.contains(name) {
            return true;
        }
        if self.partitions.contains(name) {
            return false;
        }
        // Genuinely new. Either a partition seen for the first time, or
        // hardware that arrived after startup — one directory read tells us
        // which, and either answer is remembered.
        let refreshed = read_block_devices();
        if refreshed.contains(name) {
            self.block_devices = refreshed;
            return true;
        }
        self.partitions.insert(Arc::from(name));
        false
    }

    /// Per-device rates over `elapsed`, from `/proc/diskstats`.
    ///
    /// `None` when the file cannot be read at all, which is the same "this
    /// platform will not say" that macOS reports — the panel then draws no disk
    /// row rather than a flat one.
    ///
    /// Empty on the first sample, because every figure here is a delta and
    /// there is nothing to subtract from yet. That one is drawn as zero, the
    /// same way the first sample's CPU is: a rate needs two reads, and the
    /// alternative is a graph that starts one sample later than every other.
    fn read_diskstats(&mut self, elapsed: Duration) -> Option<Vec<DiskStat>> {
        let text = fs::read_to_string("/proc/diskstats").ok()?;
        Some(self.diskstats_from(&text, elapsed))
    }

    /// Split from the read so the filters and the arithmetic can be tested
    /// against a fixture rather than against whatever devices this machine
    /// happens to have — the same split `parse_meminfo` has, for the same
    /// reason.
    fn diskstats_from(&mut self, text: &str, elapsed: Duration) -> Vec<DiskStat> {
        let secs = elapsed.as_secs_f64();

        let mut out = Vec::new();
        let mut seen = HashMap::new();
        for line in text.lines() {
            let mut fields = line.split_whitespace();
            // major, minor, name, then the counters.
            let (Some(_), Some(_), Some(name)) = (fields.next(), fields.next(), fields.next())
            else {
                continue;
            };
            let rest = &line[line.find(name).map_or(line.len(), |i| i + name.len())..];
            let now = DiskTimes::parse(rest);
            if !now.ever_used() || !self.is_whole_device(name) {
                continue;
            }
            let name: Arc<str> = self
                .prev_disks
                .keys()
                .find(|k| ***k == *name)
                .cloned()
                .unwrap_or_else(|| Arc::from(name));
            if let Some(prev) = self.prev_disks.get(&name)
                && secs > 0.0
            {
                out.push(rates(&name, prev, &now, secs));
            }
            seen.insert(name, now);
        }
        self.prev_disks = seen;
        // Busiest first, so a table that can only show two rows shows the two
        // that matter. Utilisation rather than throughput, for the reason in
        // `DiskStat::util`.
        out.sort_by(|a, b| b.util.total_cmp(&a.util));
        out
    }

    fn read_mem(&mut self) -> io::Result<MemStat> {
        Ok(parse_meminfo(read_into("/proc/meminfo", &mut self.buf)?))
    }

    fn read_load(&mut self) -> io::Result<[f64; 3]> {
        let text = read_into("/proc/loadavg", &mut self.buf)?;
        let mut it = text.split_whitespace();
        let mut out = [0.0; 3];
        for slot in out.iter_mut() {
            *slot = it.next().and_then(|v| v.parse().ok()).unwrap_or(0.0);
        }
        Ok(out)
    }

    fn read_uptime(&mut self) -> io::Result<Duration> {
        let text = read_into("/proc/uptime", &mut self.buf)?;
        let secs: f64 = text
            .split_whitespace()
            .next()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.0);
        Ok(Duration::from_secs_f64(secs))
    }

    fn read_procs(
        &mut self,
        elapsed: Duration,
        needs: Needs,
        denied: &mut usize,
    ) -> io::Result<Vec<ProcSample>> {
        // Destructured so the shared read buffer, the path buffer and the
        // previous-sample maps are all borrowed disjointly. Going through
        // `&mut self` would hold the whole collector for as long as the text
        // read out of the buffer lives.
        let Self {
            buf,
            path,
            prev_proc_jiffies,
            prev_proc_io,
            users,
            names,
            ticks_per_sec,
            page_size,
            io_supported,
            ..
        } = self;
        let ctx = StatCtx {
            prev_jiffies: prev_proc_jiffies,
            ticks_per_sec: *ticks_per_sec,
            page_size: *page_size,
        };

        let mut out = Vec::new();
        let mut seen = HashMap::new();
        let mut seen_io = HashMap::new();
        let elapsed_secs = elapsed.as_secs_f64();

        for entry in fs::read_dir("/proc")? {
            let Ok(entry) = entry else { continue };
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            let Ok(pid) = name.parse::<i32>() else {
                continue; // non-numeric entries are not processes
            };

            // Processes exit while we walk the directory; a vanished pid is
            // normal, not an error worth surfacing. Read first, so an exited
            // process costs one failed open and nothing else — putting the uid
            // stat above this made every dead pid pay for a statx too.
            path.clear();
            let _ = write!(path, "/proc/{pid}/stat");
            let Ok(stat) = read_into(path, buf) else {
                continue;
            };

            // The /proc/<pid> directory is owned by the process's uid, so one
            // stat answers what parsing /proc/<pid>/status also would.
            let uid = entry.metadata().map(|m| m.uid()).unwrap_or(0);
            let user = users
                .entry(uid)
                .or_insert_with(|| Arc::from(uid.to_string().as_str()))
                .clone();

            let Some(mut p) =
                parse_proc_stat(pid, stat, elapsed_secs, user, &mut seen, names, &ctx)
            else {
                continue;
            };
            // Kernel threads are skipped rather than attempted and counted as
            // denied. They are root-owned and unreadable to an ordinary user,
            // and on a many-core box they outnumber the real processes — so
            // counting them would fire the IO probe on exactly the laptop it
            // exists to protect. Skipping also saves an open and a read each.
            if needs.io && *io_supported && !p.is_kernel_thread() {
                match read_proc_io(pid, elapsed_secs, &mut seen_io, prev_proc_io, path, buf) {
                    Ok(rates) => p.io = rates,
                    // Either way the row shows an em dash. Only one of them is
                    // something root would fix, and only that one is counted.
                    Err(why) => *denied += usize::from(why.counts()),
                }
            }
            out.push(p);
        }

        // Drop counters for processes that have exited, or the maps grow
        // without bound on a busy box.
        // Drop cached names for processes that have exited, or the map grows
        // without bound exactly like the counters would.
        names.retain(|pid, _| seen.contains_key(pid));
        *prev_proc_jiffies = seen;
        // Cleared rather than kept while collection is off. Rates are a delta
        // against the previous read divided by one interval, so counters left
        // over from five minutes ago would render every long-lived process at
        // three hundred times its real rate on the frame collection resumes —
        // and a pid reused in the meantime would diff against a stranger.
        //
        // Collection could not resume before the probe existed, which is why
        // this held: the ratchet only ever went off to on.
        *prev_proc_io = if needs.io { seen_io } else { HashMap::new() };
        Ok(out)
    }
}

/// Why a process's disk IO could not be read.
///
/// The two mean opposite things and the kernel already distinguishes them:
/// `/proc/999999/io` is `NotFound`, `/proc/1/io` as a normal user is
/// `PermissionDenied`. Collapsing them made a process that exited between the
/// directory listing and the read into one more process that "needs root" — in
/// a figure poptop prints, and which item 0022 turned into a decision about
/// whether to show the columns at all.
enum Unreadable {
    /// Needs `CAP_SYS_PTRACE`. Root would fix it, and the count says so.
    Denied,
    /// The process exited, or its file was malformed. Nothing would fix it and
    /// nothing is wrong: it is normal on any box with process churn, which is
    /// exactly the kind poptop gets pointed at.
    Moot,
}

impl Unreadable {
    /// Classify a read failure. Only a permission problem is something root
    /// would fix, and only that is worth counting.
    fn from_kind(kind: io::ErrorKind) -> Self {
        match kind {
            io::ErrorKind::PermissionDenied => Self::Denied,
            _ => Self::Moot,
        }
    }

    fn counts(&self) -> bool {
        matches!(self, Self::Denied)
    }
}

/// Per-process disk throughput from `/proc/<pid>/io`.
///
/// That file is mode 0400 and owned by the process owner, so reading another
/// user's process needs CAP_SYS_PTRACE. Unreadable means no figure, never zero
/// — showing every process you do not own as idle would be a confident lie,
/// where a blank is merely an absence.
///
/// `Err` means the file could not be read, and [`Unreadable`] says whether
/// anyone could do anything about that. `Ok(None)` means the process is too new
/// to have a previous counter to diff against, which fixes itself on the next
/// sample. All of them render as a dash; only one is worth advising about.
fn read_proc_io(
    pid: i32,
    elapsed_secs: f64,
    seen: &mut HashMap<i32, (u64, u64)>,
    prev: &HashMap<i32, (u64, u64)>,
    path: &mut String,
    buf: &mut Vec<u8>,
) -> Result<Option<IoRates>, Unreadable> {
    path.clear();
    let _ = write!(path, "/proc/{pid}/io");
    let text = read_into(path, buf).map_err(|e| Unreadable::from_kind(e.kind()))?;
    let (read, write) = parse_proc_io(text).ok_or(Unreadable::Moot)?;
    seen.insert(pid, (read, write));

    let Some((prev_r, prev_w)) = prev.get(&pid).copied() else {
        return Ok(None);
    };
    if elapsed_secs <= 0.0 {
        return Ok(None);
    }
    Ok(Some(IoRates {
        read: ((read.saturating_sub(prev_r)) as f64 / elapsed_secs) as u64,
        write: ((write.saturating_sub(prev_w)) as f64 / elapsed_secs) as u64,
    }))
}

/// Turn one `/proc/<pid>/stat` line into a sample.
#[allow(clippy::too_many_arguments)]
fn parse_proc_stat(
    pid: i32,
    stat: &str,
    elapsed_secs: f64,
    user: Arc<str>,
    seen: &mut HashMap<i32, u64>,
    names: &mut HashMap<i32, (u64, Arc<str>)>,
    ctx: &StatCtx,
) -> Option<ProcSample> {
    // Field 2 is the executable name in parentheses, and it may contain spaces
    // *and* parentheses, so splitting on whitespace corrupts every field after
    // it. Split on the last ')' instead.
    let close = stat.rfind(')')?;
    let open = stat.find('(')?;
    let comm = stat.get(open + 1..close)?;
    let rest: Vec<&str> = stat.get(close + 1..)?.split_whitespace().collect();

    // rest[0] is field 3 (state), so field N lives at rest[N - 3].
    let state = rest.first()?.chars().next().unwrap_or('?');
    let ppid: i32 = rest.get(1)?.parse().ok()?;
    let utime: u64 = rest.get(11)?.parse().ok()?;
    let stime: u64 = rest.get(12)?.parse().ok()?;
    let threads: u32 = rest.get(17)?.parse().unwrap_or(1);
    // Field 22, the start time, in clock ticks since boot.
    let starttime: u64 = rest.get(19)?.parse().unwrap_or(0);
    let rss_pages: u64 = rest.get(21)?.parse().unwrap_or(0);

    // Reuse the name unless this is a different process wearing the same pid.
    // Comparing the string as well would defeat the point — the comparison is
    // the work being avoided — so the start time is the whole guard.
    let name = match names.get(&pid) {
        Some((t, n)) if *t == starttime => n.clone(),
        _ => {
            let n: Arc<str> = Arc::from(comm);
            names.insert(pid, (starttime, n.clone()));
            n
        }
    };

    let jiffies = utime + stime;
    seen.insert(pid, jiffies);

    // Same story as the aggregate CPU: only the delta means anything.
    let cpu = match ctx.prev_jiffies.get(&pid) {
        Some(&prev) if elapsed_secs > 0.0 => {
            let dj = jiffies.saturating_sub(prev) as f64;
            // Unclamped on purpose. A pid reused between samples diffs the
            // new process against the old one's counter and lands in the
            // thousands of percent, but the ceiling that catches it is a fact
            // about the machine rather than about `/proc` — see
            // [`Sample::cpu_ceiling`], applied to every backend's output.
            ((dj / ctx.ticks_per_sec / elapsed_secs) * 100.0) as f32
        }
        _ => 0.0,
    };

    Some(ProcSample {
        pid,
        ppid,
        name,
        user,
        cpu,
        rss: rss_pages * ctx.page_size,
        threads: Some(threads),
        state,
        started: Some(starttime),
        io: None,
    })
}

/// The collector state a stat line needs to become a `ProcSample`.
///
/// Passed explicitly rather than through `&self` so the shared read buffer can
/// be borrowed mutably at the same time.
struct StatCtx<'a> {
    prev_jiffies: &'a HashMap<i32, u64>,
    ticks_per_sec: f64,
    page_size: u64,
}

/// Starting size of the shared read buffer.
///
/// A floor, not a cap: the buffer ratchets up to the largest `/proc` file seen
/// and never shrinks, so on a many-core machine `/proc/stat` alone will push it
/// past this on the first sample and every later read carries the larger
/// allocation. That is bounded by the largest file poptop reads — a few
/// kilobytes — and is the price of never reallocating during a sample.
const READ_BUF: usize = 8192;

/// `/proc/meminfo` into a `MemStat`.
///
/// Split from the read so a test can assert on a fixed input. Comparing a
/// collector's figures against a second read of the same file does not work:
/// memory moves between them, and the test fails a couple of runs in six. That
/// is the same trap that ruled out deriving the page size from `statm` against
/// `status`, met again in a test.
fn parse_meminfo(text: &str) -> MemStat {
    let get = |key: &str| -> u64 {
        text.lines()
            .find_map(|l| {
                l.strip_prefix(key)?
                    .split_whitespace()
                    .next()?
                    .parse::<u64>()
                    .ok()
            })
            .unwrap_or(0)
            * 1024 // meminfo is in kB
    };
    let total = get("MemTotal:");
    let available = get("MemAvailable:");
    let free = get("MemFree:");
    let swap_total = get("SwapTotal:");
    let swap_free = get("SwapFree:");
    MemStat {
        total,
        // MemAvailable already accounts for reclaimable cache, so this is the
        // "really in use" figure rather than the alarming one.
        used: total.saturating_sub(available),
        available,
        // Clamped: `MemFree` and `MemAvailable` are read from the same snapshot
        // but computed differently, and on a box with almost no cache the
        // estimate can land just under free — which would make the cache
        // segment of the memory bar negative and wrap.
        free: Some(free.min(available)),
        swap_total,
        swap_used: swap_total.saturating_sub(swap_free),
    }
}

/// Raw cumulative counters for one device, straight out of `/proc/diskstats`.
#[derive(Clone, Copy, Default)]
struct DiskTimes {
    reads: u64,
    writes: u64,
    sectors_read: u64,
    sectors_written: u64,
    /// Milliseconds spent servicing reads, and writes, summed across requests —
    /// so this can exceed wall clock on a device with parallelism.
    read_ms: u64,
    write_ms: u64,
    /// Milliseconds the device had at least one request in flight. Bounded by
    /// wall clock, which is what makes it a utilisation figure.
    io_ms: u64,
    /// Request-milliseconds: time in flight weighted by queue depth.
    weighted_ms: u64,
}

impl DiskTimes {
    /// Field 4 onwards of a `/proc/diskstats` line, after major, minor, name.
    ///
    /// Older kernels publish 14 fields and newer ones 20, the extra six being
    /// discard and flush accounting that nothing here wants. Read by index with
    /// a default so both shapes parse — a kernel with fewer fields loses only
    /// the figures it never had.
    fn parse(rest: &str) -> Self {
        let v: Vec<u64> = rest
            .split_whitespace()
            .map(|f| f.parse().unwrap_or(0))
            .collect();
        let at = |i: usize| v.get(i).copied().unwrap_or(0);
        Self {
            reads: at(0),
            sectors_read: at(2),
            read_ms: at(3),
            writes: at(4),
            sectors_written: at(6),
            write_ms: at(7),
            io_ms: at(9),
            weighted_ms: at(10),
        }
    }

    /// Whether this device has ever done anything.
    ///
    /// The filter that keeps the table readable, and a measurement rather than
    /// a naming rule. This container publishes 42 whole devices — `ram0..15`,
    /// `loop0..7`, `nbd0..15` — of which two have ever completed an operation.
    /// Excluding them by name would also exclude a loop device that is actually
    /// backing something, which on a machine running containers is a device
    /// worth watching.
    fn ever_used(&self) -> bool {
        self.reads > 0 || self.writes > 0
    }
}

/// Turn two cumulative reads into the rates a reader wants.
///
/// The arithmetic `iostat` does, and worth naming because two of the four are
/// not obvious:
///
/// - `util` is time-the-device-was-busy over wall clock, so it saturates at
///   100% however deep the queue goes.
/// - `await` divides service time by *completed* operations, so it is a mean
///   per operation rather than a share of the interval — and is `None` when
///   nothing completed, since a mean of no samples is not zero.
fn rates(name: &Arc<str>, prev: &DiskTimes, now: &DiskTimes, secs: f64) -> DiskStat {
    let d = |a: u64, b: u64| a.saturating_sub(b) as f64;
    let reads = d(now.reads, prev.reads);
    let writes = d(now.writes, prev.writes);
    let ops = reads + writes;
    let service = d(now.read_ms, prev.read_ms) + d(now.write_ms, prev.write_ms);
    DiskStat {
        name: name.clone(),
        read: (d(now.sectors_read, prev.sectors_read) * SECTOR as f64 / secs) as u64,
        write: (d(now.sectors_written, prev.sectors_written) * SECTOR as f64 / secs) as u64,
        reads: (reads / secs) as u64,
        writes: (writes / secs) as u64,
        // Clamped: the counter is in whole milliseconds and `secs` is measured,
        // so rounding can put a fully busy device a hair over 100.
        util: ((d(now.io_ms, prev.io_ms) / 10.0 / secs) as f32).min(100.0),
        await_ms: (ops > 0.0).then(|| (service / ops) as f32),
        queue: (d(now.weighted_ms, prev.weighted_ms) / 1000.0 / secs) as f32,
    }
}

/// Sectors are 512 bytes in `/proc/diskstats` whatever the device's physical
/// block size. The kernel converts; this is not an assumption about hardware.
const SECTOR: u64 = 512;

/// Whole block devices, from `/sys/block`.
///
/// Empty if the directory is missing — some minimal containers do not mount
/// `sysfs` — and [`ProcFs::is_whole_device`] treats that as "cannot tell", which
/// keeps every device rather than silently dropping them all.
fn read_block_devices() -> std::collections::HashSet<Arc<str>> {
    fs::read_dir("/sys/block")
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| Arc::from(e.file_name().to_string_lossy().as_ref()))
        .collect()
}

/// The size of a page on this kernel, and what to say if we had to guess.
///
/// Read rather than assumed, because `/proc/<pid>/stat` reports RSS as a count
/// of pages and the multiplication is the whole figure. A 16 KiB-page kernel —
/// Asahi, several ARM distributions — would otherwise report every process's
/// memory at a quarter of its real size, and a 64 KiB one, which has been the
/// RHEL aarch64 default, at a sixteenth. Nothing about the output would look
/// wrong.
///
/// From `/proc/self/auxv` rather than `/proc/self/smaps`, which was the first
/// attempt. auxv is what the kernel handed this process at exec, so it is there
/// whatever the kernel was built with; it is one 336-byte read rather than a
/// walk over every mapping; and it cannot be misled by a process whose first
/// mapping is a huge page, where smaps says `2048 kB` and the only thing
/// between that and a sixteen-fold error is a sanity clamp.
///
/// Deriving it from `statm` pages against `status` `VmRSS` does not work at
/// all: the two files are read microseconds apart and RSS moves in between,
/// which measured 4084 against a real 4096.
fn read_page_size() -> (u64, Vec<String>) {
    page_size_from(std::fs::read("/proc/self/auxv").ok().as_deref())
}

/// The file split out, so the fallback can be tested on a machine that can read
/// it. The same shape as `Tier::detect_from` and `config::path_from`.
fn page_size_from(auxv: Option<&[u8]>) -> (u64, Vec<String>) {
    const ASSUMED: u64 = 4096;
    match auxv.and_then(parse_page_size) {
        Some(n) => (n, Vec::new()),
        None => (
            ASSUMED,
            vec![format!(
                "could not read the page size from /proc/self/auxv; assuming \
                 {ASSUMED} bytes. Every process memory figure is a page count \
                 times this, so they are all wrong by a whole factor if this \
                 kernel disagrees."
            )],
        ),
    }
}

/// `AT_PAGESZ` out of an auxiliary vector.
///
/// A flat list of `usize` key/value pairs in native order, terminated by a zero
/// key. Sized from `usize` rather than assuming eight bytes: the layout follows
/// the pointer width, and this is the one place that matters.
fn parse_page_size(auxv: &[u8]) -> Option<u64> {
    const AT_PAGESZ: usize = 6;
    let w = size_of::<usize>();
    for pair in auxv.chunks_exact(w * 2) {
        let key = usize::from_ne_bytes(pair[..w].try_into().ok()?);
        if key == 0 {
            break;
        }
        if key == AT_PAGESZ {
            let v = usize::from_ne_bytes(pair[w..].try_into().ok()?) as u64;
            // Sanity rather than trust: a page is a power of two between 4 KiB
            // and 64 KiB on every architecture Linux runs on, and a figure
            // outside that is a read that went wrong rather than an exotic
            // machine — which would otherwise be indistinguishable.
            return (v.is_power_of_two() && (4096..=65536).contains(&v)).then_some(v);
        }
    }
    None
}

/// Read a file into `buf` and return it as text.
///
/// `File::open` + `read` rather than `fs::read_to_string`, which calls
/// `metadata()` to size its buffer. `/proc` reports every file as zero bytes,
/// so that `statx` is pure waste — and because the size comes back as zero the
/// buffer then grows a page at a time, which is where 8.5 reads per process
/// came from. htop does the same thing with `openat` on a cached directory fd
/// and one `read` into a fixed buffer.
fn read_into<'b>(path: &str, buf: &'b mut Vec<u8>) -> io::Result<&'b str> {
    let mut f = File::open(path)?;
    if buf.len() < READ_BUF {
        buf.resize(READ_BUF, 0);
    }
    let mut n = 0;
    loop {
        if n == buf.len() {
            // Only for a file larger than anything /proc is expected to serve.
            buf.resize(buf.len() * 2, 0);
        }
        match f.read(&mut buf[n..]) {
            Ok(0) => break,
            Ok(k) => n += k,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
    std::str::from_utf8(&buf[..n])
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "not utf-8"))
}

/// Cumulative block-layer bytes from `/proc/<pid>/io`.
///
/// `read_bytes`/`write_bytes` are actual device traffic, which is what iotop
/// reports; `rchar`/`wchar` count bytes passed to syscalls and include reads
/// served from page cache, which would overstate disk load considerably.
fn parse_proc_io(text: &str) -> Option<(u64, u64)> {
    let mut read = None;
    let mut write = None;
    for line in text.lines() {
        if let Some(v) = line.strip_prefix("read_bytes:") {
            read = v.trim().parse().ok();
        } else if let Some(v) = line.strip_prefix("write_bytes:") {
            write = v.trim().parse().ok();
        }
    }
    Some((read?, write?))
}

/// uid -> name, from /etc/passwd. Good enough without NSS; unknown uids fall
/// back to their numeric form at lookup time.
fn parse_passwd() -> HashMap<u32, Arc<str>> {
    let mut map = HashMap::new();
    if let Ok(text) = fs::read_to_string("/etc/passwd") {
        for line in text.lines() {
            let mut f = line.split(':');
            let (Some(name), Some(_pw), Some(uid)) = (f.next(), f.next(), f.next()) else {
                continue;
            };
            if let Ok(uid) = uid.parse() {
                map.insert(uid, Arc::from(name));
            }
        }
    }
    map
}

impl Collector for ProcFs {
    fn notes(&self) -> Vec<String> {
        self.notes.clone()
    }

    fn collect(&mut self, needs: Needs) -> io::Result<Sample> {
        let now = SystemTime::now();
        let elapsed = self
            .prev_at
            .and_then(|p| now.duration_since(p).ok())
            .unwrap_or(Duration::ZERO);
        self.prev_at = Some(now);

        // Before anything fallible. `prev_at` has already moved, so if a later
        // read fails with `?` the next successful sample measures one interval
        // of wall clock against two intervals of counters — every disk rate
        // roughly doubled, and `util` silently clamping to 100 on a device that
        // was never busy. Taking the snapshot here keeps the two in step
        // whatever happens below.
        let disks = self.read_diskstats(elapsed);

        let mut io_denied = 0;
        let procs = self.read_procs(elapsed, needs, &mut io_denied)?;
        // `/proc/stat` is read *after* the process walk, not before, so every
        // process in `procs` is guaranteed to have been counted by `forks`.
        // Read first, a task created during the walk appeared in `procs`
        // without being in `forks` — and on the next sample it was counted as
        // created while already present in both process lists, fabricating a
        // `1 task came and went`. A small permanent floor under a figure whose
        // whole value is that it is exact.
        //
        // Free: the file was being read here either way, and the CPU delta is
        // taken between consecutive reads, so moving both by a millisecond
        // changes nothing about it.
        let stat = self.read_stat_file()?;
        Ok(Sample {
            at: now,
            cpu_total: stat.busy,
            cpu_per_core: stat.per_core,
            iowait: Some(stat.iowait),
            running: stat.running,
            blocked: stat.blocked,
            mem: self.read_mem()?,
            load: self.read_load()?,
            procs,
            uptime: self.read_uptime()?,
            forks: stat.forks,
            io_supported: self.io_supported,
            io_collected: needs.io && self.io_supported,
            io_denied,
            disks,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a parse context from a collector, so the tests exercise the same
    /// state the collector would hand the parser.
    fn ctx(pf: &ProcFs) -> StatCtx<'_> {
        StatCtx {
            prev_jiffies: &pf.prev_proc_jiffies,
            ticks_per_sec: pf.ticks_per_sec,
            page_size: pf.page_size,
        }
    }

    #[test]
    fn cpu_times_skips_guest_double_count() {
        // user nice system idle iowait irq softirq steal guest guest_nice
        let t = CpuTimes::parse(" 100 10 50 800 20 5 5 10 999 999").unwrap();
        assert_eq!(t.total, 100 + 10 + 50 + 800 + 20 + 5 + 5 + 10);
        assert_eq!(t.idle, 820);
        // iowait is inside idle *and* kept separately: the CPU had nothing to
        // run, and the reason is the thing worth reporting.
        assert_eq!(t.iowait, 20);
    }

    #[test]
    fn busy_pct_uses_deltas() {
        let a = CpuTimes {
            idle: 900,
            total: 1000,
            iowait: 0,
        };
        let b = CpuTimes {
            idle: 950,
            total: 1100,
            iowait: 0,
        };
        // 100 jiffies passed, 50 idle -> 50% busy
        assert!((b.busy_pct_since(&a) - 50.0).abs() < 0.01);
    }

    #[test]
    fn iowait_is_reported_against_the_same_denominator_as_busy() {
        // So the two can be read against each other: 20% busy and 30% waiting
        // is a machine doing nothing while unable to get on with anything.
        let a = CpuTimes {
            idle: 900,
            total: 1000,
            iowait: 100,
        };
        let b = CpuTimes {
            idle: 980,
            total: 1100,
            iowait: 130,
        };
        // 100 jiffies passed: 80 idle (30 of it waiting), 20 busy.
        assert!((b.busy_pct_since(&a) - 20.0).abs() < 0.01);
        assert!((b.iowait_pct_since(&a) - 30.0).abs() < 0.01);
    }

    #[test]
    fn iowait_never_counts_as_busy() {
        // Folding it in would overstate CPU on exactly the machine that most
        // needs reading carefully — one that is stalled rather than working.
        let a = CpuTimes {
            idle: 0,
            total: 0,
            iowait: 0,
        };
        let b = CpuTimes {
            idle: 100,
            total: 100,
            iowait: 100,
        };
        assert_eq!(
            b.busy_pct_since(&a),
            0.0,
            "a wholly stalled CPU read as busy"
        );
        assert!((b.iowait_pct_since(&a) - 100.0).abs() < 0.01);
    }

    #[test]
    fn first_read_reports_zero_not_garbage() {
        let a = CpuTimes {
            idle: 900,
            total: 1000,
            iowait: 0,
        };
        assert_eq!(a.busy_pct_since(&a), 0.0);
    }

    #[test]
    fn proc_stat_survives_parens_in_process_name() {
        let pf = ProcFs::new().unwrap();
        let mut seen = HashMap::new();
        // A process literally named "(evil) proc)" — the case that breaks
        // naive whitespace splitting.
        let line = "1234 ((evil) proc)) S 1 0 0 0 -1 4194560 100 0 0 0 \
                    55 45 0 0 20 0 8 0 12345 1000 512 0 0 0 0 0 0 0 0 0 0 0 0";
        let p = parse_proc_stat(
            1234,
            line,
            1.0,
            Arc::from("root"),
            &mut seen,
            &mut HashMap::new(),
            &ctx(&pf),
        )
        .unwrap();
        assert_eq!(p.name.as_ref(), "(evil) proc)");
        assert_eq!(p.ppid, 1);
        assert_eq!(
            p.threads,
            Some(8),
            "the thread count from /proc stat field 20"
        );
        assert_eq!(p.state, 'S');
    }

    #[test]
    fn proc_cpu_is_zero_without_a_previous_reading() {
        let pf = ProcFs::new().unwrap();
        let mut seen = HashMap::new();
        let line = "1 (init) S 0 0 0 0 -1 4194560 100 0 0 0 \
                    55 45 0 0 20 0 1 0 12345 1000 512 0 0 0 0 0 0 0 0 0 0 0 0";
        let p = parse_proc_stat(
            1,
            line,
            1.0,
            Arc::from("root"),
            &mut seen,
            &mut HashMap::new(),
            &ctx(&pf),
        )
        .unwrap();
        assert_eq!(p.cpu, 0.0);
        assert_eq!(seen.get(&1), Some(&100)); // 55 + 45 recorded for next time
    }

    /// A stat line with a chosen pid, name and start time.
    fn stat_line(pid: i32, comm: &str, starttime: u64) -> String {
        // Fields after the comm: state ppid pgrp session tty tpgid flags minflt
        // cminflt majflt cmajflt utime stime cutime cstime prio nice threads
        // itrealvalue starttime vsize rss ...
        format!(
            "{pid} ({comm}) S 1 0 0 0 -1 0 0 0 0 0 55 45 0 0 20 0 1 0 {starttime} 1000 512 \
             0 0 0 0 0 0 0 0 0 0 0 0"
        )
    }

    #[test]
    fn a_reused_pid_does_not_inherit_the_old_name() {
        // This is why the cache is keyed on the start time as well as the pid.
        // Without it the interning is not an optimisation, it is a bug that
        // reports one process under another's name.
        let pf = ProcFs::new().unwrap();
        let mut names = HashMap::new();
        let mut seen = HashMap::new();

        let first = stat_line(4242, "postgres", 111);
        let a = parse_proc_stat(
            4242,
            &first,
            1.0,
            Arc::from("root"),
            &mut seen,
            &mut names,
            &ctx(&pf),
        )
        .unwrap();
        assert_eq!(a.name.as_ref(), "postgres");

        // Same pid, different process.
        let second = stat_line(4242, "nginx", 999);
        let b = parse_proc_stat(
            4242,
            &second,
            1.0,
            Arc::from("root"),
            &mut seen,
            &mut names,
            &ctx(&pf),
        )
        .unwrap();
        assert_eq!(b.name.as_ref(), "nginx", "a recycled pid kept the old name");
    }

    #[test]
    fn an_unchanged_process_shares_one_allocation() {
        // The point of the change: the same process across samples must hand
        // back the same `Arc`, not an equal string.
        let pf = ProcFs::new().unwrap();
        let mut names = HashMap::new();
        let mut seen = HashMap::new();
        let line = stat_line(7, "some-daemon", 555);

        let a = parse_proc_stat(
            7,
            &line,
            1.0,
            Arc::from("root"),
            &mut seen,
            &mut names,
            &ctx(&pf),
        )
        .unwrap();
        let b = parse_proc_stat(
            7,
            &line,
            1.0,
            Arc::from("root"),
            &mut seen,
            &mut names,
            &ctx(&pf),
        )
        .unwrap();
        assert!(
            Arc::ptr_eq(&a.name, &b.name),
            "the name was reallocated for an unchanged process"
        );
    }

    #[test]
    fn parses_block_layer_bytes_not_syscall_bytes() {
        let text = "rchar: 999\nwchar: 888\nsyscr: 7\nsyscw: 3\n\
                    read_bytes: 4096\nwrite_bytes: 8192\ncancelled_write_bytes: 512\n";
        assert_eq!(parse_proc_io(text), Some((4096, 8192)));
    }

    #[test]
    fn io_parse_rejects_a_truncated_file() {
        assert_eq!(parse_proc_io("rchar: 1\nwchar: 2\n"), None);
    }

    #[test]
    fn io_rate_needs_a_previous_reading() {
        let pf = ProcFs::new().unwrap();
        let mut seen = HashMap::new();
        // A pid that cannot be read is Err — the case root would fix.
        assert!(
            read_proc_io(
                -1,
                1.0,
                &mut seen,
                &pf.prev_proc_io,
                &mut String::new(),
                &mut Vec::new()
            )
            .is_err()
        );
    }

    #[test]
    fn a_process_that_exited_is_not_counted_as_needing_root() {
        // The two failures mean opposite things and the kernel distinguishes
        // them. Collapsing them made a process that vanished between the
        // directory listing and the read into one more process that "needs
        // root" — in a figure poptop prints, and which item 0022 turned into a
        // decision about whether to show the columns at all.
        let mut seen = HashMap::new();
        let prev = HashMap::new();
        let mut path = String::new();
        let mut buf = Vec::new();

        // A pid that cannot exist: the kernel says NotFound.
        let gone = read_proc_io(i32::MAX, 1.0, &mut seen, &prev, &mut path, &mut buf);
        assert!(
            matches!(gone, Err(Unreadable::Moot)),
            "an exited process was counted as needing privileges"
        );

        // Our own is readable, whoever we are.
        let ours = std::process::id() as i32;
        assert!(
            read_proc_io(ours, 1.0, &mut seen, &prev, &mut path, &mut buf).is_ok(),
            "could not read our own io"
        );
    }

    #[test]
    fn only_a_permission_failure_is_counted() {
        // Asked of the classifier rather than restated in the test: an earlier
        // version of this reimplemented the match and asserted its own copy,
        // which would have passed with the real one saying anything at all.
        assert!(Unreadable::from_kind(io::ErrorKind::PermissionDenied).counts());
        for moot in [
            io::ErrorKind::NotFound,
            io::ErrorKind::InvalidData,
            io::ErrorKind::Other,
            io::ErrorKind::Interrupted,
        ] {
            assert!(
                !Unreadable::from_kind(moot).counts(),
                "{moot:?} was counted as needing root"
            );
        }
    }

    #[test]
    fn turning_io_off_forgets_its_counters() {
        // Rates are a cumulative counter diffed against the previous read and
        // divided by one interval. Counters kept while collection is off would
        // be diffed across the whole gap on the frame it resumes — five
        // minutes of bytes reported as one second of rate, on every long-lived
        // process at once — and a pid reused meanwhile would diff against a
        // stranger's total.
        //
        // This could not happen before the IO probe existed: the ratchet only
        // ever went off to on.
        let mut pf = ProcFs::new().unwrap();
        let mut denied = 0;
        pf.read_procs(Duration::from_secs(1), Needs { io: true }, &mut denied)
            .unwrap();
        assert!(
            !pf.prev_proc_io.is_empty(),
            "collecting IO recorded no counters"
        );

        pf.read_procs(Duration::from_secs(1), Needs { io: false }, &mut denied)
            .unwrap();
        assert!(
            pf.prev_proc_io.is_empty(),
            "counters survived collection being switched off"
        );
    }

    #[test]
    fn memory_is_parsed_into_a_composition_that_accounts_for_the_machine() {
        // Against a fixed input, not against a second read of the live file.
        // The earlier version compared the collector's figures with its own
        // re-read of /proc/meminfo and failed two runs in six, because memory
        // moves between them — the same trap that ruled out deriving the page
        // size from `statm` against `status`.
        let m = parse_meminfo(
            "MemTotal:       16384000 kB\n\
             MemFree:         1024000 kB\n\
             MemAvailable:    8192000 kB\n\
             Buffers:          100000 kB\n\
             SwapTotal:       4096000 kB\n\
             SwapFree:        3096000 kB\n",
        );
        assert_eq!(m.total, 16_384_000 * 1024);
        assert_eq!(m.available, 8_192_000 * 1024);
        assert_eq!(m.free, Some(1_024_000 * 1024), "MemFree was not read");
        assert_eq!(m.used, (16_384_000 - 8_192_000) * 1024);
        assert_eq!(m.cache(), Some((8_192_000 - 1_024_000) * 1024));
        assert_eq!(m.swap_used, (4_096_000 - 3_096_000) * 1024);

        let (parts, has_cache) = m.composition();
        assert!(has_cache);
        assert_eq!(
            parts.iter().sum::<u64>(),
            m.total,
            "the segments lose memory"
        );
    }

    #[test]
    fn free_is_clamped_to_available_so_the_cache_segment_cannot_wrap() {
        // The two are computed differently from one snapshot, and on a box with
        // almost no cache the estimate can land just under free.
        let m = parse_meminfo("MemTotal:  1000 kB\nMemFree:  900 kB\nMemAvailable:  800 kB\n");
        assert_eq!(m.free, Some(800 * 1024));
        assert_eq!(m.cache(), Some(0), "the cache segment went negative");
        assert_eq!(m.composition().0.iter().sum::<u64>(), m.total);
    }

    #[test]
    fn a_meminfo_that_is_missing_a_field_does_not_invent_one() {
        let m = parse_meminfo("MemTotal:  1000 kB\n");
        assert_eq!(m.total, 1000 * 1024);
        assert_eq!(m.available, 0);
        assert_eq!(m.swap_total, 0, "swap was invented");
        // Everything is used when nothing is available, which is what the file
        // said rather than a guess about what it meant.
        assert_eq!(m.used, m.total);
    }

    #[test]
    fn the_live_meminfo_parses_into_something_coherent() {
        // The live read still gets a look, because a parse that is right about
        // a fixture and wrong about the real file would pass everything above.
        let mut pf = ProcFs::new().unwrap();
        let m = pf.read_mem().unwrap();
        assert!(m.total > 0, "no total memory");
        assert!(m.available <= m.total, "available exceeds total");
        assert!(
            m.free.is_some_and(|f| f <= m.available),
            "free exceeds available"
        );
        assert_eq!(m.composition().0.iter().sum::<u64>(), m.total);
        // Presence, not equality. Every relationship above still holds when
        // `MemFree` fails to parse and free is zero — which is exactly the
        // misreading the memory bar exists to prevent, so the live file has to
        // be asked as well. Compared as "both non-zero" rather than as figures,
        // because the two reads are microseconds apart and memory moves in
        // between: that is what made an earlier version of this flaky.
        let raw = std::fs::read_to_string("/proc/meminfo").unwrap();
        let file_free: u64 = raw
            .lines()
            .find_map(|l| {
                l.strip_prefix("MemFree:")?
                    .split_whitespace()
                    .next()?
                    .parse()
                    .ok()
            })
            .unwrap_or(0);
        if file_free > 0 {
            assert!(
                m.free.is_some_and(|f| f > 0),
                "the file reports free memory and the parse did not"
            );
        }
    }

    /// The counter fields of a `/proc/diskstats` line, from a real one.
    fn disk_line(reads: u64, writes: u64, io_ms: u64) -> String {
        // reads merged sect_r rd_ms writes merged sect_w wr_ms inflight io_ms
        // weighted, then the discard and flush fields a modern kernel adds.
        format!(
            "{reads} 0 {} 500 {writes} 0 {} 1500 0 {io_ms} 9000 0 0 0 0 0 0",
            reads * 8,
            writes * 8
        )
    }

    #[test]
    fn disk_rates_are_deltas_over_the_interval() {
        let prev = DiskTimes::parse(&disk_line(100, 200, 1000));
        let now = DiskTimes::parse(&disk_line(140, 320, 1500));
        let d = rates(&Arc::from("vda"), &prev, &now, 2.0);

        assert_eq!(d.reads, 20, "40 reads over 2s");
        assert_eq!(d.writes, 60, "120 writes over 2s");
        // 40 reads x 8 sectors x 512 bytes over 2s.
        // Written out, not computed from `SECTOR`: an expectation derived
        // from the constant under test moves with it and asserts nothing.
        // 40 reads x 8 sectors x 512 bytes over 2s.
        assert_eq!(d.read, 81_920, "read bytes/s");
        assert_eq!(d.write, 245_760, "write bytes/s");
        // 500ms busy in 2s of wall clock.
        assert!((d.util - 25.0).abs() < 0.01, "util was {}", d.util);
    }

    #[test]
    fn a_device_that_completed_nothing_reports_no_await_rather_than_zero() {
        // A mean of no samples is not zero, and zero here would read as an
        // infinitely fast disk — the most flattering possible lie about the
        // figure most worth trusting.
        let same = DiskTimes::parse(&disk_line(100, 200, 1000));
        let d = rates(&Arc::from("vda"), &same, &same, 1.0);
        assert_eq!(d.await_ms, None, "an idle device claimed a service time");
        assert_eq!(d.reads + d.writes, 0);

        let busy = DiskTimes::parse(&disk_line(110, 200, 1000));
        let d = rates(&Arc::from("vda"), &same, &busy, 1.0);
        // 10 reads, and the fixture puts read service time at a flat 500ms.
        assert_eq!(d.await_ms, Some(0.0), "a real zero was discarded");
    }

    #[test]
    fn utilisation_cannot_exceed_a_full_interval() {
        // The counter is in whole milliseconds and the interval is measured, so
        // rounding can put a fully busy device a hair over 100 — which would
        // then overflow the bar drawn from it.
        let prev = DiskTimes::parse(&disk_line(0, 0, 0));
        let now = DiskTimes::parse(&disk_line(1, 1, 1100));
        let d = rates(&Arc::from("vda"), &prev, &now, 1.0);
        assert_eq!(d.util, 100.0, "util ran past a full interval");
    }

    #[test]
    fn a_short_diskstats_line_from_an_older_kernel_still_parses() {
        // Kernels before 4.18 publish 14 fields, not 20; the extra six are
        // discard and flush accounting nothing here reads.
        let short = "100 0 800 500 200 0 1600 1500 0 1000 9000";
        let long = disk_line(100, 200, 1000);
        let a = DiskTimes::parse(short);
        let b = DiskTimes::parse(&long);
        assert_eq!(a.reads, b.reads);
        assert_eq!(a.writes, b.writes);
        assert_eq!(a.io_ms, b.io_ms);
        assert_eq!(a.sectors_written, b.sectors_written);
    }

    #[test]
    fn a_device_that_has_never_done_anything_is_not_listed() {
        // The filter that keeps 40 idle `ram`/`loop`/`nbd` devices out of the
        // table, and a measurement rather than a rule about names — a loop
        // device actually backing something stays.
        assert!(!DiskTimes::parse(&disk_line(0, 0, 0)).ever_used());
        assert!(DiskTimes::parse(&disk_line(0, 1, 0)).ever_used());
        assert!(DiskTimes::parse(&disk_line(1, 0, 0)).ever_used());
    }

    /// A whole `/proc/diskstats` file: one busy disk, its partition, an idle
    /// device, and a second disk.
    fn diskstats_fixture(reads: u64, writes: u64, io_ms: u64) -> String {
        format!(
            "254 0 vda {}\n254 1 vda1 {}\n1 0 ram0 {}\n254 16 vdb {}\n",
            disk_line(reads, writes, io_ms),
            disk_line(reads, writes, io_ms),
            disk_line(0, 0, 0),
            disk_line(reads / 2, writes / 2, io_ms / 2),
        )
    }

    /// A collector that believes `vda` and `vdb` are whole devices, as
    /// `/sys/block` would say.
    fn pf_with_devices() -> ProcFs {
        let mut pf = ProcFs::new().unwrap();
        pf.prev_disks.clear();
        pf.partitions.clear();
        pf.block_devices = ["vda", "vdb", "ram0"]
            .iter()
            .map(|s| Arc::from(*s))
            .collect();
        pf
    }

    #[test]
    fn a_partition_is_not_listed_beside_the_disk_it_belongs_to() {
        // Its IO is already inside the disk's counters, so showing both invites
        // the reader to add them up.
        let mut pf = pf_with_devices();
        pf.diskstats_from(&diskstats_fixture(100, 200, 1000), Duration::from_secs(1));
        let out = pf.diskstats_from(&diskstats_fixture(140, 320, 1500), Duration::from_secs(1));
        let names: Vec<&str> = out.iter().map(|d| &*d.name).collect();
        assert_eq!(names, vec!["vda", "vdb"], "expected the two whole disks");
    }

    #[test]
    fn a_device_that_has_never_worked_is_left_out_of_the_table() {
        // `ram0` is in `/sys/block` and has done nothing. Excluded by
        // measurement, not by its name.
        let mut pf = pf_with_devices();
        pf.diskstats_from(&diskstats_fixture(100, 200, 1000), Duration::from_secs(1));
        let out = pf.diskstats_from(&diskstats_fixture(140, 320, 1500), Duration::from_secs(1));
        assert!(
            !out.iter().any(|d| &*d.name == "ram0"),
            "an idle device was listed"
        );
    }

    #[test]
    fn the_busiest_device_sorts_first() {
        let mut pf = pf_with_devices();
        pf.diskstats_from(&diskstats_fixture(100, 200, 1000), Duration::from_secs(1));
        let out = pf.diskstats_from(&diskstats_fixture(140, 320, 1500), Duration::from_secs(1));
        assert_eq!(&*out[0].name, "vda", "the quieter disk sorted first");
        assert!(out[0].util > out[1].util);
    }

    #[test]
    fn the_first_read_has_nothing_to_subtract_from() {
        let mut pf = pf_with_devices();
        assert!(
            pf.diskstats_from(&diskstats_fixture(100, 200, 1000), Duration::from_secs(1))
                .is_empty(),
            "rates were reported from a single read"
        );
    }

    #[test]
    fn a_partition_is_only_looked_up_in_sysfs_once() {
        // Without remembering the miss, every mounted partition costs a full
        // `/sys/block` enumeration on every sample — sixty a second at the
        // interval floor, against a per-sample budget of about a millisecond.
        let mut pf = pf_with_devices();
        assert!(!pf.is_whole_device("vda1"));
        assert!(
            pf.partitions.iter().any(|p| &**p == "vda1"),
            "the miss was not remembered, so it will be re-read every sample"
        );
        // And the answer does not change on the second ask.
        assert!(!pf.is_whole_device("vda1"));
    }

    #[test]
    fn the_live_diskstats_reads_devices_that_agree_with_the_file() {
        // The fixtures above prove the arithmetic; this proves the parse is
        // pointed at the right file, the right columns, and the right devices.
        let mut pf = ProcFs::new().unwrap();
        // First call primes the counters and returns nothing to subtract from.
        assert!(
            pf.read_diskstats(Duration::from_secs(1))
                .unwrap()
                .is_empty()
        );
        std::thread::sleep(Duration::from_millis(50));
        let disks = pf
            .read_diskstats(Duration::from_millis(50))
            .expect("/proc/diskstats is readable on this kernel");

        let raw = fs::read_to_string("/proc/diskstats").unwrap();
        for d in &disks {
            let line = raw
                .lines()
                .find(|l| l.split_whitespace().nth(2) == Some(&*d.name))
                .unwrap_or_else(|| panic!("device not in the file: {}", d.name));
            assert!(
                (0.0..=100.0).contains(&d.util),
                "{} util {}",
                d.name,
                d.util
            );
            assert!(d.queue >= 0.0, "{} negative queue", d.name);

            // Every reported device has actually done something. This container
            // publishes forty-odd whole devices — `ram0..15`, `loop0..7`,
            // `nbd0..15` — of which two have ever completed an operation.
            let f: Vec<u64> = line
                .split_whitespace()
                .skip(3)
                .map(|x| x.parse().unwrap_or(0))
                .collect();
            assert!(
                f[0] > 0 || f[4] > 0,
                "{} has never completed an operation and was listed anyway",
                d.name
            );

            // And each is a whole device, not a partition whose IO is already
            // inside its disk's counters.
            if !pf.block_devices.is_empty() {
                assert!(
                    pf.block_devices.contains(&d.name),
                    "{} is a partition and would double-count its disk",
                    d.name
                );
            }
        }

        // The filters together have to remove something, or the assertions
        // above are satisfied by a machine that had nothing to exclude.
        assert!(
            disks.len() < raw.lines().count(),
            "every one of {} diskstats lines was reported",
            raw.lines().count()
        );
    }

    #[test]
    fn a_reused_pid_produces_an_absurd_delta_for_the_model_to_catch() {
        let mut pf = ProcFs::new().unwrap();
        pf.prev_cores = vec![CpuTimes::default(); 4];
        // The previous occupant of this pid had barely run; the new one shows a
        // huge cumulative counter, so the naive delta is ~500000%.
        pf.prev_proc_jiffies.insert(7, 1);
        let mut seen = HashMap::new();
        let line = "7 (reused) S 0 0 0 0 -1 0 0 0 0 0 \
                    500000 1 0 0 20 0 1 0 1 1000 512 0 0 0 0 0 0 0 0 0 0 0 0";
        let p = parse_proc_stat(
            7,
            line,
            1.0,
            Arc::from("root"),
            &mut seen,
            &mut HashMap::new(),
            &ctx(&pf),
        )
        .unwrap();
        // Left as the raw delta here. The ceiling that catches it belongs to
        // the model, so this test's job is to show the parser really does
        // produce the figure the ceiling exists for — a clamp applied to an
        // input that never exceeds it is a clamp nobody can tell is working.
        assert!(
            p.cpu > 4.0 * 100.0,
            "the delta a pid reuse produces no longer exceeds the ceiling: {}",
            p.cpu
        );
    }

    #[test]
    fn proc_cpu_from_jiffy_delta() {
        let mut pf = ProcFs::new().unwrap();
        pf.prev_cores = vec![CpuTimes::default(); 4];
        pf.prev_proc_jiffies.insert(1, 50);
        let mut seen = HashMap::new();
        // 100 total jiffies now, 50 before -> 50 jiffies in 1s at 100Hz = 50%
        let line = "1 (init) S 0 0 0 0 -1 4194560 100 0 0 0 \
                    55 45 0 0 20 0 1 0 12345 1000 512 0 0 0 0 0 0 0 0 0 0 0 0";
        let p = parse_proc_stat(
            1,
            line,
            1.0,
            Arc::from("root"),
            &mut seen,
            &mut HashMap::new(),
            &ctx(&pf),
        )
        .unwrap();
        assert!((p.cpu - 50.0).abs() < 0.01);
    }
}

#[cfg(test)]
mod page_size_tests {
    use super::*;

    /// One auxv entry, in the native layout.
    fn entry(key: usize, value: usize) -> Vec<u8> {
        let mut v = key.to_ne_bytes().to_vec();
        v.extend_from_slice(&value.to_ne_bytes());
        v
    }

    #[test]
    fn the_page_size_is_read_from_the_kernel_not_assumed() {
        // RSS is a count of pages multiplied by this, so a wrong constant
        // reports every process at a quarter of its real memory on a 16 KiB
        // kernel — and nothing about the output looks wrong.
        //
        // Where auxv is absent the fallback is the documented behaviour, so
        // that is what gets asserted: a test that panics on the machines a
        // fallback exists for is testing the wrong thing.
        let Ok(raw) = std::fs::read("/proc/self/auxv") else {
            let (size, notes) = read_page_size();
            assert_eq!(size, 4096);
            assert_eq!(notes.len(), 1, "assumed silently where auxv is absent");
            return;
        };

        let (size, notes) = read_page_size();
        assert!(notes.is_empty(), "could not read it here: {notes:?}");
        // Against the file rather than against 4096: a test that hardcodes the
        // answer cannot fail on the machines this exists for.
        assert_eq!(size, parse_page_size(&raw).expect("auxv carries AT_PAGESZ"));
        assert!(size.is_power_of_two() && size >= 4096);
    }

    #[test]
    fn an_auxv_that_says_nothing_useful_is_not_believed() {
        // A read that went wrong looks exactly like an exotic machine, so the
        // figure is bounded by what a page can actually be.
        assert_eq!(parse_page_size(&entry(6, 16384)), Some(16384));
        assert_eq!(parse_page_size(&entry(6, 65536)), Some(65536));

        let mut later = entry(3, 0x40_0000);
        later.extend(entry(6, 4096));
        assert_eq!(parse_page_size(&later), Some(4096), "later keys are missed");

        for bad in [
            entry(6, 0),
            entry(6, 3000),    // not a power of two
            entry(6, 1024),    // smaller than any architecture
            entry(6, 2 << 20), // a huge page, which is not the page size
            entry(3, 4096),    // the right value under the wrong key
            Vec::new(),
            vec![0u8; 3], // truncated past a whole pair
        ] {
            assert_eq!(parse_page_size(&bad), None, "believed {bad:?}");
        }

        // A zero key terminates the vector, so nothing after it is read.
        let mut after_end = entry(0, 0);
        after_end.extend(entry(6, 16384));
        assert_eq!(parse_page_size(&after_end), None);
    }

    #[test]
    fn a_kernel_that_will_not_say_is_reported_rather_than_guessed_at_silently() {
        // The fallback is almost always right, which is exactly why it has to
        // be announced: an assumption nobody hears about is indistinguishable
        // from a wrong number.
        for absent in [None, Some(&[][..]), Some(&entry(3, 4096)[..])] {
            let (size, notes) = page_size_from(absent);
            assert_eq!(size, 4096, "the fallback is not the common case");
            assert_eq!(notes.len(), 1, "assumed silently");
            assert!(notes[0].contains("4096"), "{}", notes[0]);
            assert!(
                notes[0].contains("wrong by a whole factor"),
                "the note does not say what is at stake: {}",
                notes[0]
            );
        }
        // …and a kernel that does say gets no note at all.
        assert!(page_size_from(Some(&entry(6, 16384))).1.is_empty());
    }
}
