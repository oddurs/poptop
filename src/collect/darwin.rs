//! Non-Linux backend, via `sysinfo`.
//!
//! There is no `/proc` here, and the mach calls that replace it are a different
//! project's worth of unsafe. This backend exists so the tool runs on a dev
//! laptop; the `/proc` backend is the one to read for how any of it works.

use super::{Collector, Needs};
use crate::sample::{IoRates, MemStat, ProcSample, Sample};
use std::collections::HashMap;
use std::io;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use sysinfo::{ProcessesToUpdate, System, Users};

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
    names: HashMap<i32, (u64, Arc<str>)>,
}

impl SysinfoCollector {
    pub fn new() -> io::Result<Self> {
        Ok(Self {
            primed: false,
            sys: System::new_all(),
            users: Users::new_with_refreshed_list(),
            names: HashMap::new(),
        })
    }
}

impl Collector for SysinfoCollector {
    fn sample(&mut self, needs: Needs) -> io::Result<Sample> {
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
                let started = p.start_time();
                let name = match names.get(&id) {
                    Some((t, n)) if *t == started => n.clone(),
                    _ => {
                        let n: Arc<str> = Arc::from(p.name().to_string_lossy().as_ref());
                        names.insert(id, (started, n.clone()));
                        n
                    }
                };
                ProcSample {
                    pid: pid.as_u32() as i32,
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
                    // sysinfo exposes tasks only on Linux, where we use the other
                    // backend anyway.
                    threads: 1,
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
            io_collected: needs.io,
            // sysinfo reports per-refresh deltas directly, so there is no
            // permission-denied path to count here.
            io_denied,
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
