//! Non-Linux backend, via `sysinfo`.
//!
//! There is no `/proc` here, and the mach calls that replace it are a different
//! project's worth of unsafe. This backend exists so the tool runs on a dev
//! laptop; the `/proc` backend is the one to read for how any of it works.

use super::procinfo::{self, Kinfo};
use super::{Collector, Needs};
use crate::sample::{IoRates, Link, MemStat, NetStat, ProcSample, Sample};
use std::collections::HashMap;
use std::io;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use sysinfo::{Networks, ProcessesToUpdate, System, Users};

/// The fastest sysinfo can be sampled and still report the truth.
///
/// A correctness argument rather than a cost one. sysinfo documents a minimum
/// interval between CPU refreshes, and below it the figures are not noisy —
/// they are wrong, with nothing on screen to say so. Measured on an idle-ish
/// machine, the busiest process reported:
///
/// ```text
///   50ms -> 3.5%      100ms -> 262.9%      1000ms -> 323.8%
/// ```
///
/// Two and a half cores of work, reported as three and a half percent.
/// Taken from sysinfo rather than transcribed. The number and the sentence
/// describing it were separately maintained for about an hour, which is long
/// enough: a message stating a figure the code no longer enforces is worse than
/// one stating none.
pub const MIN_INTERVAL: std::time::Duration = sysinfo::MINIMUM_CPU_UPDATE_INTERVAL;

/// Deliberately without the number in it — the caller already prints
/// `MIN_INTERVAL`, and a second copy is a second thing to keep in step.
pub const MIN_INTERVAL_WHY: &str = "sysinfo needs that long between CPU refreshes on this platform, and below \
     it the per-process figures are wrong rather than merely noisy";

pub struct SysinfoCollector {
    /// Whether a sample has been taken yet. See `sample`: the first one cannot
    /// carry valid CPU figures, so it reports none.
    primed: bool,
    sys: System,
    users: Users,
    /// pid -> (start time, name). Same key as the Linux collector, and for the
    /// same reason: a recycled pid must not inherit the dead process's name.
    names: HashMap<i32, (Option<u64>, Arc<str>)>,
    /// Per-interface counters. sysinfo reports these as deltas since the last
    /// refresh, so unlike the `/proc` backend there is nothing to diff.
    nets: Networks,
    /// Start times for processes this user does not own, which sysinfo will not
    /// report. `None` if this kernel's `kinfo_proc` is not the one
    /// [`Kinfo::probe`] recognises, in which case sysinfo's answer is used and
    /// the third of the table it cannot see has no identity — as before.
    kinfo: Option<Kinfo>,
}

impl SysinfoCollector {
    pub fn new() -> io::Result<Self> {
        Ok(Self {
            primed: false,
            sys: System::new_all(),
            users: Users::new_with_refreshed_list(),
            names: HashMap::new(),
            nets: Networks::new_with_refreshed_list(),
            kinfo: Kinfo::probe(),
        })
    }
}

impl Collector for SysinfoCollector {
    fn notes(&self) -> Vec<String> {
        // Said once at startup, because an absence nobody is told about is the
        // same shape as a machine with no disks. Every other unknowable here
        // announces itself; this one was silent, and the header simply had no
        // storage figure with nothing to explain it.
        vec![
            "no per-device disk figures on this platform: reading them costs 12ms a sample \
             against a budget of about four, so poptop does not read them at all rather than \
             pay it or show a stale number"
                .to_string(),
        ]
    }

