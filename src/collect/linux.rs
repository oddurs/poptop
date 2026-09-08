//! Linux backend: reads `/proc` directly, no dependencies.
//!
//! The kernel exposes cumulative counters, not rates. Every CPU number here is
//! a delta between the previous read and this one, which is why the collector
//! is stateful and why the very first sample reports zero busy time.

use super::{Collector, Needs, Source, cgroups, taskstats};

/// Every optional source this backend reads.
pub const SUPPORTED: &[Source] = &[
    Source::Io,
    Source::Threads,
    Source::ClockPolicies,
    Source::Exited,
    Source::Cgroups,
    Source::Pss,
];
use crate::sample::{
    CgroupStat, DiskStat, FsStat, IoRates, Link, MemStat, NetStat, Pressure, ProcSample, Sample,
    Stall, ThreadSample,
};
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
    /// Time the hypervisor took for something else.
    ///
    /// On a cloud instance this is the difference between "the box is busy" and
    /// "the box is not being given a box", and there is no other way to see it.
    /// It is one field of a line already being parsed.
    steal: u64,
    /// Time given to guests, which on a hypervisor is the work rather than the
    /// overhead.
    guest: u64,
    /// Hard and soft interrupt time, kept apart. A network-heavy box's softirq
    /// time is the answer to why user time looks low while nothing is idle.
    irq: u64,
    softirq: u64,
    /// Whether the line carried the fields beyond idle and iowait. A short line
    /// is a platform that does not publish them, not one where they are zero.
    extended: bool,
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
        // The extended classes need the fields to actually be there. A line
        // that stops short is a platform that does not publish them, and a zero
        // would be poptop claiming it does — the one thing this codebase
        // refuses everywhere else.
        let extended = v.len() >= 8;
        let at = |i: usize| v.get(i).copied().unwrap_or(0);
        Some(Self {
            idle: v[3] + iowait,
            total,
            iowait,
            extended,
            irq: at(5),
            softirq: at(6),
            steal: at(7),
            // Already inside `user`, so it is *not* added to `total` — the same
            // double count the line above avoids. Reported as its own share of
            // the same denominator.
            guest: at(8) + at(9),
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

    /// Any of these counters as a share of the interval.
    ///
    /// The same denominator `busy_pct_since` uses, so every figure on the CPU
    /// line can be read against every other.
    fn share_since(&self, prev: &Self, f: impl Fn(&Self) -> u64) -> f32 {
        let dt = self.total.saturating_sub(prev.total);
        if dt == 0 {
            return 0.0;
        }
        ((f(self).saturating_sub(f(prev)) as f64 / dt as f64) * 100.0) as f32
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
    /// Raw `/proc/net/dev` counters from the previous read, per interface.
    prev_links: HashMap<Arc<str>, LinkTimes>,
    /// Previous cumulative totals for the whole-stack counters.
    /// `None` until the first read. Not a zeroed `NetTotals`: subtracting from
    /// zero turns the machine's lifetime error count into this second's, which
    /// is a large and very alarming number to invent.
    prev_net: Option<NetTotals>,
    /// The nominal maximum frequency of each frequency policy, read once.
    ///
    /// Read once and then rescanned rarely. The *ceiling* changes constantly,
    /// which is the point; what this holds is the hardware maximum, which does
    /// not — so reading it every sample would double the cost of the figure for
    /// nothing.
    ///
    /// Rarely rather than never, because the policy *set* does change: CPU
    /// hotplug is routine on cloud instances and on `cpuset`-managed hosts, and
    /// a `cpufreq` driver can load after the tool starts. Read strictly once, a
    /// machine that published no policy at launch would never show `CLK` again
    /// for the life of the process — which for a tool whose whole point is
    /// being left running is the wrong way round.
    ///
    /// Empty when the machine publishes no frequency policy at all, which is
    /// every virtualised CPU — including every machine available to test this
    /// on. The clock ceiling is then `None` for the life of the process, at no
    /// cost per sample.
    nominal_khz: Vec<(String, u64)>,
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
    /// The same, per thread, keyed by tid. Separate from the per-process map
    /// even though a main thread's tid equals its pid: the two are read at
    /// different times and only one of them exists when the thread view is off,
    /// and sharing a map would make a stale process entry look like a thread
    /// that had not moved.
    prev_task_jiffies: HashMap<i32, u64>,
    /// Cumulative fault counts from the previous sample, per pid.
    prev_proc_faults: HashMap<i32, (u64, u64)>,
    /// The exit listener, once somebody has asked for one.
    ///
    /// Opened lazily rather than at startup: it needs `CAP_NET_ADMIN` and the
    /// initial namespace, and a tool that refused to start without them would
    /// be useless on every box that has neither.
    exits: Option<taskstats::Listener>,
    /// Why there is no listener, said once. `None` before anything has tried.
    exits_why: Option<String>,
    /// The epoch second the machine booted, from `/proc/stat`'s `btime`, read
    /// once. Exit records carry an absolute time and live rows carry ticks
    /// since boot; this is what puts them on one clock.
    boot_epoch: Option<u64>,
    /// Cumulative cgroup counters from the previous sample, for the ones that
    /// are rates.
    prev_cgroups: cgroups::Prev,
    /// Whether this machine has a unified hierarchy, decided once.
    cgroup_v2: Option<bool>,
    /// Whether the previous sample read cgroups, so a reopened view starts
    /// from a fresh baseline rather than a stale one.
    cgroups_were_read: bool,
    /// Whether the PSS permission note has been said. Once per run, not once
    /// per sample.
    said_pss_denied: bool,
    /// Cumulative context-switch and interrupt counts from the previous sample.
    prev_ctxt: Option<u64>,
    prev_intr: Option<u64>,
    /// pid -> cumulative (read_bytes, write_bytes) at the previous sample.
    prev_proc_io: HashMap<i32, (u64, u64)>,
    prev_at: Option<SystemTime>,
    /// uid -> username, parsed once from /etc/passwd.
    users: HashMap<u32, Arc<str>>,
    /// pid -> (start time, name). The start time is what makes this safe: a
    /// recycled pid would otherwise inherit the dead process's name, which is
    /// a correctness bug wearing an optimisation's clothes.
    names: HashMap<i32, (u64, Arc<str>)>,
    /// pid -> (start time, command line). Keyed exactly like `names`, and for
    /// the same reason: a recycled pid must not inherit the dead process's
    /// command line.
    ///
    /// Held for the same reason `names` is, and it is mostly about allocation
    /// rather than syscalls. A command line is an `Arc<str>` shared by every
    /// retained sample: read once per process, the buffer holds 227 of them on
    /// this machine; read afresh each sample it would hold 227 x 600, and
    /// allocate 136,000 identical strings to get there.
    ///
    /// The syscall saving is real but small. Measured with `--bench` against
    /// the VM's own `/proc`, 227 processes: 630us a sample with this cache and
    /// 676us with `CMD_REFRESH` set to 1 so every process is re-read every
    /// time. Seven percent — worth having, and nowhere near the 45% a
    /// standalone loop over the same files suggested, because that loop
    /// allocated a path per read where the collector reuses one buffer.
    cmds: HashMap<i32, (u64, Option<Arc<str>>)>,
    /// The container each process is in, keyed by pid and validated on start
    /// time. No refresh slot, unlike `cmds`: a process cannot change container,
    /// so a hit is good for the process's whole life.
    containers: HashMap<i32, (u64, Option<Arc<str>>)>,
    /// Which sample this is, so `cmdline` re-reads can be spread across
    /// samples rather than all landing on one. See [`CMD_REFRESH`].
    tick: u64,
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
    /// Shares of the interval, on the same denominator as `busy`.
    steal: Option<f32>,
    guest: Option<f32>,
    irq: Option<f32>,
    softirq: Option<f32>,
    /// Context switches and interrupts since boot, to be turned into rates.
    ctxt: Option<u64>,
    intr: Option<u64>,
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
            prev_links: HashMap::new(),
            prev_net: None,
            nominal_khz: nominal_clocks(),
            block_devices: read_block_devices(),
            partitions: std::collections::HashSet::new(),
            prev_proc_jiffies: HashMap::new(),
            prev_task_jiffies: HashMap::new(),
            prev_proc_faults: HashMap::new(),
            exits: None,
            exits_why: None,
            boot_epoch: None,
            prev_cgroups: cgroups::Prev::default(),
            cgroup_v2: None,
            cgroups_were_read: false,
            said_pss_denied: false,
            prev_ctxt: None,
            prev_intr: None,
            prev_proc_io: HashMap::new(),
            prev_at: None,
            users: parse_passwd(),
            names: HashMap::new(),
            cmds: HashMap::new(),
            containers: HashMap::new(),
            tick: 0,
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
        let mut ctxt = None;
        let mut intr = None;
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
            if let Some(n) = line.strip_prefix("ctxt ") {
                ctxt = n.trim().parse().ok();
                continue;
            }
            if let Some(n) = line.strip_prefix("intr ") {
                // The first number is the total; the rest are per-IRQ counts.
                intr = n.split_whitespace().next().and_then(|v| v.parse().ok());
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

        let extended = total_now.extended;
        let (total_pct, iowait_pct, steal, guest, irq, softirq) = match *prev_total {
            Some(prev) => (
                total_now.busy_pct_since(&prev),
                total_now.iowait_pct_since(&prev),
                total_now.share_since(&prev, |t| t.steal),
                total_now.share_since(&prev, |t| t.guest),
                total_now.share_since(&prev, |t| t.irq),
                total_now.share_since(&prev, |t| t.softirq),
            ),
            None => (0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
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
            steal: extended.then_some(steal),
            guest: extended.then_some(guest),
            irq: extended.then_some(irq),
            softirq: extended.then_some(softirq),
            ctxt,
            intr,
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

    /// Stall pressure for all three resources, or `None` if the kernel does not
    /// publish it.
    ///
    /// All three or nothing: a kernel with `/proc/pressure` has all of them, so
    /// a partial read means something stranger is happening than a missing
    /// config option, and half an answer is worse than none.
    fn read_pressure(&self) -> Option<Pressure> {
        let one = |what: &str| fs::read_to_string(format!("/proc/pressure/{what}")).ok();
        pressure_from(
            one("cpu").as_deref(),
            one("io").as_deref(),
            one("memory").as_deref(),
        )
    }

    /// Traffic and health over `elapsed`, or `None` if `/proc/net/dev` cannot
    /// be read.
    ///
    /// The interface list is filtered the same way the disk list is: an
    /// interface appears once it has ever carried a byte. A laptop publishes
    /// twenty-odd, almost all of them idle `utun*` tunnels, and a measurement
    /// keeps the real one visible without a rule about names.
    fn read_net(&mut self, elapsed: Duration) -> Option<NetStat> {
        let dev = fs::read_to_string("/proc/net/dev").ok()?;
        // Absent files stay absent rather than becoming zeroes: a kernel that
        // does not publish retransmits and one reporting none are opposite
        // answers, and only `/proc/net/dev` is required for the rest to mean
        // anything.
        let snmp = fs::read_to_string("/proc/net/snmp").unwrap_or_default();
        let netstat = fs::read_to_string("/proc/net/netstat").unwrap_or_default();
        Some(self.net_from(&dev, &snmp, &netstat, elapsed))
    }

    /// Split from the read so the counters, the filter and the absent-file
    /// rules can be tested against fixtures — the same split `parse_meminfo`,
    /// `diskstats_from` and `pressure_from` have.
    fn net_from(&mut self, dev: &str, snmp: &str, netstat: &str, elapsed: Duration) -> NetStat {
        let secs = elapsed.as_secs_f64();

        let mut links = Vec::new();
        let mut seen = HashMap::new();
        // Summed from per-interface deltas, not from a whole-machine total
        // diffed against the last one. An interface that appears mid-session —
        // a NIC plugged in, a namespace settling — brings its lifetime counters
        // with it, and a difference of sums would charge all of them to this
        // second. One that disappears would shrink the sum and swallow a real
        // interval's errors as zero.
        let (mut errors, mut drops) = (0u64, 0u64);
        let mut had_baseline = false;
        for line in dev.lines() {
            let Some((name, rest)) = line.split_once(':') else {
                continue;
            };
            let name = name.trim();
            let now = parse_link(rest);
            let name: Arc<str> = self
                .prev_links
                .keys()
                .find(|k| ***k == *name)
                .cloned()
                .unwrap_or_else(|| Arc::from(name));
            if let Some(prev) = self.prev_links.get(&name) {
                had_baseline = true;
                let d = |a: u64, b: u64| a.saturating_sub(b);
                errors += d(now.rx_errs, prev.rx_errs) + d(now.tx_errs, prev.tx_errs);
                drops += d(now.rx_drop, prev.rx_drop) + d(now.tx_drop, prev.tx_drop);
                // Listed only once it has carried something, the same
                // measurement the disk table uses — but its errors are counted
                // either way, because an interface erroring and passing nothing
                // is the worst case there is.
                if secs > 0.0 && (now.rx > 0 || now.tx > 0) {
                    let rate = |a: u64, b: u64| (d(a, b) as f64 / secs) as u64;
                    links.push(Link {
                        name: name.clone(),
                        rx: rate(now.rx, prev.rx),
                        tx: rate(now.tx, prev.tx),
                        rx_packets: rate(now.rx_packets, prev.rx_packets),
                        tx_packets: rate(now.tx_packets, prev.tx_packets),
                    });
                }
            }
            seen.insert(name, now);
        }
        self.prev_links = seen;

        // Each counter carries its own baseline, so a file that was unreadable
        // last sample reports nothing this sample rather than everything since
        // boot. `None` here means the counter was not found — which is not the
        // same as a counter that found nothing.
        let totals = NetTotals {
            retrans: snmp_counter(snmp, "Tcp:", "RetransSegs"),
            listen_drops: snmp_counter(netstat, "TcpExt:", "ListenDrops"),
        };
        let prev = self.prev_net.replace(totals);
        // Both halves must be present: a counter needs a reading now *and* a
        // reading to subtract from.
        let since =
            |now: Option<u64>, was: Option<Option<u64>>| Some(now?.saturating_sub(was.flatten()?));
        NetStat {
            links,
            errors: had_baseline.then_some(errors),
            drops: had_baseline.then_some(drops),
            retrans: since(totals.retrans, prev.map(|p| p.retrans)),
            listen_drops: since(totals.listen_drops, prev.map(|p| p.listen_drops)),
        }
    }

    /// How full each mounted filesystem is.
    ///
    /// `None` when `/proc/mounts` cannot be read, or when the root filesystem
    /// does not come back with a size — which is the check that the `statfs`
    /// offsets mean what this file believes, since every machine has one and it
    /// is never empty.
    fn read_filesystems(&self) -> Option<Vec<FsStat>> {
        let text = fs::read_to_string("/proc/mounts").ok()?;
        let mut out: Vec<(String, bool, FsStat)> = Vec::new();
        for (dev, mount, writable, _kind) in mount_points(&text) {
            let Some((total, avail)) = statfs_at(&mount) else {
                continue;
            };
            // Pseudo-filesystems report no blocks at all, so they need no rule
            // about names: `proc`, `sysfs`, `cgroup2` and the rest fall out
            // here as a measurement.
            if total == 0 {
                continue;
            }
            out.push((
                dev,
                writable,
                FsStat {
                    mount: Arc::from(mount.as_str()),
                    total,
                    avail,
                },
            ));
        }
        // Bind mounts and shared containers collapse here — this container has
        // `/dev/vda1` at three paths.
        let out = super::merge_filesystems(out);
        // The root specifically, not merely "something came back". Every
        // machine has a sized filesystem there, so if `/` was rejected while
        // some other mount happened to yield a plausible block size, the
        // offsets are wrong and the figures that did come back are garbage.
        super::with_a_root(out)
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

    /// Per-cgroup figures, when the view is open.
    ///
    /// `None` when nobody asked or when this machine has no unified hierarchy.
    /// cgroup v1 has no `cpu.stat` in this shape, no unified tree and no PSI,
    /// so there is nothing to walk — said once rather than shown as an empty
    /// table, which would read as "this machine has no cgroups".
    fn read_cgroups(&mut self, needs: Needs, elapsed_secs: f64) -> Option<Vec<CgroupStat>> {
        if !needs.wants(Source::Cgroups) {
            self.cgroups_were_read = false;
            return None;
        }
        if self.cgroup_v2.is_none() {
            let ok = cgroups::v2_available();
            if !ok {
                self.notes.push(
                    "cgroup v2 is not mounted here, so per-cgroup figures are unavailable".into(),
                );
            }
            self.cgroup_v2 = Some(ok);
        }
        if self.cgroup_v2 != Some(true) {
            return None;
        }
        // Cleared when the view has been shut, so reopening it does not diff
        // against counters from ten minutes ago and render every cgroup at
        // hundreds of percent. The same reasoning as the per-process IO map.
        if !self.cgroups_were_read {
            self.prev_cgroups = cgroups::Prev::default();
        }
        self.cgroups_were_read = true;
        Some(cgroups::read(
            &mut self.prev_cgroups,
            elapsed_secs,
            cgroups::DEFAULT_DEPTH,
        ))
    }

    /// Processes that exited since the last sample.
    ///
    /// `None` when nobody asked or when the kernel refused, which are different
    /// from an interval in which nothing exited — and that difference is the
    /// whole point: a table that quietly omits what happened in an interval is
    /// worse than one that says it cannot see it.
    fn read_exited(&mut self, needs: Needs, elapsed_secs: f64) -> Option<Vec<ProcSample>> {
        if !needs.wants(Source::Exited) {
            return None;
        }
        if self.boot_epoch.is_none() {
            // `btime` in `/proc/stat` is the epoch second the machine booted.
            // Exact, and read once: a derivation from `uptime` would drift by
            // however long this sample took.
            self.boot_epoch = std::fs::read_to_string("/proc/stat").ok().and_then(|s| {
                s.lines()
                    .find_map(|l| l.strip_prefix("btime "))
                    .and_then(|v| v.trim().parse().ok())
            });
        }
        if self.exits.is_none() && self.exits_why.is_none() {
            match taskstats::Listener::open() {
                Ok(l) => self.exits = Some(l),
                Err(why) => {
                    // Said once, through the same channel as everything else
                    // the backend had to assume about this machine.
                    let why = why.why();
                    self.notes.push(why.clone());
                    self.exits_why = Some(why);
                }
            }
        }
        let boot = taskstats::Boot {
            epoch_secs: self.boot_epoch.unwrap_or(0),
            ticks_per_sec: self.ticks_per_sec,
        };
        let Self {
            exits,
            users,
            prev_proc_jiffies,
            ..
        } = self;
        exits
            .as_mut()
            .map(|l| l.drain(elapsed_secs, boot, prev_proc_jiffies, users))
    }

    /// The clock ceiling, from the policy maximum each CPU currently permits.
    ///
    /// One small read per frequency policy — a handful on any machine — and
    /// none at all on one that publishes none.
    fn read_clock_ceiling(&mut self, needs: Needs) -> Option<f32> {
        // One directory listing a minute, and only that — on the cadence the
        // source declares rather than a counter kept here. The rule and the
        // cost now live in one place with every other source's; this reads it.
        if needs.wants(Source::ClockPolicies) {
            self.nominal_khz = nominal_clocks();
        }
        if self.nominal_khz.is_empty() {
            return None;
        }
        let Self {
            nominal_khz,
            path,
            buf,
            ..
        } = self;
        let mut pairs = Vec::with_capacity(nominal_khz.len());
        for (policy, nominal) in nominal_khz.iter() {
            path.clear();
            let _ = write!(path, "{CPUFREQ}/cpufreq/{policy}/scaling_max_freq");
            let Ok(text) = read_into(path, buf) else {
                continue;
            };
            let Ok(allowed) = text.trim().parse::<u64>() else {
                continue;
            };
            pairs.push((allowed, *nominal));
        }
        ceiling_from(&pairs)
    }

    fn read_procs(
        &mut self,
        elapsed: Duration,
        needs: Needs,
        denied: &mut usize,
    ) -> io::Result<(Vec<ProcSample>, Option<Vec<ThreadSample>>)> {
        // Destructured so the shared read buffer, the path buffer and the
        // previous-sample maps are all borrowed disjointly. Going through
        // `&mut self` would hold the whole collector for as long as the text
        // read out of the buffer lives.
        let Self {
            buf,
            path,
            prev_proc_jiffies,
            prev_proc_faults,
            prev_task_jiffies,
            prev_proc_io,
            users,
            names,
            cmds,
            containers,
            tick,
            ticks_per_sec,
            page_size,
            io_supported,
            ..
        } = self;
        let ctx = StatCtx {
            prev_jiffies: prev_proc_jiffies,
            prev_faults: prev_proc_faults,
            ticks_per_sec: *ticks_per_sec,
            page_size: *page_size,
        };

        *tick = tick.wrapping_add(1);
        let tick = *tick;
        let mut out = Vec::new();
        let mut seen = HashMap::new();
        let mut seen_faults: HashMap<i32, (u64, u64)> = HashMap::new();
        let mut pss_denied = 0usize;
        let mut seen_io = HashMap::new();
        let mut tasks = Vec::new();
        let mut seen_tasks = HashMap::new();
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

            let Some(mut p) = parse_proc_stat(
                pid,
                stat,
                elapsed_secs,
                user,
                &mut seen,
                &mut seen_faults,
                names,
                &ctx,
            ) else {
                continue;
            };
            // A kernel thread's `cmdline` is empty, so the open and the read
            // buy nothing. Skipped for the same reason the IO probe skips them,
            // one branch down.
            if !p.is_kernel_thread() {
                p.cmd = cmdline(pid, p.started.unwrap_or(0), tick, cmds, path, buf);
                // Same shape as the command line and one read cheaper: cached
                // for the life of the process rather than re-read on a slot.
                p.container = container_of(pid, p.started.unwrap_or(0), containers, path);
                if needs.wants(Source::Pss) {
                    p.pss = read_pss(pid, path, buf, &mut pss_denied);
                }
            }
            // Kernel threads are skipped rather than attempted and counted as
            // denied. They are root-owned and unreadable to an ordinary user,
            // and on a many-core box they outnumber the real processes — so
            // counting them would fire the IO probe on exactly the laptop it
            // exists to protect. Skipping also saves an open and a read each.
            if needs.wants(Source::Io) && *io_supported && !p.is_kernel_thread() {
                match read_proc_io(pid, elapsed_secs, &mut seen_io, prev_proc_io, path, buf) {
                    Ok(rates) => p.io = rates,
                    // Either way the row shows an em dash. Only one of them is
                    // something root would fix, and only that one is counted.
                    Err(why) => *denied += usize::from(why.counts()),
                }
            }
            if needs.wants(Source::Threads) && p.threads.unwrap_or(1) > 1 {
                read_tasks(
                    pid,
                    &p,
                    elapsed_secs,
                    &mut seen_tasks,
                    prev_task_jiffies,
                    *ticks_per_sec,
                    &mut tasks,
                    path,
                    buf,
                );
            }
            out.push(p);
        }

        // Drop counters for processes that have exited, or the maps grow
        // without bound on a busy box.
        // Drop cached names for processes that have exited, or the map grows
        // without bound exactly like the counters would.
        names.retain(|pid, _| seen.contains_key(pid));
        cmds.retain(|pid, _| seen.contains_key(pid));
        // And the container ids, for the same reason. This one is easier to
        // forget precisely because it never expires on a slot: it is the only
        // cache here whose entries are valid for a process's whole life, which
        // is exactly why nothing else would ever remove them.
        containers.retain(|pid, _| seen.contains_key(pid));
        *prev_proc_jiffies = seen;
        // Replaced wholesale, not extended: this is next sample's baseline and
        // it self-prunes the same way the jiffies do — an `extend` would keep
        // one entry per pid ever seen.
        *prev_proc_faults = seen_faults;
        // Said once per run, through the same channel as everything else the
        // backend had to give up on. Without it a non-root reader opening the
        // memory view sees a full column of em dashes, pays 4.2us a process for
        // it, and is told nothing.
        if pss_denied > 0 && !self.said_pss_denied {
            self.said_pss_denied = true;
            self.notes.push(format!(
                "{pss_denied} processes' proportional memory needs CAP_SYS_PTRACE; \
                 run as root to see them"
            ));
        }
        // Cleared rather than kept while collection is off. Rates are a delta
        // against the previous read divided by one interval, so counters left
        // over from five minutes ago would render every long-lived process at
        // three hundred times its real rate on the frame collection resumes —
        // and a pid reused in the meantime would diff against a stranger.
        //
        // Collection could not resume before the probe existed, which is why
        // this held: the ratchet only ever went off to on.
        *prev_proc_io = if needs.wants(Source::Io) {
            seen_io
        } else {
            HashMap::new()
        };
        // Same reasoning as the IO map one line up: a jiffy count from before
        // the view was turned off would be divided by one interval and render
        // a thread at hundreds of times its real share.
        *prev_task_jiffies = if needs.wants(Source::Threads) {
            seen_tasks
        } else {
            HashMap::new()
        };
        // `None` when nobody asked, an empty list when the box genuinely has no
        // multi-threaded process. The pair is the difference between "not
        // collected" and "none found", which is the distinction this codebase
        // does not collapse anywhere else either.
        Ok((out, needs.wants(Source::Threads).then_some(tasks)))
    }
}

/// Every thread of one process, from `/proc/<pid>/task/`.
///
/// Only called for a process whose `stat` says it has more than one thread. A
/// single-threaded process *is* its thread, so a row for it would repeat the
/// process row one column narrower — and skipping them is most of what keeps
/// this affordable, since most processes on any box are single-threaded.
///
/// A thread that exits while the directory is being walked is normal, not an
/// error: the same reasoning as the process loop, one level down.
#[allow(clippy::too_many_arguments)]
fn read_tasks(
    pid: i32,
    proc: &ProcSample,
    elapsed_secs: f64,
    seen: &mut HashMap<i32, u64>,
    prev: &HashMap<i32, u64>,
    ticks_per_sec: f64,
    out: &mut Vec<ThreadSample>,
    path: &mut String,
    buf: &mut Vec<u8>,
) {
    path.clear();
    let _ = write!(path, "/proc/{pid}/task");
    let Ok(dir) = fs::read_dir(&path) else { return };
    for entry in dir {
        let Ok(entry) = entry else { continue };
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        let Ok(tid) = name.parse::<i32>() else {
            continue;
        };

        path.clear();
        let _ = write!(path, "/proc/{pid}/task/{tid}/stat");
        let Ok(stat) = read_into(path, buf) else {
            continue;
        };
        let Some(t) = parse_task_stat(pid, tid, stat, elapsed_secs, seen, prev, ticks_per_sec)
        else {
            continue;
        };
        // A thread's name is its own, but the kernel gives the main thread the
        // process's, so it repeats. Shared rather than allocated again: the
        // process's `Arc<str>` is the same string.
        out.push(ThreadSample {
            name: if tid == pid {
                proc.name.clone()
            } else {
                t.name
            },
            ..t
        });
    }
}

/// The fields of a thread's `stat`, which has the same shape as a process's.
///
/// Split from the read so it can be tested against a fixture, like every other
/// parser here.
fn parse_task_stat(
    pid: i32,
    tid: i32,
    stat: &str,
    elapsed_secs: f64,
    seen: &mut HashMap<i32, u64>,
    prev: &HashMap<i32, u64>,
    ticks_per_sec: f64,
) -> Option<ThreadSample> {
    // Field 2 is the thread name in parentheses and may contain spaces and
    // parentheses of its own, so the split is on the last ')' — the same trap,
    // and the same answer, as `parse_proc_stat`.
    let close = stat.rfind(')')?;
    let open = stat.find('(')?;
    let comm = stat.get(open + 1..close)?;
    let rest: Vec<&str> = stat.get(close + 1..)?.split_whitespace().collect();

    let state = rest.first()?.chars().next().unwrap_or('?');
    let utime: u64 = rest.get(11)?.parse().ok()?;
    let stime: u64 = rest.get(12)?.parse().ok()?;
    let jiffies = utime.saturating_add(stime);
    seen.insert(tid, jiffies);

    // No previous reading means this thread was not there last time, and a
    // delta against zero would report its whole lifetime's CPU as this
    // interval's. Zero for a first sighting, exactly as the process path does.
    let cpu = match prev.get(&tid) {
        Some(&before) => {
            let delta = jiffies.saturating_sub(before) as f64;
            ((delta / ticks_per_sec) / elapsed_secs * 100.0) as f32
        }
        None => 0.0,
    };

    Some(ThreadSample {
        pid,
        tid,
        name: Arc::from(comm),
        state,
        cpu,
    })
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

/// Each frequency policy's nominal maximum, as `(policy name, kHz)`.
///
/// Read once. A policy, not a CPU: `cpufreq` applies a ceiling to a *policy*,
/// and the `cpuN/cpufreq` directories are symlinks into the policy that governs
/// them. On a machine with one policy per package or per cluster — which is
/// most of them — that is a handful of reads a sample instead of one per core.
///
/// Measured against synthetic files, since no CPU available here publishes a
/// frequency policy at all: about a microsecond a read, so 16 cores read
/// individually cost 15us and 128 cost 150us, against a whole sample of about
/// 630us. Per policy it is a rounding error on any machine.
fn nominal_clocks() -> Vec<(String, u64)> {
    let Ok(dir) = fs::read_dir(format!("{CPUFREQ}/cpufreq")) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in dir.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str().filter(|n| n.starts_with("policy")) else {
            continue;
        };
        let path = format!("{CPUFREQ}/cpufreq/{name}/cpuinfo_max_freq");
        if let Some(khz) = fs::read_to_string(&path)
            .ok()
            .and_then(|s| s.trim().parse::<u64>().ok())
            .filter(|k| *k > 0)
        {
            out.push((name.to_string(), khz));
        }
    }
    out.sort_unstable();
    out
}

/// How many samples between rescans of the frequency policy set.
///
/// The rule now lives with every other source's, in [`Source::every`]. This is
/// the same number, kept so the test that proves the rescan happens does not
/// have to hard-code it.
#[cfg(test)]
const CLOCK_RESCAN: u64 = 60;

/// Where the kernel publishes each CPU's frequency policy.
const CPUFREQ: &str = "/sys/devices/system/cpu";

/// The clock ceiling as a percentage of nominal, or `None`.
///
/// `scaling_max_freq` is what the governor is currently allowed to reach and
/// `cpuinfo_max_freq` is what the hardware can do, so the ratio is "how much of
/// this processor am I permitted to use". Read as a ceiling rather than as
/// `scaling_cur_freq` deliberately: current frequency drops on an idle core,
/// which is a healthy machine doing nothing and is indistinguishable from a
/// throttled one.
///
/// The *most* capped core wins. Thermal capping is rarely uniform — one hot
/// package on a two-socket box is still a machine that is not going as fast as
/// it should — and a mean would average the problem away.
///
/// Split from the read so the arithmetic can be tested against fixtures, which
/// on this figure is the only way it can be tested at all: no machine
/// available here has a `cpufreq` directory, virtualised CPUs having no
/// frequency policy to publish.
fn ceiling_from(pairs: &[(u64, u64)]) -> Option<f32> {
    let mut worst: Option<f32> = None;
    for (allowed, nominal) in pairs {
        // A core that reports a nominal of zero is reporting nothing.
        if *nominal == 0 {
            continue;
        }
        let pct = (*allowed as f64 / *nominal as f64 * 100.0) as f32;
        // Above nominal is a boost ceiling, not a fault. Clamped rather than
        // dropped: the answer is "not capped", which is 100.
        let pct = pct.min(100.0);
        worst = Some(worst.map_or(pct, |w: f32| w.min(pct)));
    }
    worst
}

/// How many samples a command line is trusted for before it is read again.
///
/// A command line is not quite immutable: `setproctitle` rewrites `argv` in
/// place, which is how postgres shows `postgres: writer process` and nginx
/// shows `nginx: worker process`. Those are exactly the processes this column
/// is most useful for, so a cache that never expired would freeze the one title
/// worth watching.
///
/// Re-reads are staggered by pid rather than run all at once, so the cost is
/// one thirtieth of a full pass on every sample instead of a full pass every
/// thirtieth. At the default interval a rewritten title is at most half a
/// minute stale.
const CMD_REFRESH: u64 = 30;

/// How much of `cmdline` is read at all.
///
/// Four kilobytes against a stored two hundred characters. The slack is for the
/// reduction that happens after: the cap is on raw bytes, and `argv[0]`'s
/// directory — dropped a moment later — can be most of a long path on its own.
const CMD_READ_MAX: usize = 4096;

/// Proportional set size, from `/proc/<pid>/smaps_rollup`.
///
/// `None` where the file cannot be read, which is another user's process
/// without `CAP_SYS_PTRACE` and a process that exited while the directory was
/// being walked — the same two cases as the IO probe, and neither is an error.
/// Never zero: a process whose share could not be measured is not one using no
/// memory.
fn read_pss(pid: i32, path: &mut String, buf: &mut Vec<u8>, denied: &mut usize) -> Option<u64> {
    path.clear();
    let _ = write!(path, "/proc/{pid}/smaps_rollup");
    let text = match read_into(path, buf) {
        Ok(t) => t,
        Err(e) => {
            // Counted the way the IO probe counts its own, and for the same
            // reason: a column of em dashes with no explanation is the failure
            // this codebase does not ship. Only the permission case — a process
            // that exited while the directory was walked is normal and nothing
            // would fix it.
            if e.kind() == io::ErrorKind::PermissionDenied {
                *denied += 1;
            }
            return None;
        }
    };
    // Published in kilobytes, like the rest of that file.
    text.lines()
        .find_map(|l| l.strip_prefix("Pss:"))
        .and_then(|v| v.split_whitespace().next())
        .and_then(|v| v.parse::<u64>().ok())
        .map(|kb| kb * 1024)
}

/// Which container a process is in, from `/proc/<pid>/cgroup`.
///
/// One small read per process, once per process rather than once per sample: a
/// process cannot move between containers, so the cache is good for its whole
/// life — and the start time is what stops a recycled pid inheriting the dead
/// process's answer.
fn container_of(
    pid: i32,
    started: u64,
    cache: &mut HashMap<i32, (u64, Option<Arc<str>>)>,
    path: &mut String,
) -> Option<Arc<str>> {
    if let Some((t, c)) = cache.get(&pid)
        && *t == started
    {
        return c.clone();
    }
    path.clear();
    let _ = write!(path, "/proc/{pid}/cgroup");
    let found = fs::read_to_string(&path)
        .ok()
        .and_then(|t| cgroups::container_of(&t));
    cache.insert(pid, (started, found.clone()));
    found
}

/// The command line for a process, read at most once per `CMD_REFRESH` samples.
///
/// `None` means the file was empty or unreadable. Empty is the common case and
/// means what it says — a kernel thread has no command line — and unreadable is
/// a process that exited while we walked the directory. Neither is an error and
/// both render as the `comm` fallback, so they are not told apart here.
fn cmdline(
    pid: i32,
    started: u64,
    tick: u64,
    cache: &mut HashMap<i32, (u64, Option<Arc<str>>)>,
    path: &mut String,
    buf: &mut Vec<u8>,
) -> Option<Arc<str>> {
    // Spread by pid so every process is not re-read on the same sample.
    let due = tick % CMD_REFRESH == pid.unsigned_abs() as u64 % CMD_REFRESH;
    if let Some((t, cmd)) = cache.get(&pid)
        && *t == started
        && !due
    {
        return cmd.clone();
    }
    path.clear();
    let _ = write!(path, "/proc/{pid}/cmdline");
    // Capped, not read whole: see `read_capped`. Generous against `CMD_MAX`,
    // because the cap is on bytes before `argv[0]`'s directory is dropped and a
    // long path can eat most of it on its own.
    let cmd = read_capped(path, buf, CMD_READ_MAX)
        .ok()
        .and_then(parse_cmdline)
        .map(Arc::from);
    cache.insert(pid, (started, cmd.clone()));
    cmd
}

/// `/proc/<pid>/cmdline` into the line poptop stores.
///
/// The file is `argv` with a NUL after every element, including the last, so a
/// naive split leaves a trailing empty string. Interior empties are kept: an
/// empty argument is a real argument, and dropping it would silently rewrite
/// the command.
///
/// Lossy rather than strict: an argument that is not UTF-8 is a real argument,
/// and refusing the whole command line over one byte would lose the identity of
/// a process running perfectly well.
fn parse_cmdline(raw: &[u8]) -> Option<String> {
    let raw = raw.strip_suffix(&[0]).unwrap_or(raw);
    if raw.is_empty() {
        return None;
    }
    let argv: Vec<std::borrow::Cow<'_, str>> = raw
        .split(|b| *b == 0)
        .map(String::from_utf8_lossy)
        .collect();
    crate::sample::command_from_argv(argv.iter().map(|a| a.as_ref()))
}

/// Turn one `/proc/<pid>/stat` line into a sample.
#[allow(clippy::too_many_arguments)]
fn parse_proc_stat(
    pid: i32,
    stat: &str,
    elapsed_secs: f64,
    user: Arc<str>,
    seen: &mut HashMap<i32, u64>,
    // Where this sample's cumulative fault counts go, to be next sample's
    // baseline. Recorded here rather than derived later for the same reason the
    // jiffies are: the raw counters do not survive the ProcSample.
    seen_faults: &mut HashMap<i32, (u64, u64)>,
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
    // Fields 10 and 12, cumulative since the process started, so they become
    // rates below. Field N lives at rest[N - 3], the same arithmetic as every
    // other index here.
    let minflt: u64 = rest.get(7)?.parse().unwrap_or(0);
    let majflt: u64 = rest.get(9)?.parse().unwrap_or(0);
    // Field 19. Signed: -20 to 19.
    let nice: i32 = rest.get(16)?.parse().unwrap_or(0);
    // Field 23, in bytes already — unlike `rss`, which is in pages.
    let vsize: u64 = rest.get(20)?.parse().unwrap_or(0);
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
    seen_faults.insert(pid, (minflt, majflt));

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
        cmd: None,
        io: None,
        // Filled by the caller, which has the cache.
        container: None,
        // A rate needs two readings, and a process seen for the first time has
        // one. Zero rather than its whole lifetime's faults divided by one
        // interval — the same rule as its CPU, three fields up.
        minflt: Some(rate(
            minflt,
            ctx.prev_faults.get(&pid).map(|(m, _)| *m),
            elapsed_secs,
        )),
        majflt: Some(rate(
            majflt,
            ctx.prev_faults.get(&pid).map(|(_, m)| *m),
            elapsed_secs,
        )),
        vsize: Some(vsize),
        nice: Some(nice),
        // Filled by the caller when the source is on; `smaps_rollup` is a
        // second read and does not belong in a `stat` parser.
        pss: None,
    })
}

/// The collector state a stat line needs to become a `ProcSample`.
///
/// Passed explicitly rather than through `&self` so the shared read buffer can
/// be borrowed mutably at the same time.
struct StatCtx<'a> {
    prev_jiffies: &'a HashMap<i32, u64>,
    /// Cumulative minor and major fault counts from the previous sample, so
    /// both can be reported as rates over the interval rather than as a
    /// lifetime total that only ever goes up.
    prev_faults: &'a HashMap<i32, (u64, u64)>,
    ticks_per_sec: f64,
    page_size: u64,
}

/// A machine-wide cumulative counter as a rate, or `None`.
///
/// `None` when the platform does not publish it and on the first sighting —
/// "I do not know yet" rather than a boot's worth of context switches
/// attributed to one second.
fn delta_rate(now: Option<u64>, before: Option<u64>, elapsed_secs: f64) -> Option<u64> {
    let (now, before) = (now?, before?);
    (elapsed_secs > 0.0).then(|| (now.saturating_sub(before) as f64 / elapsed_secs) as u64)
}

/// A cumulative counter as a rate over the interval.
///
/// `None` for the previous reading means this is the first sighting, and zero
/// is the honest answer: a delta against nothing would report the process's
/// whole life in one interval.
fn rate(now: u64, before: Option<u64>, elapsed_secs: f64) -> u32 {
    match before {
        Some(was) if elapsed_secs > 0.0 => (now.saturating_sub(was) as f64 / elapsed_secs) as u32,
        _ => 0,
    }
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
    // `None` where the line is absent. For the composition figures below the
    // difference matters — a kernel built without hugetlb has no huge pages to
    // report and one with none reserved has zero of them — so this is the
    // primitive, and `get` is the lossy convenience over it rather than a
    // second parser that could disagree with it.
    let find = |key: &str| -> Option<u64> {
        text.lines().find_map(|l| {
            // meminfo is in kB.
            Some(
                l.strip_prefix(key)?
                    .split_whitespace()
                    .next()?
                    .parse::<u64>()
                    .ok()?
                    * 1024,
            )
        })
    };
    let get = |key: &str| -> u64 { find(key).unwrap_or(0) };
    let total = get("MemTotal:");
    let available = get("MemAvailable:");
    let free = get("MemFree:");
    let swap_total = get("SwapTotal:");
    let swap_free = get("SwapFree:");
    // Counted in pages of `Hugepagesize`, not in kilobytes — the one family in
    // this file that is not in kB, so reading them with `find` would report a
    // two-megabyte page as two kilobytes.
    //
    // `None` without the page size rather than a fallback of zero: a meminfo
    // carrying `HugePages_Total: 64` and no `Hugepagesize:` — a filtered one,
    // as lxcfs produces, or a truncated read — would otherwise report
    // sixty-four reserved pages as `0 B`, which is the absent-versus-zero
    // confusion this whole block exists to avoid.
    let huge_kb = text.lines().find_map(|l| {
        l.strip_prefix("Hugepagesize:")?
            .split_whitespace()
            .next()?
            .parse::<u64>()
            .ok()
    });
    let huge_pages = |key: &str| -> Option<u64> {
        let kb = huge_kb?;
        text.lines()
            .find_map(|l| {
                l.strip_prefix(key)?
                    .split_whitespace()
                    .next()?
                    .parse::<u64>()
                    .ok()
            })
            .map(|n| n * kb * 1024)
    };
    let huge_total = huge_pages("HugePages_Total:");
    let huge_free = huge_pages("HugePages_Free:");
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
        dirty: find("Dirty:"),
        slab: find("Slab:"),
        slab_reclaimable: find("SReclaimable:"),
        shmem: find("Shmem:"),
        page_tables: find("PageTables:"),
        huge_total,
        huge_used: match (huge_total, huge_free) {
            (Some(t), Some(f)) => Some(t.saturating_sub(f)),
            _ => None,
        },
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

/// One `/proc/pressure/*` file: a `some` line and, on most kernels, a `full`
/// one.
///
/// ```text
/// some avg10=3.60 avg60=2.59 avg300=6.23 total=736109049
/// full avg10=3.60 avg60=2.57 avg300=6.14 total=722044449
/// ```
///
/// Only `avg10` is taken. The longer windows are the kernel's own smoothing and
/// poptop has a timeline for that — a ten-second average is already smoothed
/// enough to read and short enough to move when the machine does.
///
/// A missing `full` line leaves it zero, which is what older kernels mean by
/// omitting it: they publish `full` for IO and memory and not for CPU, where it
/// is undefined.
fn parse_pressure(text: &str) -> Stall {
    let avg10 = |kind: &str| {
        text.lines()
            .find(|l| l.starts_with(kind))?
            .split_whitespace()
            .find_map(|f| f.strip_prefix("avg10="))?
            .parse()
            .ok()
    };
    Stall {
        some: avg10("some").unwrap_or(0.0),
        full: avg10("full").unwrap_or(0.0),
    }
}

// `statfs`, whose interesting fields sit at fixed offsets in a structure the
// *kernel* owns rather than libc — so unlike `statvfs` the layout does not
// shift between C libraries.
//
// `f_type` 0, `f_bsize` 8, `f_blocks` 16, `f_bfree` 24, `f_bavail` 32 on every
// 64-bit Linux. Checked against the root filesystem rather than trusted: see
// `ProcFs::read_filesystems`.
unsafe extern "C" {
    fn statfs(path: *const std::ffi::c_char, buf: *mut std::ffi::c_void) -> i32;
}

/// Big enough for `struct statfs` on any 64-bit Linux, which is 120 bytes.
///
/// Sixty-four bit only, and enforced rather than asserted in a comment: on a
/// 32-bit kernel these fields are four bytes each and every offset below reads
/// across two of them.
const STATFS_BUF: usize = 256;
const FS_BSIZE: usize = 8;
const FS_BLOCKS: usize = 16;
const FS_BAVAIL: usize = 32;

/// Total and available bytes for one mount point, or `None` if it cannot be
/// asked.
#[cfg(target_pointer_width = "64")]
fn statfs_at(mount: &str) -> Option<(u64, u64)> {
    let path = std::ffi::CString::new(mount).ok()?;
    let mut buf = [0u8; STATFS_BUF];
    let rc = unsafe { statfs(path.as_ptr(), buf.as_mut_ptr().cast()) };
    if rc != 0 {
        return None;
    }
    parse_statfs_buf(&buf)
}

/// Total and available bytes out of a filled `struct statfs`, or `None` if it
/// does not look like one.
///
/// Split from the call so the layout check can be shown to reject something.
fn parse_statfs_buf(buf: &[u8]) -> Option<(u64, u64)> {
    let at =
        |o: usize| -> Option<u64> { Some(u64::from_ne_bytes(buf.get(o..o + 8)?.try_into().ok()?)) };
    let (bsize, blocks, avail) = (at(FS_BSIZE)?, at(FS_BLOCKS)?, at(FS_BAVAIL)?);
    // A block size outside this range means the offsets are not pointing at a
    // block size, whatever else the numbers look like.
    (bsize.is_power_of_two() && (512..=1 << 20).contains(&bsize))
        .then(|| (blocks.saturating_mul(bsize), avail.saturating_mul(bsize)))
}

/// The device, mount point and type of each filesystem worth measuring.
///
/// Everything excluded here is excluded *before* the `statfs`, because for the
/// network and automount types the reason to exclude them is that the call
/// itself can block until a server answers.
///
/// Read-only filesystems go too, and that one is a measurement rather than a
/// list of names: a filesystem you cannot write to cannot fill up, so its
/// fullness is not a thing that can go wrong. Without it an Ubuntu machine
/// reports whichever of its twenty-odd squashfs snap mounts came first as
/// `100.0% full`, permanently, and the real root filesystem can never be shown
/// — every one of them has no available space by construction.
fn mount_points(text: &str) -> Vec<(String, String, bool, &str)> {
    text.lines()
        .filter_map(|l| {
            let mut f = l.split_whitespace();
            let (dev, mount, kind, opts) = (f.next()?, f.next()?, f.next()?, f.next()?);
            let writable = opts.split(',').all(|o| o != "ro");
            // Read-only filesystems are kept and marked, not dropped:
            // `merge_filesystems` needs them for their names. See there.
            (!super::is_network_fs(kind) && !super::is_ram_backed(kind))
                .then(|| (unescape(dev), unescape(mount), writable, kind))
        })
        .collect()
}

/// Undo the octal escaping the kernel applies to `/proc/mounts`.
///
/// Space is written `\040`, tab `\011`, backslash `\134`. Passed through
/// verbatim, a mount point containing a space becomes a path that does not
/// exist, `statfs` returns `ENOENT`, and the filesystem quietly disappears from
/// the capacity figures.
fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('\\') {
        out.push_str(&rest[..i]);
        let octal = rest.get(i + 1..i + 4).unwrap_or("");
        match u8::from_str_radix(octal, 8) {
            Ok(b) if octal.len() == 3 => {
                out.push(b as char);
                rest = &rest[i + 4..];
            }
            _ => {
                out.push('\\');
                rest = &rest[i + 1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Cumulative per-interface counters from one `/proc/net/dev` line.
#[derive(Clone, Copy, Default)]
struct LinkTimes {
    rx: u64,
    rx_packets: u64,
    rx_errs: u64,
    rx_drop: u64,
    tx: u64,
    tx_packets: u64,
    tx_errs: u64,
    tx_drop: u64,
}

/// Whole-stack cumulative counters.
///
/// Each is an `Option` in its own right, not a number guarded by one shared
/// flag: `/proc/net/snmp` can be unreadable on one sample and readable on the
/// next, and a baseline of zero would then report the machine's lifetime
/// retransmit count as one interval's worth.
#[derive(Clone, Copy, Default)]
struct NetTotals {
    retrans: Option<u64>,
    listen_drops: Option<u64>,
}

/// One `/proc/net/dev` line's counters, after the `iface:` label.
///
/// Receive is bytes, packets, errs, drop, fifo, frame, compressed, multicast;
/// transmit is bytes, packets, errs, drop, fifo, colls, carrier, compressed.
/// Sixteen fields, unchanged since the format was introduced.
fn parse_link(rest: &str) -> LinkTimes {
    let v: Vec<u64> = rest
        .split_whitespace()
        .map(|f| f.parse().unwrap_or(0))
        .collect();
    let at = |i: usize| v.get(i).copied().unwrap_or(0);
    LinkTimes {
        rx: at(0),
        rx_packets: at(1),
        rx_errs: at(2),
        rx_drop: at(3),
        tx: at(8),
        tx_packets: at(9),
        tx_errs: at(10),
        tx_drop: at(11),
    }
}

/// A named counter out of `/proc/net/snmp` or `/proc/net/netstat`.
///
/// Both files pair a header line of names with a values line, per protocol.
/// Looked up **by name rather than by position**, because the set of counters a
/// kernel publishes grows: `TcpExt` alone has gained fields across releases, and
/// a fixed index would silently read the wrong one on a kernel that added
/// something ahead of it.
fn snmp_counter(text: &str, prefix: &str, name: &str) -> Option<u64> {
    let mut lines = text.lines().filter(|l| l.starts_with(prefix));
    let col = lines.next()?.split_whitespace().position(|f| f == name)?;
    lines.next()?.split_whitespace().nth(col)?.parse().ok()
}

/// All three resources, or `None` if any file is missing.
///
/// Split from the read so the all-or-nothing rule can be tested. A kernel with
/// `/proc/pressure` has all three, so a partial read means something stranger
/// is going on than a missing config option — and half an answer here is worse
/// than none, because the half that is missing would render as a machine that
/// never stalled on that resource.
fn pressure_from(cpu: Option<&str>, io: Option<&str>, memory: Option<&str>) -> Option<Pressure> {
    Some(Pressure {
        cpu: parse_pressure(cpu?),
        io: parse_pressure(io?),
        memory: parse_pressure(memory?),
    })
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
    let raw = read_bytes(path, buf)?;
    std::str::from_utf8(raw).map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "not utf-8"))
}

/// The same read without the UTF-8 check, for a file that is not text.
///
/// `cmdline` is NUL-separated `argv`, and an argument that is not UTF-8 is
/// still a real argument — refusing the whole command line over one byte would
/// lose the identity of a process running perfectly well. Everything else in
/// `/proc` goes through [`read_into`], which does check.
fn read_bytes<'b>(path: &str, buf: &'b mut Vec<u8>) -> io::Result<&'b [u8]> {
    read_capped(path, buf, usize::MAX)
}

/// The same read, stopping after `cap` bytes.
///
/// For `cmdline`, which is the one `/proc` file with no useful bound on its
/// size: a `java` invocation carries its whole classpath, and a glob-expanded
/// command carries every filename it matched. Reading those whole would
/// allocate megabytes to keep two hundred characters, and — because `buf` is
/// the collector's shared buffer and it ratchets up and never shrinks — would
/// leave every later read in the sample carrying that allocation.
fn read_capped<'b>(path: &str, buf: &'b mut Vec<u8>, cap: usize) -> io::Result<&'b [u8]> {
    let mut f = File::open(path)?;
    if buf.len() < READ_BUF {
        buf.resize(READ_BUF, 0);
    }
    let mut n = 0;
    loop {
        if n >= cap {
            break;
        }
        if n == buf.len() {
            // Only for a file larger than anything /proc is expected to serve.
            buf.resize(buf.len() * 2, 0);
        }
        let room = buf.len().min(cap);
        match f.read(&mut buf[n..room]) {
            Ok(0) => break,
            Ok(k) => n += k,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
    Ok(&buf[..n])
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
    fn take_notes(&mut self) -> Vec<String> {
        std::mem::take(&mut self.notes)
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
        let net = self.read_net(elapsed);

        let mut io_denied = 0;
        // Before the `/proc` walk, not after. A process seen alive early in the
        // walk and drained from the socket a moment later would appear twice in
        // one sample — once live and once as an `X` row with the same pid — and
        // draining first makes that window as small as it can be rather than as
        // large.
        let exited = self.read_exited(needs, elapsed.as_secs_f64());
        let cgroups = self.read_cgroups(needs, elapsed.as_secs_f64());
        let (procs, tasks) = self.read_procs(elapsed, needs, &mut io_denied)?;
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
        // Taken and stored together with the read, before anything else
        // fallible — the same discipline as `prev_at` above, and for the same
        // reason it states: a sample that dies on a later `?` leaves the clock
        // advanced and the counter behind, so the next successful sample
        // divides two intervals of counters by one interval of wall clock and
        // reports the rate at twice what it was. The jiffy figures are immune
        // because they are jiffies over jiffies; these two divide by seconds.
        // `replace` only when the file actually carried the counter: a kernel
        // that publishes neither must not have its `None` recorded as a
        // baseline, or the next sample would diff against nothing.
        let was_ctxt = stat.ctxt.and_then(|n| self.prev_ctxt.replace(n));
        let was_intr = stat.intr.and_then(|n| self.prev_intr.replace(n));
        Ok(Sample {
            at: now,
            cpu_total: stat.busy,
            cpu_per_core: stat.per_core,
            iowait: Some(stat.iowait),
            steal: stat.steal,
            guest: stat.guest,
            irq: stat.irq,
            softirq: stat.softirq,
            // Cumulative counters as rates, like every other counter here. A
            // first sighting reports nothing rather than a boot's worth of
            // switches divided by one interval.
            ctxt: delta_rate(stat.ctxt, was_ctxt, elapsed.as_secs_f64()),
            intr: delta_rate(stat.intr, was_intr, elapsed.as_secs_f64()),
            running: stat.running,
            blocked: stat.blocked,
            mem: self.read_mem()?,
            load: self.read_load()?,
            procs,
            uptime: self.read_uptime()?,
            forks: stat.forks,
            io_supported: self.io_supported,
            clock_ceiling: self.read_clock_ceiling(needs),
            io_collected: needs.wants(Source::Io) && self.io_supported,
            io_denied,
            disks,
            pressure: self.read_pressure(),
            filesystems: self.read_filesystems(),
            net,
            tasks,
            exited,
            cgroups,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A real `/proc/<pid>/task/<tid>/stat`, with the trap intact: the thread
    /// name contains a space *and* a close parenthesis, so a split on the
    /// *first* `)` cuts the name short and every field after it reads as the
    /// wrong one. A name with only an opening bracket does not test this —
    /// `find` and `rfind` land on the same byte — which is what the first
    /// version of this fixture got wrong.
    const TASK_STAT: &str = "4098 (tokio-rt (w)) R 1 4021 4021 0 -1 4194304 \
        0 0 0 0 137 42 0 0 20 0 9 0 8842145 2846720000 41221 18446744073709551615 \
        1 1 0 0 0 0 0 0 0 0 0 0 17 3 0 0 0 0 0";

    #[test]
    fn a_threads_stat_is_read_past_a_name_with_a_bracket_in_it() {
        let mut seen = HashMap::new();
        let prev = HashMap::new();
        let t = parse_task_stat(4021, 4098, TASK_STAT, 1.0, &mut seen, &prev, 100.0)
            .expect("a thread stat did not parse");
        assert_eq!(t.pid, 4021);
        assert_eq!(t.tid, 4098);
        assert_eq!(
            &*t.name, "tokio-rt (w)",
            "the name was cut at the wrong bracket"
        );
        assert_eq!(t.state, 'R');
        // No previous reading, so no rate to report yet.
        assert_eq!(t.cpu, 0.0, "a thread's first sighting invented a rate");
        assert_eq!(
            seen.get(&4098),
            Some(&179),
            "the jiffies carried forward are not utime + stime"
        );
    }

    #[test]
    fn a_threads_cpu_is_the_delta_and_never_its_whole_lifetime() {
        // The bug this shape invites: a thread appearing for the first time
        // with a lifetime's worth of jiffies, divided by one interval, renders
        // at thousands of percent. The first sighting is zero and the second
        // is a real delta.
        let mut seen = HashMap::new();
        let mut prev = HashMap::new();
        prev.insert(4098, 129u64);
        let t = parse_task_stat(4021, 4098, TASK_STAT, 2.0, &mut seen, &prev, 100.0)
            .expect("did not parse");
        // 179 - 129 = 50 jiffies, 100 per second, over two seconds.
        assert_eq!(t.cpu, 25.0, "the rate is not the delta over the interval");
    }

    #[test]
    fn a_thread_that_went_backwards_reads_as_idle_rather_than_enormous() {
        // A tid reused between two samples. Saturating rather than wrapping:
        // the alternative is a subtraction underflowing into a rate with
        // eighteen digits.
        let mut seen = HashMap::new();
        let mut prev = HashMap::new();
        prev.insert(4098, 1_000_000u64);
        let t = parse_task_stat(4021, 4098, TASK_STAT, 1.0, &mut seen, &prev, 100.0)
            .expect("did not parse");
        assert_eq!(t.cpu, 0.0, "a counter that went backwards was not clamped");
    }

    /// Build a parse context from a collector, so the tests exercise the same
    /// state the collector would hand the parser.
    fn ctx(pf: &ProcFs) -> StatCtx<'_> {
        StatCtx {
            prev_jiffies: &pf.prev_proc_jiffies,
            prev_faults: &pf.prev_proc_faults,
            ticks_per_sec: pf.ticks_per_sec,
            page_size: pf.page_size,
        }
    }

    #[test]
    fn the_clock_ceiling_is_the_most_capped_policy() {
        // Thermal capping is rarely uniform — one hot package on a two-socket
        // box is still a machine that is not going as fast as it should — and a
        // mean would average the problem away.
        assert_eq!(
            ceiling_from(&[(3_600_000, 3_600_000), (2_232_000, 3_600_000)]),
            Some(62.0)
        );
        assert_eq!(
            ceiling_from(&[(3_600_000, 3_600_000)]),
            Some(100.0),
            "an uncapped machine did not read as uncapped"
        );
    }

    #[test]
    fn a_boost_ceiling_is_not_a_fault() {
        // Some drivers report a policy maximum above the nominal figure. That
        // is headroom, not damage: the answer is "not capped".
        assert_eq!(ceiling_from(&[(4_200_000, 3_600_000)]), Some(100.0));
    }

    #[test]
    fn a_policy_that_reports_nothing_is_skipped_rather_than_counted_as_zero() {
        // A nominal of zero would divide into an infinite ratio, and treating
        // it as a fully capped core would report a throttled machine because
        // one policy declined to answer.
        assert_eq!(ceiling_from(&[(0, 0), (3_600_000, 3_600_000)]), Some(100.0));
        assert_eq!(
            ceiling_from(&[]),
            None,
            "a machine with no frequency policy claimed a clock ceiling"
        );
        // The case the guard is actually for. Without it the division is
        // `0 / 0`, and `f32::min` treats the resulting NaN as the *other*
        // operand — so the clamp quietly turns "this file said nothing" into a
        // confident 100%, and a machine whose every policy declined to answer
        // reports that it is running at full speed.
        assert_eq!(
            ceiling_from(&[(0, 0)]),
            None,
            "policies that reported nothing were read as an uncapped machine"
        );
        assert_eq!(ceiling_from(&[(500, 0), (0, 0)]), None);
    }

    #[test]
    fn the_policy_set_is_rescanned_so_a_late_driver_is_not_missed() {
        // CPU hotplug is routine on cloud instances and a `cpufreq` driver can
        // load after the tool starts. Read strictly once, a machine that
        // published no policy at launch would never show `CLK` again for the
        // life of the process — which for a tool whose whole point is being
        // left running is the wrong way round.
        let mut pf = ProcFs::new().unwrap();
        // Poison it with a policy that does not exist. A rescan replaces the
        // vector wholesale, so its disappearance is the rescan happening.
        pf.nominal_khz = vec![("policy-that-is-not-there".to_string(), 3_600_000)];
        // Real ticks, from *one*. Two ways this goes vacuous: `Needs::default()`
        // is tick zero every time, which is due on every cadence; and starting
        // the loop at zero clears the poison on the first iteration, so the
        // assertion holds however long the cadence is — it would pass with a
        // cadence of a million. Starting at one means only a rescan that is
        // actually scheduled can satisfy it.
        for i in 1..=CLOCK_RESCAN {
            pf.collect(Needs::at(i).with(Source::ClockPolicies))
                .unwrap();
        }
        assert!(
            !pf.nominal_khz
                .iter()
                .any(|(n, _)| n == "policy-that-is-not-there"),
            "the policy set was never rescanned"
        );
    }

    #[test]
    fn a_machine_with_no_cpufreq_costs_nothing_and_says_nothing() {
        // Every virtualised CPU, which is every machine available to test this
        // on. `None` rather than 100%, which would claim the machine is running
        // at full speed on the strength of not being able to look.
        let mut pf = ProcFs::new().unwrap();
        if pf.nominal_khz.is_empty() {
            assert_eq!(
                pf.read_clock_ceiling(Needs::at(0).with(Source::ClockPolicies)),
                None
            );
        }
        // One direction only. A ceiling implies a policy was found, but a
        // policy does not imply a ceiling: `cpuinfo_max_freq` and
        // `scaling_max_freq` are separate reads, and a hardened host can permit
        // the first and refuse the second. Asserted the other way round, this
        // fails on a machine where the code is behaving exactly as designed.
        let s = pf.collect(Needs::default()).unwrap();
        if s.clock_ceiling.is_some() {
            assert!(
                !pf.nominal_khz.is_empty(),
                "a ceiling was reported with no policy behind it"
            );
        }
    }

    #[test]
    fn a_cmdline_becomes_one_line() {
        // The file is argv with a NUL after every element, including the last,
        // so a naive split leaves a trailing empty string and the line ends in
        // a space.
        let raw = b"/usr/bin/node\0/srv/api/server.js\0--port\x003000\0";
        assert_eq!(
            parse_cmdline(raw).unwrap(),
            "node /srv/api/server.js --port 3000"
        );
    }

    #[test]
    fn an_enormous_cmdline_does_not_inflate_the_shared_buffer() {
        // A java invocation carries its whole classpath and a glob-expanded
        // command carries every filename it matched. Read whole, that allocates
        // megabytes to keep two hundred characters — and `buf` is the
        // collector's shared buffer, which ratchets up and never shrinks, so
        // every later read in the sample would carry the allocation too.
        let dir = std::env::temp_dir().join(format!("poptop-cap-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("huge");
        let mut raw = Vec::new();
        raw.extend_from_slice(b"/usr/bin/java\0");
        for i in 0..40_000 {
            raw.extend_from_slice(format!("/opt/lib/jar-{i}.jar").as_bytes());
            raw.push(0);
        }
        assert!(raw.len() > 700_000, "the fixture is not large enough");
        std::fs::write(&file, &raw).unwrap();

        let mut buf = vec![0u8; READ_BUF];
        let (read, named) = {
            let got = read_capped(file.to_str().unwrap(), &mut buf, CMD_READ_MAX).unwrap();
            (got.len(), parse_cmdline(got))
        };
        assert!(read <= CMD_READ_MAX, "read {read} bytes");
        assert_eq!(
            buf.len(),
            READ_BUF,
            "the shared buffer was inflated to {}",
            buf.len()
        );
        // And the process is still named.
        let named = named.expect("no command line");
        assert!(named.starts_with("java "), "{named:?}");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_kernel_thread_has_no_command_line() {
        // Its cmdline is empty and its bracketed comm is the only identity it
        // has. `None`, not an empty string.
        assert_eq!(parse_cmdline(b""), None);
        assert_eq!(parse_cmdline(b"\0"), None);
    }

    #[test]
    fn a_live_process_with_a_bad_byte_in_argv_is_still_named() {
        // Through `cmdline`, not `parse_cmdline`: the parse is lossy either
        // way, and what this pins is that the *read* does not go through the
        // strict UTF-8 check. With `read_into` in there the file comes back as
        // `InvalidData` and the process shows as a blank.
        //
        // `arg0`, not a shell. The first version ran `sh -c 'sleep 5' <bad>`,
        // and a shell whose script is a single command execs it directly —
        // replacing the argv this test had just planted. Whether the read
        // landed before or after that exec was a race: it passed locally every
        // time and failed in CI. `sleep` execs nothing, so its argv is the one
        // it was given, and `spawn` returning `Ok` already means the exec
        // happened.
        use std::os::unix::ffi::OsStrExt as _;
        use std::os::unix::process::CommandExt as _;
        let bad = std::ffi::OsStr::from_bytes(b"\xff\xfe-bad");
        let mut child = std::process::Command::new("sleep")
            .arg0(bad)
            .arg("30")
            .spawn()
            .expect("could not spawn");

        // Polled rather than read once. Between the fork and the exec, the
        // child's `cmdline` still shows the *parent's* argv — the copied image
        // has not been replaced yet — so a single read a moment after `spawn`
        // returns can come back as the test harness's own command line. It did:
        // one full-suite run in six, reporting `poptop-de69… --quiet`.
        //
        // The loop does not weaken what this pins. With the strict UTF-8 read
        // in place every attempt returns `None`, so it times out and the
        // expectation below still fails.
        let mut cache = HashMap::new();
        let (mut path, mut buf) = (String::new(), Vec::new());
        let pid = child.id() as i32;
        let mut got = None;
        for _ in 0..200 {
            cache.clear();
            got = cmdline(pid, 1, 0, &mut cache, &mut path, &mut buf);
            if got.as_deref().is_some_and(|c| c.ends_with(" 30")) {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        let _ = child.kill();
        let _ = child.wait();

        let got = got.expect("a command line with one bad byte was refused entirely");
        assert!(
            got.ends_with(" 30"),
            "the readable arguments were lost: {got:?}"
        );
        assert!(
            got.contains('\u{fffd}'),
            "the bad byte was not the one under test: {got:?}"
        );
    }

    #[test]
    fn an_argument_that_is_not_utf8_does_not_lose_the_command_line() {
        // A process running perfectly well with one bad byte in its arguments
        // still has an identity, and `read_into`'s strict check would have
        // thrown the whole line away.
        let raw = b"/bin/grep\0\xff\xfe\0/etc/passwd\0";
        let got = parse_cmdline(raw).expect("the command line was refused");
        assert!(got.starts_with("grep "), "{got:?}");
        assert!(got.ends_with("/etc/passwd"), "{got:?}");
    }

    #[test]
    fn a_command_line_is_read_once_and_then_cached() {
        // The cache is mostly about allocation — see the field's comment —
        // but it only saves anything if it is actually consulted.
        let mut cache = HashMap::new();
        let (mut path, mut buf) = (String::new(), Vec::new());
        let me = std::process::id() as i32;

        let first = cmdline(me, 100, 1, &mut cache, &mut path, &mut buf);
        assert!(first.is_some(), "this process has no command line");

        // A tick that is not this pid's refresh slot must not re-read. Proved
        // by poisoning the cache: if the file is read again the poison is gone.
        cache.insert(me, (100, Some(Arc::from("poison"))));
        let quiet = (0..CMD_REFRESH)
            .find(|t| t % CMD_REFRESH != me.unsigned_abs() as u64 % CMD_REFRESH)
            .unwrap();
        let again = cmdline(me, 100, quiet, &mut cache, &mut path, &mut buf);
        assert_eq!(again.as_deref(), Some("poison"), "the file was read again");
    }

    #[test]
    fn a_rewritten_title_is_picked_up_within_the_refresh_window() {
        // postgres and nginx rewrite argv in place, and those are exactly the
        // processes this column is most useful for. A cache that never expired
        // would freeze the one title worth watching.
        let mut cache = HashMap::new();
        let (mut path, mut buf) = (String::new(), Vec::new());
        let me = std::process::id() as i32;
        cache.insert(me, (100, Some(Arc::from("stale"))));

        let due = me.unsigned_abs() as u64 % CMD_REFRESH;
        let got = cmdline(me, 100, due, &mut cache, &mut path, &mut buf);
        assert_ne!(
            got.as_deref(),
            Some("stale"),
            "the command line was never re-read"
        );
    }

    #[test]
    fn a_recycled_pid_does_not_inherit_the_dead_process_command_line() {
        // Same guard as the name cache, one field over.
        let mut cache = HashMap::new();
        let (mut path, mut buf) = (String::new(), Vec::new());
        let me = std::process::id() as i32;
        cache.insert(me, (100, Some(Arc::from("the dead one"))));
        // A different start time is a different process wearing the same pid.
        let quiet = (0..CMD_REFRESH)
            .find(|t| t % CMD_REFRESH != me.unsigned_abs() as u64 % CMD_REFRESH)
            .unwrap();
        let got = cmdline(me, 200, quiet, &mut cache, &mut path, &mut buf);
        assert_ne!(got.as_deref(), Some("the dead one"));
    }

    #[test]
    fn the_command_line_cache_does_not_grow_without_bound() {
        // Every other per-pid map in this collector is pruned against `seen`,
        // and one that is not is a leak on a box with process churn — which is
        // the kind poptop gets pointed at.
        let mut pf = ProcFs::new().unwrap();
        pf.collect(Needs::default()).unwrap();
        let before = pf.cmds.len();
        pf.cmds.insert(-12345, (0, Some(Arc::from("a ghost"))));
        pf.collect(Needs::default()).unwrap();
        assert!(
            !pf.cmds.contains_key(&-12345),
            "an exited process was kept: {} entries, was {before}",
            pf.cmds.len()
        );
    }

    /// A real `/proc/<pid>/stat`, field for field. The indices this parser uses
    /// are `rest[N - 3]`, so a fixture is the only way to know they line up
    /// with the fields the kernel actually writes.
    ///
    /// minflt 4210, majflt 17, priority 20, nice -5, num_threads 8,
    /// starttime 1234, vsize 2846720000, rss 41221.
    const PROC_STAT: &str = "4021 (postgres) S 1 4021 4021 0 -1 4194304 \
        4210 0 17 0 137 42 0 0 20 -5 8 0 1234 2846720000 41221 \
        18446744073709551615 1 1 0 0 0 0 0 0 0 0 0 0 17 3 0 0 0 0 0";

    #[test]
    fn the_new_stat_fields_land_on_the_fields_the_kernel_wrote() {
        let pf = ProcFs::new().unwrap();
        let mut seen = HashMap::new();
        let mut seen_faults = HashMap::new();
        let mut names = HashMap::new();
        let p = parse_proc_stat(
            4021,
            PROC_STAT,
            1.0,
            Arc::from("postgres"),
            &mut seen,
            &mut seen_faults,
            &mut names,
            &ctx(&pf),
        )
        .expect("the fixture did not parse");
        assert_eq!(p.nice, Some(-5), "nice landed on the wrong field");
        assert_eq!(
            p.vsize,
            Some(2_846_720_000),
            "vsize landed on the wrong field"
        );
        assert_eq!(p.threads, Some(8), "the fixture disagrees with the parser");
        // Cumulative counters carried forward, to become next sample's baseline.
        assert_eq!(seen_faults.get(&4021), Some(&(4210, 17)));
    }

    #[test]
    fn the_fault_baseline_survives_to_the_next_sample() {
        // Without the write-back, `prev_faults` is the empty map on every
        // sample, `rate` always takes its first-sighting arm, and the column
        // this whole item is about reads zero forever — for a process thrashing
        // on major faults as much as for an idle one.
        //
        // Asserted on the collector rather than on the parser: the parser's own
        // test checks the map it is handed, which is exactly the thing that was
        // being thrown away.
        let mut pf = ProcFs::new().unwrap();
        pf.collect(Needs::default()).unwrap();
        assert!(
            !pf.prev_proc_faults.is_empty(),
            "a sample left no fault baseline for the next one"
        );
        let me = std::process::id() as i32;
        assert!(
            pf.prev_proc_faults.contains_key(&me),
            "the baseline does not include this process"
        );
    }

    #[test]
    fn a_faults_first_sighting_is_zero_and_its_second_is_a_rate() {
        // The counters are cumulative since the process started, so a delta
        // against nothing reports a whole lifetime of paging as this second's —
        // the same trap as its CPU, and the same answer.
        assert_eq!(rate(4210, None, 1.0), 0, "a first sighting invented a rate");
        assert_eq!(rate(4210, Some(4200), 1.0), 10);
        // Over two seconds it is half as many a second.
        assert_eq!(rate(4210, Some(4200), 2.0), 5);
        // A counter that went backwards is a recycled pid, not a negative rate.
        assert_eq!(rate(10, Some(4200), 1.0), 0);
    }

    #[test]
    fn proportional_memory_is_read_in_kilobytes_and_reported_in_bytes() {
        // `smaps_rollup` publishes kB like the rest of that family, and a
        // figure a thousand times too small in a byte column reads as a
        // suspiciously light process rather than as a bug.
        let mut path = String::new();
        let mut buf = Vec::new();
        let me = std::process::id() as i32;
        if let Some(pss) = read_pss(me, &mut path, &mut buf, &mut 0) {
            assert!(
                pss > 64 * 1024,
                "this process reads {pss} bytes of proportional memory, which \
                 is kilobytes rendered as bytes"
            );
        }
    }

    #[test]
    fn the_container_cache_does_not_grow_without_bound_either() {
        // The one most likely to be forgotten, and it was: every other per-pid
        // map here expires on a refresh slot, so something removes stale
        // entries anyway. This cache is deliberately valid for a process's
        // whole life, which means nothing removes an entry unless the prune
        // does — one permanent entry per pid ever seen, on the kind of box
        // that churns thousands an hour.
        let mut pf = ProcFs::new().unwrap();
        pf.collect(Needs::default()).unwrap();
        pf.containers
            .insert(-12345, (0, Some(Arc::from("a ghost"))));
        pf.collect(Needs::default()).unwrap();
        assert!(
            !pf.containers.contains_key(&-12345),
            "an exited process kept its container: {} entries",
            pf.containers.len()
        );
    }

    #[test]
    fn meminfo_partitions_what_the_machine_is_holding() {
        // Huge pages are counted in *pages* of `Hugepagesize`, not kilobytes
        // like everything else in this file — reading them with the same helper
        // reports a two-megabyte page as two kilobytes.
        let m = parse_meminfo(
            "MemTotal:       16384000 kB\n\
             MemFree:         1024000 kB\n\
             MemAvailable:    8192000 kB\n\
             Dirty:            512000 kB\n\
             Shmem:            256000 kB\n\
             Slab:            1024000 kB\n\
             SReclaimable:     768000 kB\n\
             PageTables:        64000 kB\n\
             HugePages_Total:      64\n\
             HugePages_Free:       16\n\
             Hugepagesize:       2048 kB\n\
             SwapTotal:       2048000 kB\n\
             SwapFree:        2048000 kB\n",
        );
        assert_eq!(m.dirty, Some(512_000 * 1024));
        assert_eq!(m.slab, Some(1_024_000 * 1024));
        assert_eq!(m.slab_reclaimable, Some(768_000 * 1024));
        assert_eq!(m.shmem, Some(256_000 * 1024));
        assert_eq!(m.page_tables, Some(64_000 * 1024));
        // 64 pages of 2 MiB.
        assert_eq!(m.huge_total, Some(64 * 2048 * 1024));
        assert_eq!(m.huge_used, Some(48 * 2048 * 1024), "total minus free");
    }

    #[test]
    fn a_meminfo_without_a_line_says_nothing_rather_than_zero() {
        // A kernel built without hugetlb has no huge pages to report; one with
        // none reserved has zero of them. Different answers.
        let m = parse_meminfo("MemTotal: 100 kB\nMemFree: 50 kB\nMemAvailable: 60 kB\n");
        assert_eq!(m.huge_total, None, "an absent line was reported as zero");
        // …and a meminfo that names the pages without naming their size cannot
        // be turned into bytes, so it says nothing rather than `0 B`.
        let partial = parse_meminfo("MemTotal: 100 kB\nHugePages_Total: 64\n");
        assert_eq!(
            partial.huge_total, None,
            "64 reserved pages of unknown size were reported as no memory"
        );
        assert_eq!(m.dirty, None);
        assert_eq!(m.slab, None);
        // The figures that were there are still read.
        assert_eq!(m.total, 100 * 1024);
    }

    #[test]
    fn the_cpu_line_is_read_past_busy_and_idle() {
        // user nice system idle iowait irq softirq steal guest guest_nice
        let a = CpuTimes::parse(" 100 10 50 800 20 5 5 10 7 3").unwrap();
        let b = CpuTimes::parse(" 200 10 50 800 20 9 25 60 7 3").unwrap();
        // total counts the first eight, so it advances by
        // 100 + 4 + 20 + 50 = 174.
        let pct = |f: fn(&CpuTimes) -> u64| b.share_since(&a, f);
        assert!(
            (pct(|t| t.steal) - 28.7).abs() < 0.1,
            "steal: {}",
            pct(|t| t.steal)
        );
        assert!((pct(|t| t.softirq) - 11.5).abs() < 0.1, "softirq");
        assert!((pct(|t| t.irq) - 2.3).abs() < 0.1, "irq");
        // guest is inside user and is *not* in the total; it did not move here.
        assert_eq!(pct(|t| t.guest), 0.0);
    }

    #[test]
    fn a_counter_with_no_previous_reading_is_absent_rather_than_a_boots_worth() {
        // A boot's worth of context switches divided by one interval is a
        // number in the millions presented as this second's.
        assert_eq!(delta_rate(Some(1_000), None, 1.0), None);
        assert_eq!(delta_rate(None, Some(1_000), 1.0), None);
        assert_eq!(delta_rate(Some(1_500), Some(1_000), 1.0), Some(500));
        assert_eq!(delta_rate(Some(1_500), Some(1_000), 2.0), Some(250));
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
            steal: 0,
            guest: 0,
            irq: 0,
            softirq: 0,
            extended: true,
        };
        let b = CpuTimes {
            idle: 950,
            total: 1100,
            iowait: 0,
            steal: 0,
            guest: 0,
            irq: 0,
            softirq: 0,
            extended: true,
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
            steal: 0,
            guest: 0,
            irq: 0,
            softirq: 0,
            extended: true,
        };
        let b = CpuTimes {
            idle: 980,
            total: 1100,
            iowait: 130,
            steal: 0,
            guest: 0,
            irq: 0,
            softirq: 0,
            extended: true,
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
            steal: 0,
            guest: 0,
            irq: 0,
            softirq: 0,
            extended: true,
        };
        let b = CpuTimes {
            idle: 100,
            total: 100,
            iowait: 100,
            steal: 0,
            guest: 0,
            irq: 0,
            softirq: 0,
            extended: true,
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
            steal: 0,
            guest: 0,
            irq: 0,
            softirq: 0,
            extended: true,
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
            &mut HashMap::new(),
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
            &mut HashMap::new(),
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
            &mut HashMap::new(),
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
            &mut HashMap::new(),
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
        pf.read_procs(
            Duration::from_secs(1),
            Needs::NONE.with(Source::Io),
            &mut denied,
        )
        .unwrap();
        assert!(
            !pf.prev_proc_io.is_empty(),
            "collecting IO recorded no counters"
        );

        pf.read_procs(Duration::from_secs(1), Needs::NONE, &mut denied)
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
    fn a_read_only_mount_is_kept_and_marked_rather_than_dropped() {
        // Dropped here it could not lend its name to a writable sibling, which
        // is how macOS reports its sealed root. Marked, `merge_filesystems`
        // decides — see `collect::fs_tests`.
        let mounts = "/dev/loop3 /snap/core22/1234 squashfs ro,nodev 0 0\n";
        let got = mount_points(mounts);
        assert_eq!(got.len(), 1, "a read-only mount was dropped outright");
        assert!(!got[0].2, "a read-only mount was marked writable");
    }

    #[test]
    fn a_mount_point_with_a_space_in_it_survives() {
        // The kernel writes `\\040` for space, `\\011` for tab and `\\134` for
        // backslash. Passed through verbatim the path does not exist, `statfs`
        // returns ENOENT, and the filesystem quietly vanishes from the figures.
        assert_eq!(unescape("/mnt/my\\040disk"), "/mnt/my disk");
        assert_eq!(unescape("/mnt/a\\011b"), "/mnt/a\tb");
        assert_eq!(unescape("/mnt/back\\134slash"), "/mnt/back\\slash");
        assert_eq!(unescape("/plain/path"), "/plain/path");
        // Not an escape: left alone rather than eaten.
        assert_eq!(unescape("/mnt/\\9zz"), "/mnt/\\9zz");
        assert_eq!(unescape("/mnt/trailing\\"), "/mnt/trailing\\");

        let mounts = "/dev/vda1 /mnt/my\\040disk ext4 rw 0 0\n";
        assert_eq!(mount_points(mounts)[0].1, "/mnt/my disk");
    }

    #[test]
    fn a_statfs_that_is_not_one_is_refused() {
        // The check that licenses reading three numbers at fixed offsets from a
        // structure this file never declares.
        let mut buf = [0u8; STATFS_BUF];
        buf[FS_BSIZE..FS_BSIZE + 8].copy_from_slice(&4096u64.to_ne_bytes());
        buf[FS_BLOCKS..FS_BLOCKS + 8].copy_from_slice(&1000u64.to_ne_bytes());
        buf[FS_BAVAIL..FS_BAVAIL + 8].copy_from_slice(&400u64.to_ne_bytes());
        assert_eq!(parse_statfs_buf(&buf), Some((4096 * 1000, 4096 * 400)));

        // A block size that is not one. Anything at that offset would be *a*
        // number; only a plausible block size means the offsets are right.
        for bad in [0u64, 3, 100, 1 << 30] {
            buf[FS_BSIZE..FS_BSIZE + 8].copy_from_slice(&bad.to_ne_bytes());
            assert_eq!(
                parse_statfs_buf(&buf),
                None,
                "a block size of {bad} was accepted"
            );
        }
        assert_eq!(parse_statfs_buf(&[]), None);
    }

    #[test]
    fn mount_points_drop_what_cannot_or_should_not_be_measured() {
        let mounts = "\
/dev/vda1 / ext4 rw,relatime 0 0\n\
proc /proc proc rw,nosuid 0 0\n\
tmpfs /dev/shm tmpfs rw 0 0\n\
server:/export /mnt/nfs nfs4 rw 0 0\n\
//host/share /mnt/smb cifs rw 0 0\n\
/dev/loop3 /snap/core22/1234 squashfs ro,nodev 0 0\n\
auto /net autofs rw,fd=7 0 0\n\
/dev/vda2 /home ext4 rw 0 0\n";
        let got: Vec<String> = mount_points(mounts)
            .into_iter()
            .filter(|(_, _, writable, _)| *writable)
            .map(|(_, m, _, _)| m)
            .collect();
        // `proc` survives this filter and falls out later on its own, because
        // it reports no blocks — a measurement rather than a rule about names.
        //
        // The squashfs snap does not, and that one matters: it is read-only, so
        // it has no available space by construction and reads as 100% full
        // forever. An Ubuntu machine carries twenty-odd of them, and the real
        // root filesystem could never be reported past any of them.
        assert_eq!(got, vec!["/", "/proc", "/home"]);
    }

    #[test]
    fn a_hung_fileserver_cannot_be_reached_from_here() {
        // The reason network filesystems are named rather than measured:
        // `statfs` on an unresponsive mount blocks until it answers, and a
        // monitor that freezes when the fileserver does is worse than one that
        // does not mention the fileserver.
        for kind in ["nfs", "nfs4", "cifs", "smbfs", "ceph", "fuse.sshfs"] {
            let line = format!("srv:/x /mnt {kind} rw 0 0\n");
            assert!(
                mount_points(&line).is_empty(),
                "{kind} would have been given to statfs"
            );
        }
    }

    #[test]
    fn the_live_filesystems_agree_with_the_mount_table() {
        let pf = ProcFs::new().unwrap();
        let fs = pf.read_filesystems().expect("no filesystems at all");
        let mounts = fs::read_to_string("/proc/mounts").unwrap();
        for f in &fs {
            assert!(f.total > 0, "{} reported no size", f.mount);
            assert!(f.avail <= f.total, "{} has more free than it has", f.mount);
            assert!(
                mounts
                    .lines()
                    .any(|l| l.split_whitespace().nth(1) == Some(&f.mount)),
                "{} is not in the mount table",
                f.mount
            );
        }
        // Bind mounts collapse. This container has `/dev/vda1` at three paths
        // and every pseudo-filesystem reports nothing, so the list is far
        // shorter than the table.
        assert!(
            fs.len() < mounts.lines().count(),
            "every mount was reported: {} of {}",
            fs.len(),
            mounts.lines().count()
        );
        // Distinct mount points. Not distinct *sizes* — two devices reporting
        // identical numbers are two filesystems, and this container has exactly
        // that in the `overlay` at `/` and the `ext4` behind it. Merging on
        // size would have deleted one of them; that rule is tested directly in
        // `collect::fs_tests`.
        let mut seen: Vec<&str> = fs.iter().map(|f| &*f.mount).collect();
        seen.sort_unstable();
        let before = seen.len();
        seen.dedup();
        assert_eq!(before, seen.len(), "a mount point was listed twice");

        // And nothing read-only, which has no available space by construction
        // and would read as full forever.
        let mounts_text = fs::read_to_string("/proc/mounts").unwrap();
        for f in &fs {
            let opts = mounts_text
                .lines()
                .find(|l| l.split_whitespace().nth(1) == Some(&f.mount))
                .and_then(|l| l.split_whitespace().nth(3))
                .unwrap_or("");
            assert!(
                opts.split(',').all(|o| o != "ro"),
                "{} is read-only and cannot fill up",
                f.mount
            );
        }
    }

    #[test]
    fn a_net_dev_line_parses_both_directions() {
        // Receive is bytes, packets, errs, drop, fifo, frame, compressed,
        // multicast; transmit is the same shape starting at field eight.
        let l = parse_link("  1000 10 1 2 0 0 0 0   2000 20 3 4 0 0 0 0");
        assert_eq!((l.rx, l.rx_packets, l.rx_errs, l.rx_drop), (1000, 10, 1, 2));
        assert_eq!((l.tx, l.tx_packets, l.tx_errs, l.tx_drop), (2000, 20, 3, 4));
    }

    #[test]
    fn snmp_counters_are_found_by_name_not_by_position() {
        // The set of counters a kernel publishes grows: `TcpExt` has gained
        // fields across releases, and a fixed index would silently read the
        // wrong one on a kernel that added something ahead of it.
        let before = "Tcp: RtoAlgorithm RetransSegs InErrs\nTcp: 1 42 7\n";
        let after = "Tcp: RtoAlgorithm NewCounter RetransSegs InErrs\nTcp: 1 99 42 7\n";
        assert_eq!(snmp_counter(before, "Tcp:", "RetransSegs"), Some(42));
        assert_eq!(
            snmp_counter(after, "Tcp:", "RetransSegs"),
            Some(42),
            "a counter inserted ahead of it shifted the reading"
        );
        assert_eq!(snmp_counter(before, "Tcp:", "NotThere"), None);
        assert_eq!(snmp_counter("", "Tcp:", "RetransSegs"), None);
    }

    /// A `/proc/net/dev` file: a busy interface, an idle tunnel, and loopback.
    fn net_dev_fixture(rx: u64, tx: u64, errs: u64, drop: u64) -> String {
        format!(
            "Inter-|   Receive  |  Transmit\n face |bytes packets errs drop\n\
             {:>7}: {rx} 10 {errs} {drop} 0 0 0 0 {tx} 20 0 0 0 0 0 0\n\
             {:>7}: 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0\n\
             {:>7}: 500 5 0 0 0 0 0 0 500 5 0 0 0 0 0 0\n",
            "eth0", "utun0", "lo"
        )
    }

    #[test]
    fn a_counter_that_becomes_readable_reports_nothing_not_everything() {
        // `/proc/net/snmp` can be unreadable on one sample and readable on the
        // next — a transient failure, or a container namespace settling. With a
        // shared baseline of zero the next sample reported the machine's
        // lifetime retransmit count as one interval's worth, in critical red.
        let mut pf = ProcFs::new().unwrap();
        let dev = net_dev_fixture(1000, 2000, 0, 0);
        pf.net_from(&dev, "", "", Duration::from_secs(1));
        let after = pf.net_from(
            &dev,
            "Tcp: RetransSegs\nTcp: 5000000\n",
            "",
            Duration::from_secs(1),
        );
        assert_eq!(
            after.retrans, None,
            "a lifetime counter was charged to one second"
        );
        // And the sample after that, with a baseline, reports the real delta.
        let then = pf.net_from(
            &dev,
            "Tcp: RetransSegs\nTcp: 5000004\n",
            "",
            Duration::from_secs(1),
        );
        assert_eq!(then.retrans, Some(4));
    }

    #[test]
    fn a_counter_the_kernel_does_not_publish_is_absent_not_zero() {
        // The file exists and does not carry the counter. Reporting zero would
        // say "this kernel is retransmitting nothing", which is the confident
        // lie this module refuses everywhere else.
        let mut pf = ProcFs::new().unwrap();
        let dev = net_dev_fixture(1000, 2000, 0, 0);
        let snmp = "Tcp: RtoAlgorithm InErrs\nTcp: 1 0\n";
        pf.net_from(&dev, snmp, "", Duration::from_secs(1));
        let net = pf.net_from(&dev, snmp, "", Duration::from_secs(1));
        assert_eq!(net.retrans, None, "a missing counter became a zero");
    }

    #[test]
    fn an_interface_appearing_mid_session_does_not_dump_its_history() {
        // A NIC plugged in, or a namespace settling, brings its lifetime
        // counters with it. Diffing whole-machine sums would charge all of
        // them to this second; diffing per interface charges none of them,
        // because there is nothing yet to subtract from.
        let mut pf = ProcFs::new().unwrap();
        let one = "eth0: 1000 10 5 5 0 0 0 0 2000 20 0 0 0 0 0 0\n";
        let two = "eth0: 1000 10 5 5 0 0 0 0 2000 20 0 0 0 0 0 0\n\
                   eth1: 500 5 900 900 0 0 0 0 500 5 0 0 0 0 0 0\n";
        pf.net_from(one, "", "", Duration::from_secs(1));
        let net = pf.net_from(two, "", "", Duration::from_secs(1));
        assert_eq!(
            net.errors,
            Some(0),
            "a new interface's lifetime errors were charged to this interval"
        );
    }

    #[test]
    fn an_interface_going_away_does_not_swallow_a_real_interval() {
        // The inverse: a VPN tunnel disappearing shrinks a whole-machine sum,
        // and `saturating_sub` would report a real interval's errors as zero.
        let mut pf = ProcFs::new().unwrap();
        let two = "eth0: 1000 10 0 0 0 0 0 0 2000 20 0 0 0 0 0 0\n\
                   utun0: 500 5 100 0 0 0 0 0 500 5 0 0 0 0 0 0\n";
        let gone = "eth0: 1000 10 7 0 0 0 0 0 2000 20 0 0 0 0 0 0\n";
        pf.net_from(two, "", "", Duration::from_secs(1));
        let net = pf.net_from(gone, "", "", Duration::from_secs(1));
        assert_eq!(net.errors, Some(7), "a real interval's errors were lost");
    }

    #[test]
    fn an_absent_snmp_file_leaves_the_counter_absent_rather_than_zero() {
        // A kernel that does not publish retransmits and one reporting none are
        // opposite answers, and only `/proc/net/dev` is required for the rest.
        let mut pf = ProcFs::new().unwrap();
        let dev = net_dev_fixture(1000, 2000, 0, 0);
        let snmp = "Tcp: RetransSegs\nTcp: 5\n";
        let netstat = "TcpExt: ListenDrops\nTcpExt: 2\n";

        pf.net_from(&dev, snmp, netstat, Duration::from_secs(1));
        let with = pf.net_from(&dev, snmp, netstat, Duration::from_secs(1));
        assert_eq!(with.retrans, Some(0), "no retransmits since the last read");
        assert_eq!(with.listen_drops, Some(0));

        let mut pf = ProcFs::new().unwrap();
        pf.net_from(&dev, "", "", Duration::from_secs(1));
        let without = pf.net_from(&dev, "", "", Duration::from_secs(1));
        assert_eq!(without.retrans, None, "an absent file became a zero");
        assert_eq!(without.listen_drops, None);
        // The interface counters still work: only the missing halves are absent.
        assert_eq!(without.errors, Some(0));
    }

    #[test]
    fn an_idle_interface_is_not_listed() {
        // A laptop publishes twenty-odd interfaces and almost all are idle
        // tunnels. Excluded by measurement, not by a rule about names.
        let mut pf = ProcFs::new().unwrap();
        let dev = net_dev_fixture(1000, 2000, 0, 0);
        pf.net_from(&dev, "", "", Duration::from_secs(1));
        let net = pf.net_from(
            &net_dev_fixture(3000, 6000, 0, 0),
            "",
            "",
            Duration::from_secs(1),
        );
        let names: Vec<&str> = net.links.iter().map(|l| &*l.name).collect();
        assert_eq!(names, vec!["eth0", "lo"], "an idle tunnel was listed");
        // 2000 bytes over one second, in each direction's own delta.
        let eth = net.links.iter().find(|l| &*l.name == "eth0").unwrap();
        assert_eq!((eth.rx, eth.tx), (2000, 4000));
    }

    #[test]
    fn errors_and_drops_are_summed_across_interfaces() {
        let mut pf = ProcFs::new().unwrap();
        pf.net_from(&net_dev_fixture(0, 0, 0, 0), "", "", Duration::from_secs(1));
        let net = pf.net_from(&net_dev_fixture(0, 0, 7, 3), "", "", Duration::from_secs(1));
        assert_eq!(net.errors, Some(7));
        assert_eq!(net.drops, Some(3));
    }

    #[test]
    fn the_live_network_reads_interfaces_that_carry_something() {
        let mut pf = ProcFs::new().unwrap();
        // First read primes the counters; rates need two.
        let first = pf
            .read_net(Duration::from_secs(1))
            .expect("no /proc/net/dev");
        assert!(first.links.is_empty(), "rates from a single read");
        assert_eq!(first.errors, None, "a delta was reported from one read");

        std::thread::sleep(Duration::from_millis(50));
        let net = pf.read_net(Duration::from_millis(50)).unwrap();

        let raw = fs::read_to_string("/proc/net/dev").unwrap();
        for l in &net.links {
            let line = raw
                .lines()
                .find(|x| x.split_once(':').is_some_and(|(n, _)| n.trim() == &*l.name))
                .unwrap_or_else(|| panic!("interface not in the file: {}", l.name));
            // Every listed interface has carried something. A laptop publishes
            // twenty-odd and almost all are idle tunnels.
            let c = parse_link(line.split_once(':').unwrap().1);
            assert!(
                c.rx > 0 || c.tx > 0,
                "{} has never carried a byte and was listed",
                l.name
            );
        }
        assert!(
            net.links.len() < raw.lines().count(),
            "every line of /proc/net/dev was reported"
        );
        assert!(net.errors.is_some(), "errors went unreported on Linux");
    }

    #[test]
    fn pressure_takes_the_ten_second_average_of_both_lines() {
        let s = parse_pressure(
            "some avg10=3.60 avg60=2.59 avg300=6.23 total=736109049\n\
             full avg10=1.25 avg60=2.57 avg300=6.14 total=722044449\n",
        );
        assert_eq!(s.some, 3.60);
        assert_eq!(s.full, 1.25);
    }

    #[test]
    fn a_pressure_file_with_no_full_line_reports_none_of_it() {
        // Which is what older kernels mean by omitting it: `full` is published
        // for IO and memory and not for CPU, where it is undefined.
        let s = parse_pressure("some avg10=0.40 avg60=0.27 avg300=0.47 total=106158967\n");
        assert_eq!(s.some, 0.40);
        assert_eq!(s.full, 0.0);
    }

    #[test]
    fn pressure_is_all_three_resources_or_none() {
        // A kernel with `/proc/pressure` has all three. Half an answer would
        // render the missing half as a machine that never stalled on it.
        let some = "some avg10=1.0\nfull avg10=0.5\n";
        assert!(pressure_from(Some(some), Some(some), Some(some)).is_some());
        assert_eq!(pressure_from(None, Some(some), Some(some)), None);
        assert_eq!(pressure_from(Some(some), None, Some(some)), None);
        assert_eq!(pressure_from(Some(some), Some(some), None), None);
    }

    #[test]
    fn a_pressure_file_that_makes_no_sense_reports_nothing_rather_than_guessing() {
        assert_eq!(parse_pressure(""), Stall::default());
        assert_eq!(parse_pressure("some avg60=1.0\n"), Stall::default());
        assert_eq!(parse_pressure("garbage\n"), Stall::default());
    }

    #[test]
    fn the_live_pressure_files_read_as_percentages() {
        // Present on every kernel checked here and absent on plenty still in
        // service, so this asserts the shape of an answer rather than that
        // there is one.
        let pf = ProcFs::new().unwrap();
        let Some(p) = pf.read_pressure() else {
            return;
        };
        for (what, s) in [("cpu", p.cpu), ("io", p.io), ("memory", p.memory)] {
            assert!(
                (0.0..=100.0).contains(&s.some),
                "{what} some out of range: {}",
                s.some
            );
            assert!(
                (0.0..=100.0).contains(&s.full),
                "{what} full out of range: {}",
                s.full
            );
            // `full` is a subset of `some` by construction: every task stalled
            // implies some task stalled. A parse that crossed the two lines
            // would break this and nothing else would notice.
            assert!(
                s.full <= s.some + 0.01,
                "{what} full {} exceeds some {}",
                s.full,
                s.some
            );
        }
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
