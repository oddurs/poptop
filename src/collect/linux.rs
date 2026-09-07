//! Linux backend: reads `/proc` directly, no dependencies.
//!
//! The kernel exposes cumulative counters, not rates. Every CPU number here is
//! a delta between the previous read and this one, which is why the collector
//! is stateful and why the very first sample reports zero busy time.

use super::{Collector, Needs};
use crate::sample::{IoRates, MemStat, ProcSample, Sample};
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
    /// since the CPU genuinely had nothing to run — but that makes ptop right
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

pub struct ProcFs {
    prev_total: Option<CpuTimes>,
    prev_cores: Vec<CpuTimes>,
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
        Ok(Self {
            prev_total: None,
            prev_cores: Vec::new(),
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

    fn read_mem(&mut self) -> io::Result<MemStat> {
        let text = read_into("/proc/meminfo", &mut self.buf)?;
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
        Ok(MemStat {
            total,
            // MemAvailable already accounts for reclaimable cache, so this is
            // the "really in use" figure rather than the alarming one.
            used: total.saturating_sub(available),
            available,
            // Clamped: `MemFree` and `MemAvailable` are read from the same
            // snapshot but computed differently, and on a box with almost no
            // cache the estimate can land just under free — which would make
            // the cache segment of the bar negative and wrap.
            free: Some(free.min(available)),
            swap_total,
            swap_used: swap_total.saturating_sub(swap_free),
        })
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
            prev_cores,
            ..
        } = self;
        let ctx = StatCtx {
            prev_jiffies: prev_proc_jiffies,
            ticks_per_sec: *ticks_per_sec,
            page_size: *page_size,
            cores: prev_cores.len().max(1),
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
            if needs.io && !p.is_kernel_thread() {
                match read_proc_io(pid, elapsed_secs, &mut seen_io, prev_proc_io, path, buf) {
                    Ok(rates) => p.io = rates,
                    Err(()) => *denied += 1,
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

/// Per-process disk throughput from `/proc/<pid>/io`.
///
/// That file is mode 0400 and owned by the process owner, so reading another
/// user's process needs CAP_SYS_PTRACE. Unreadable means no figure, never zero
/// — showing every process you do not own as idle would be a confident lie,
/// where a blank is merely an absence.
///
/// `Err(())` means the file could not be read — running as root would fix it.
/// `Ok(None)` means the process is too new to have a previous counter to diff
/// against, which fixes itself on the next sample. Both render as a dash, but
/// only one is worth advising the user about.
fn read_proc_io(
    pid: i32,
    elapsed_secs: f64,
    seen: &mut HashMap<i32, (u64, u64)>,
    prev: &HashMap<i32, (u64, u64)>,
    path: &mut String,
    buf: &mut Vec<u8>,
) -> Result<Option<IoRates>, ()> {
    path.clear();
    let _ = write!(path, "/proc/{pid}/io");
    let text = read_into(path, buf).map_err(|_| ())?;
    let (read, write) = parse_proc_io(text).ok_or(())?;
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
            let pct = ((dj / ctx.ticks_per_sec / elapsed_secs) * 100.0) as f32;
            // Clamp to the total the machine can actually deliver. A pid reused
            // between samples diffs the new process against the old one's
            // counter and can otherwise report thousands of percent. htop
            // guards the same way.
            pct.min(ctx.cores as f32 * 100.0)
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
        threads,
        state,
        started: starttime,
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
    cores: usize,
}

/// Starting size of the shared read buffer.
///
/// A floor, not a cap: the buffer ratchets up to the largest `/proc` file seen
/// and never shrinks, so on a many-core machine `/proc/stat` alone will push it
/// past this on the first sample and every later read carries the larger
/// allocation. That is bounded by the largest file ptop reads — a few
/// kilobytes — and is the price of never reallocating during a sample.
const READ_BUF: usize = 8192;

/// Read a file into `buf` and return it as text.
///
/// `File::open` + `read` rather than `fs::read_to_string`, which calls
/// `metadata()` to size its buffer. `/proc` reports every file as zero bytes,
/// so that `statx` is pure waste — and because the size comes back as zero the
/// buffer then grows a page at a time, which is where 8.5 reads per process
/// came from. htop does the same thing with `openat` on a cached directory fd
/// and one `read` into a fixed buffer.
/// The size of a page on this kernel, and what to say if we had to guess.
///
/// Read rather than assumed, because `/proc/<pid>/stat` reports RSS as a count
/// of pages and the multiplication is the whole figure. A 16 KiB-page kernel
/// — Asahi, several ARM distributions — would otherwise report every process's
/// memory at a quarter of its real size, and a 64 KiB one, which has been the
/// RHEL aarch64 default, at a sixteenth. Nothing about the output would look
/// wrong.
///
/// `/proc/self/smaps` states it exactly on its first mapping. Deriving it
/// instead from `statm` pages against `status` `VmRSS` does not work: the two
/// files are read microseconds apart and RSS moves in between, which measured
/// 4084 against a real 4096.
///
/// Read once, into its own buffer rather than the collector's — smaps is the
/// largest file this backend touches and there is no reason to carry that
/// capacity for the rest of the run.
fn read_page_size() -> (u64, Vec<String>) {
    let mut buf = Vec::new();
    page_size_from(read_into("/proc/self/smaps", &mut buf).ok())
}

/// The file split out, so the fallback can be tested on a machine that can
/// read it. Same shape as `Tier::detect_from` and `config::path_from`.
fn page_size_from(smaps: Option<&str>) -> (u64, Vec<String>) {
    const ASSUMED: u64 = 4096;
    match smaps.and_then(parse_page_size) {
        Some(n) => (n, Vec::new()),
        None => (
            ASSUMED,
            vec![format!(
                "could not read the page size from /proc/self/smaps; assuming \
                 {ASSUMED} bytes. Every process memory figure is a page count \
                 times this, so they are all wrong by a whole factor if this \
                 kernel disagrees."
            )],
        ),
    }
}

/// The first `KernelPageSize:` in a smaps dump, in bytes.
fn parse_page_size(text: &str) -> Option<u64> {
    let kb: u64 = text
        .lines()
        .find_map(|l| l.strip_prefix("KernelPageSize:"))?
        .split_whitespace()
        .next()?
        .parse()
        .ok()?;
    // Sanity rather than trust: a page is a power of two between 4 KiB and
    // 64 KiB on every architecture Linux runs on, and a figure outside that is
    // a parse that went wrong rather than an exotic machine.
    (kb.is_power_of_two() && (4..=64).contains(&kb)).then(|| kb * 1024)
}

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

    fn sample(&mut self, needs: Needs) -> io::Result<Sample> {
        let now = SystemTime::now();
        let elapsed = self
            .prev_at
            .and_then(|p| now.duration_since(p).ok())
            .unwrap_or(Duration::ZERO);
        self.prev_at = Some(now);

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
            io_collected: needs.io,
            io_denied,
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
            cores: pf.prev_cores.len().max(1),
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
        assert_eq!(p.threads, 8);
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
    fn memory_reads_a_composition_that_holds_together() {
        // Against the live `/proc/meminfo`, because the parse is the thing that
        // can silently return zero — and a zero `free` makes every byte of
        // headroom look like cache the kernel is about to have to drop.
        let mut pf = ProcFs::new().unwrap();
        let m = pf.read_mem().unwrap();
        assert!(m.total > 0, "no total memory");
        assert!(m.available <= m.total, "available exceeds total");

        // Compared against the file rather than against a threshold. Asserting
        // `free > 0` conflates "the parse failed" with "this box has no free
        // pages", and the second is a legitimate state for a container to be
        // in — the test would fail for the right reason on the wrong machine.
        let raw = std::fs::read_to_string("/proc/meminfo").unwrap();
        let field = |key: &str| -> u64 {
            raw.lines()
                .find_map(|l| l.strip_prefix(key)?.split_whitespace().next()?.parse().ok())
                .map(|v: u64| v * 1024)
                .unwrap_or(0)
        };
        assert!(
            field("MemFree:") > 0,
            "the fixture file has no MemFree line"
        );
        assert_eq!(
            m.free,
            Some(field("MemFree:").min(field("MemAvailable:"))),
            "MemFree was not read"
        );

        // The partition the bar draws has to account for the whole machine.
        // Asserted on the real expression rather than on `a + (t - a) == t`,
        // which is true of any `a` and can only fail by panicking.
        let (parts, has_cache) = m.composition();
        assert!(has_cache, "Linux can separate cache from free");
        assert_eq!(
            parts.iter().sum::<u64>(),
            m.total,
            "the three segments do not account for the whole machine"
        );
    }

    #[test]
    fn proc_cpu_is_clamped_when_a_pid_is_reused() {
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
        assert_eq!(p.cpu, 400.0, "must clamp to cores * 100");
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

    #[test]
    fn the_page_size_is_read_from_the_kernel_not_assumed() {
        // RSS is a count of pages multiplied by this, so on a 16 KiB-page
        // kernel a wrong constant reports every process at a quarter of its
        // real memory — and nothing about the output looks wrong.
        let (size, notes) = read_page_size();
        assert!(notes.is_empty(), "could not read it here: {notes:?}");

        // Against the file itself rather than against 4096: a test that
        // hardcodes the answer cannot fail on the machines this exists for.
        let raw = std::fs::read_to_string("/proc/self/smaps").unwrap();
        let want: u64 = raw
            .lines()
            .find_map(|l| l.strip_prefix("KernelPageSize:"))
            .unwrap()
            .split_whitespace()
            .next()
            .unwrap()
            .parse::<u64>()
            .unwrap()
            * 1024;
        assert_eq!(size, want);
    }

    #[test]
    fn a_smaps_that_says_nothing_useful_is_not_believed() {
        // A parse that went wrong looks exactly like an exotic machine, so the
        // figure is bounded by what a page can actually be.
        assert_eq!(parse_page_size("KernelPageSize:        4 kB\n"), Some(4096));
        assert_eq!(
            parse_page_size("KernelPageSize:       64 kB\n"),
            Some(65536)
        );
        for bad in [
            "",
            "Size: 4 kB\n",
            "KernelPageSize:\n",
            "KernelPageSize:        0 kB\n",
            "KernelPageSize:        3 kB\n", // not a power of two
            "KernelPageSize:      128 kB\n", // larger than any architecture
            "KernelPageSize:  nonsense kB\n",
        ] {
            assert_eq!(parse_page_size(bad), None, "believed {bad:?}");
        }
        // The first mapping wins; later ones can be huge pages.
        assert_eq!(
            parse_page_size("KernelPageSize:       16 kB\nKernelPageSize:     2048 kB\n"),
            Some(16384)
        );
    }

    #[test]
    fn a_kernel_that_will_not_say_is_reported_rather_than_guessed_at_silently() {
        // The fallback is almost always right, which is exactly why it has to
        // be announced: an assumption nobody hears about is indistinguishable
        // from a wrong number.
        for absent in [None, Some(""), Some("Size: 4 kB\n")] {
            let (size, notes) = page_size_from(absent);
            assert_eq!(size, 4096, "the fallback is not the common case");
            assert_eq!(notes.len(), 1, "assumed silently for {absent:?}");
            assert!(notes[0].contains("4096"), "{}", notes[0]);
            assert!(
                notes[0].contains("wrong by a whole factor"),
                "the note does not say what is at stake: {}",
                notes[0]
            );
        }
        // …and a kernel that does say gets no note at all.
        assert!(
            page_size_from(Some("KernelPageSize:       16 kB\n"))
                .1
                .is_empty()
        );
    }
}