    fn collect(&mut self, needs: Needs) -> io::Result<Sample> {
        // `System::new_all` has already refreshed by the time this runs, and
        // this call lands microseconds later — far inside the interval sysinfo
        // needs between CPU refreshes. So the first sample's CPU figures are
        // exactly what the interval floor exists to refuse, and they would
        // otherwise be drawn as the first frame, pushed into history, and
        // persisted.
        //
        // Reported as zero rather than waited out. The `/proc` backend already
        // says zero for its first sample — there is no previous counter to diff
        // against — so this makes the two agree, instead of making the first
        // frame arrive a fifth of a second late.
        let first = !self.primed;
        self.primed = true;
        self.sys.refresh_cpu_all();
        self.sys.refresh_memory();
        self.sys.refresh_processes(ProcessesToUpdate::All, true);

        let cpu_per_core: Vec<f32> = if first {
            vec![0.0; self.sys.cpus().len()]
        } else {
            self.sys.cpus().iter().map(|c| c.cpu_usage()).collect()
        };
        let cpu_total = if cpu_per_core.is_empty() {
            0.0
        } else {
            cpu_per_core.iter().sum::<f32>() / cpu_per_core.len() as f32
        };

        // Read before the process list is walked, not after: a process forked in
        // between would be in sysinfo's list with no start time here, and get
        // none for its first sample. Reading first means the miss is a process
        // that *exited*, which is about to leave the table anyway.
        //
        // One source per session, never a mix. Falling back to sysinfo
        // per-process would put seconds and microseconds in the same field, so
        // a process that slipped from one source to the other would look like a
        // different process — the exact failure this key exists to prevent.
        let starts = self.kinfo.as_mut().map(|k| k.starts());

        // 418us warm for twenty-seven interfaces, measured — about a tenth of a
        // sample, which is affordable where the disk equivalent at 12ms was
        // not.
        self.nets.refresh(false);
        let net = NetStat {
            links: self
                .nets
                .iter()
                // Ever carried a byte. This machine publishes twenty-seven
                // interfaces and thirteen have; the rest are idle tunnels.
                .filter(|(_, d)| d.total_received() + d.total_transmitted() > 0)
                .map(|(name, d)| Link {
                    name: Arc::from(name.as_str()),
                    rx: d.received(),
                    tx: d.transmitted(),
                    rx_packets: d.packets_received(),
                    tx_packets: d.packets_transmitted(),
                })
                .collect(),
            errors: Some(
                self.nets
                    .values()
                    .map(|d| d.errors_on_received() + d.errors_on_transmitted())
                    .sum(),
            ),
            // sysinfo counts errors and does not separate out drops, and there
            // is no TCP counter behind it at all. Three em dashes rather than
            // three zeroes: this platform does not know, which is not the same
            // as nothing having gone wrong.
            drops: None,
            retrans: None,
            listen_drops: None,
        };

        let Self {
            sys, users, names, ..
        } = self;

        // Whose processes we can actually see the IO of.
        //
        // sysinfo returns zeros for a process the caller has no access to, and
        // zero is indistinguishable from idle — so without this every other
        // user's work is reported as doing no IO at all. That is the confident
        // lie the `/proc` backend refuses to tell, and it became a default the
        // moment the columns stopped being opt-in.
        let me = sysinfo::get_current_pid()
            .ok()
            .and_then(|pid| sys.process(pid))
            .and_then(|p| p.user_id().cloned());
        let readable = |uid: Option<&sysinfo::Uid>| match (&me, uid) {
            // Root sees everything; otherwise only our own.
            (Some(mine), _) if **mine == 0 => true,
            (Some(mine), Some(theirs)) => mine == theirs,
            // Our own uid unknown: claim nothing rather than guess.
            _ => false,
        };

        let mut io_denied = 0usize;
        let procs = sys
            .processes()
            .iter()
            .map(|(pid, p)| {
                let id = pid.as_u32() as i32;
                let started = match &starts {
                    Some(table) => table.get(&id).copied(),
                    // No usable `kinfo_proc`. sysinfo counts whole seconds, so
                    // scale to keep the field's unit constant, and treat its
                    // zero as what it means: it would not say.
                    None => match p.start_time() {
                        0 => None,
                        s => Some(s * 1_000_000),
                    },
                };
                let name = cached_name(names, id, started, || {
                    Arc::from(p.name().to_string_lossy().as_ref())
                });
                ProcSample {
                    pid: id,
                    // sysinfo reports no parent for processes this user does not
                    // own, so on macOS roughly a third of pids come back with 0 and
                    // the tree view shows them as roots. The /proc backend has real
                    // parentage for everything.
                    ppid: p.parent().map(|p| p.as_u32() as i32).unwrap_or(0),
                    name,
                    user: p
                        .user_id()
                        .and_then(|uid| users.get_user_by_id(uid))
                        .map(|u| std::sync::Arc::from(u.name()))
                        .unwrap_or_else(|| std::sync::Arc::from("?")),
                    cpu: if first { 0.0 } else { p.cpu_usage() },
                    rss: p.memory(),
                    // sysinfo exposes tasks only on Linux, so this is read
                    // directly — a flat `1` beside a CPU figure of several
                    // hundred percent was the table contradicting itself.
                    threads: procinfo::threads(id),
                    state: status_char(p.status()),
                    started,
                    // sysinfo already reports these as bytes since the last
                    // refresh, so unlike the /proc backend there is no counter to
                    // diff here.
                    io: needs
                        .io
                        .then(|| {
                            if !readable(p.user_id()) {
                                io_denied += 1;
                                return None;
                            }
                            let d = p.disk_usage();
                            Some(IoRates {
                                read: d.read_bytes,
                                write: d.written_bytes,
                            })
                        })
                        .flatten(),
                }
            })
            .collect();

        let load = System::load_average();

        Ok(Sample {
            at: SystemTime::now(),
            cpu_total,
            cpu_per_core,
            mem: MemStat {
                total: self.sys.total_memory(),
                used: self.sys.used_memory(),
                available: self.sys.available_memory(),
                // Not `free_memory()`. See `MemStat::free`: on macOS it is
                // `free - speculative` with a saturating subtract, which is
                // zero on any warm machine — and `used` and `available` overlap
                // anyway, so there is no partition here to draw.
                free: None,
                swap_total: self.sys.total_swap(),
                swap_used: self.sys.used_swap(),
            },
            load: [load.one, load.five, load.fifteen],
            procs,
            uptime: Duration::from_secs(System::uptime()),
            // macOS publishes no equivalent of `/proc/stat`'s `processes`
            // counter, so poptop cannot say how many tasks were created in an
            // interval here. `None`, not zero: "I do not know" and "none
            // happened" are opposite answers, and a fabricated zero would
            // quietly promise the table is complete.
            forks: None,
            // macOS exposes no equivalent of these three either. `None`, not
            // zero: "I cannot see this" and "there is none of it" are opposite
            // answers, and a fabricated zero would say the box is never
            // blocked on anything.
            iowait: None,
            running: None,
            blocked: None,
            // sysinfo reports disk usage for every process it can see, so
            // "does this platform keep the accounting" does not arise here —
            // only "can this user read it", which `io_denied` answers.
            io_supported: true,
            io_collected: needs.io,
            // sysinfo reports per-refresh deltas directly, so there is no
            // permission-denied path to count here.
            io_denied,
            // `sysinfo::Disks::refresh` costs 12.5ms steady state, measured,
            // against a whole sample of about 4ms. An em dash until there is a
            // cheaper route to the same counters — see `notes`.
            disks: None,
            // No equivalent on this platform. Not zero: a machine that never
            // stalls and a machine that cannot say are opposite answers.
            pressure: None,
            net: Some(net),
        })
    }
}

/// Collapse to the single-letter states `ps` uses, so both backends agree.
fn status_char(s: sysinfo::ProcessStatus) -> char {
    use sysinfo::ProcessStatus::*;
    match s {
        Run => 'R',
        Sleep => 'S',
        Idle => 'I',
        Stop => 'T',
        Zombie => 'Z',
        _ => '?',
    }
}

/// The cached name for a process, allocating one only when this is not the
/// process the cache last saw under that pid.
///
/// The allocation is the point: a name is an `Arc<str>` shared by every
/// retained sample, so the ring buffer holds one per process rather than one
/// per row per second.
///
/// Matched on `Some(t) == Some(t)` and never on two `None`s. An unknown start
/// time is not evidence of sameness, and treating it as such lets a pid
/// recycled between two samples inherit the dead process's name — the same
/// splice [`crate::sample::ProcSample::key`] refuses, one field over. That path
/// is live whenever [`Kinfo::probe`] found no usable `kinfo_proc` and sysinfo
/// is answering instead.
fn cached_name(
    names: &mut HashMap<i32, (Option<u64>, Arc<str>)>,
    pid: i32,
    started: Option<u64>,
    fresh: impl FnOnce() -> Arc<str>,
) -> Arc<str> {
    match names.get(&pid) {
        Some((Some(t), n)) if Some(*t) == started => n.clone(),
        _ => {
            let n = fresh();
            names.insert(pid, (started, n.clone()));
            n
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_counters_this_platform_lacks_are_absent_rather_than_zero() {
        // sysinfo counts errors and does not separate drops, and there is no
        // TCP counter behind it at all. Three zeroes would claim a perfectly
        // healthy network on a machine that cannot see one.
        let mut c = SysinfoCollector::new().unwrap();
        let s = c.collect(Needs::default()).unwrap();
        let net = s.net.expect("no network at all");
        assert_eq!(net.drops, None, "drops were invented");
        assert_eq!(net.retrans, None, "retransmits were invented");
        assert_eq!(net.listen_drops, None, "listen drops were invented");
        assert!(net.errors.is_some(), "errors are available and went unread");
    }

    #[test]
    fn the_absence_of_disk_figures_is_announced_once() {
        // An absence nobody is told about is the same shape as a machine with
        // no disks. Every other unknowable here says so; this one was silent,
        // and the header simply had no storage figure to explain.
        let c = SysinfoCollector::new().unwrap();
        let notes = c.notes();
        assert!(
            !notes.is_empty(),
            "nothing was said about the missing disks"
        );
        assert!(
            notes.iter().any(|n| n.contains("disk")),
            "the note does not mention disks: {notes:?}"
        );
    }

    #[test]
    fn a_process_seen_again_keeps_the_name_it_already_had() {
        // The whole reason the cache exists — one allocation per process, not
        // one per row per sample.
        let mut names = HashMap::new();
        let first = cached_name(&mut names, 7, Some(100), || Arc::from("shell"));
        let again = cached_name(&mut names, 7, Some(100), || Arc::from("shell"));
        assert!(Arc::ptr_eq(&first, &again), "the name was reallocated");
    }

    #[test]
    fn a_recycled_pid_does_not_inherit_the_dead_process_name() {
        let mut names = HashMap::new();
        cached_name(&mut names, 7, Some(100), || Arc::from("shell"));
        let after = cached_name(&mut names, 7, Some(200), || Arc::from("compiler"));
        assert_eq!(&*after, "compiler");
    }

    #[test]
    fn two_unknown_start_times_are_not_treated_as_the_same_process() {
        // `None == None` is true and would be a cache hit, so a pid recycled
        // while neither occupant could be timed would show the dead one's name.
        // This is the path taken whenever `Kinfo::probe` finds no usable
        // `kinfo_proc` and sysinfo, which cannot time other users' processes,
        // is answering instead.
        let mut names = HashMap::new();
        cached_name(&mut names, 7, None, || Arc::from("shell"));
        let after = cached_name(&mut names, 7, None, || Arc::from("compiler"));
        assert_eq!(
            &*after, "compiler",
            "an unknown start time was taken as proof of sameness"
        );
    }
}
