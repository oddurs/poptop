//! Non-Linux backend, via `sysinfo`.
//!
//! There is no `/proc` here, and the mach calls that replace it are a different
//! project's worth of unsafe. This backend exists so the tool runs on a dev
//! laptop; the `/proc` backend is the one to read for how any of it works.

use super::procinfo::{self, Kinfo};
use super::{Collector, Needs, Source};

/// Every optional source this backend reads.
///
/// No threads: sysinfo exposes no per-thread accounting, and mach's
/// `task_threads` — which would — is not called here. Declared rather than left
/// implicit, so `y` cannot start a collection that will never produce a row and
/// the budget cannot give up something that was never costing anything.
pub const SUPPORTED: &[Source] = &[Source::Io];
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
    /// Per-interface counters. sysinfo reports these as counts *since the last
    /// refresh*, so there is nothing to diff — but there is still something to
    /// divide by, which is what `net_at` is for.
    nets: Networks,
    /// When the interfaces were last refreshed, so a count since then can be
    /// turned into a rate. Without it a ten-second interval reported ten
    /// seconds of traffic as one second's worth.
    net_at: Option<std::time::Instant>,
    /// Interned interface names, so the ring buffer holds one `Arc<str>` per
    /// interface rather than one per interface per sample. The `/proc` backend
    /// interns against its previous read for the same reason.
    link_names: HashMap<String, Arc<str>>,
    /// Start times for processes this user does not own, which sysinfo will not
    /// report. `None` if this kernel's `kinfo_proc` is not the one
    /// [`Kinfo::probe`] recognises, in which case sysinfo's answer is used and
    /// the third of the table it cannot see has no identity — as before.
    kinfo: Option<Kinfo>,
    /// pid -> (start time, command line). Keyed like `names`, and for the same
    /// reason.
    ///
    /// Held only to avoid rebuilding the string: sysinfo has already read
    /// `argv` by the time this is consulted, so unlike the `/proc` backend
    /// there is no syscall to save. What is saved is one allocation per process
    /// per sample across a buffer of six hundred.
    cmds: HashMap<i32, (Option<u64>, Option<Arc<str>>)>,
    /// Which sample this is, so command lines can be rebuilt on a slot
    /// staggered by pid. Same reason as the `/proc` backend's: `setproctitle`
    /// rewrites `argv` in place — `postgres: writer process` — and a cache
    /// keyed only on the start time would freeze the one title worth watching.
    tick: u64,
}

impl SysinfoCollector {
    pub fn new() -> io::Result<Self> {
        Ok(Self {
            primed: false,
            sys: System::new_all(),
            users: Users::new_with_refreshed_list(),
            names: HashMap::new(),
            nets: Networks::new_with_refreshed_list(),
            net_at: None,
            link_names: HashMap::new(),
            kinfo: Kinfo::probe(),
            cmds: HashMap::new(),
            tick: 0,
        })
    }
}

impl Collector for SysinfoCollector {
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
        // sysinfo counts bytes *since the last refresh*, not per second, and
        // the two only coincide at a one-second interval. At `interval = 10s` a
        // link doing 4.4K/s was being reported at 44K/s.
        let now = std::time::Instant::now();
        let secs = self
            .net_at
            .replace(now)
            .map_or(0.0, |t| now.duration_since(t).as_secs_f64());
        let rate = |n: u64| {
            if secs > 0.0 {
                (n as f64 / secs) as u64
            } else {
                0
            }
        };
        let names = &mut self.link_names;
        let net = NetStat {
            links: self
                .nets
                .iter()
                // Ever carried a byte. This machine publishes twenty-seven
                // interfaces and thirteen have; the rest are idle tunnels.
                .filter(|(_, d)| d.total_received() + d.total_transmitted() > 0)
                .map(|(name, d)| Link {
                    name: names
                        .entry(name.clone())
                        .or_insert_with(|| Arc::from(name.as_str()))
                        .clone(),
                    rx: rate(d.received()),
                    tx: rate(d.transmitted()),
                    rx_packets: rate(d.packets_received()),
                    tx_packets: rate(d.packets_transmitted()),
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

        self.tick = self.tick.wrapping_add(1);
        let tick = self.tick;
        let Self {
            sys,
            users,
            names,
            cmds,
            ..
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
        let procs: Vec<ProcSample> = sys
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
                let name = cached(names, id, started, || {
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
                    // Free here: `refresh_processes` already reads `argv`, so
                    // unlike the `/proc` backend there is no extra syscall to
                    // pay for and nothing to cache against. Interned all the
                    // same, so the ring buffer holds one string per process
                    // rather than one per row per second.
                    cmd: cached_until(cmds, id, started, tick, || {
                        let argv: Vec<_> = p.cmd().iter().map(|a| a.to_string_lossy()).collect();
                        crate::sample::command_from_argv(argv.iter().map(|a| a.as_ref()))
                            .map(|c| Arc::from(c.as_str()))
                    }),
                    // sysinfo already reports these as bytes since the last
                    // refresh, so unlike the /proc backend there is no counter to
                    // diff here.
                    io: needs
                        .wants(Source::Io)
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

        // Pruned against the pids just walked, or every process that has ever
        // run leaves an entry behind — a real leak on a build box or a CI
        // runner, which is the kind of machine poptop gets left running on.
        // The `/proc` backend prunes its equivalent against `seen`.
        let live: std::collections::HashSet<i32> =
            procs.iter().map(|p: &ProcSample| p.pid).collect();
        self.cmds.retain(|pid, _| live.contains(pid));
        self.names.retain(|pid, _| live.contains(pid));

        let load = System::load_average();

        // What macOS cannot answer, and why — each of these defaults to `None`
        // through `Sample::unknown()` rather than being restated here:
        //
        // - `forks`: no equivalent of `/proc/stat`'s `processes` counter, so
        //   poptop cannot say how many tasks were created in an interval.
        // - `iowait`, `running`, `blocked`: no equivalent either. A fabricated
        //   zero would say the box is never blocked on anything.
        // - `disks`: `sysinfo::Disks::refresh` costs 12.5ms steady state,
        //   measured, against a whole sample of about 4ms. An em dash until
        //   there is a cheaper route to the same counters — see `notes`.
        // - `pressure`: no equivalent. A machine that never stalls and a
        //   machine that cannot say are opposite answers.
        // - `clock_ceiling`: nothing reachable without shelling out, and
        //   `pmset -g therm` — the documented route — reports nothing at all on
        //   Apple Silicon: "No CPU power status has been recorded". An em dash
        //   rather than 100%, which would claim the machine is running at full
        //   speed on the strength of not being able to look.
        //
        // All of them `None`, never zero: "I cannot see this" and "there is
        // none of it" are opposite answers, and a fabricated zero would quietly
        // promise the table is complete.
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
            // sysinfo reports disk usage for every process it can see, so
            // "does this platform keep the accounting" does not arise here —
            // only "can this user read it", which `io_denied` answers.
            io_supported: true,
            io_collected: needs.wants(Source::Io),
            // sysinfo reports per-refresh deltas directly, so there is no
            // permission-denied path to count here.
            io_denied,
            net: Some(net),
            filesystems: procinfo::filesystems(),
            ..Sample::unknown()
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
/// [`cached`], but the entry also expires on a refresh slot staggered by pid.
///
/// See the `cmds` field: a command line is not quite immutable, and a cache
/// keyed only on the start time never notices a title rewritten in place.
fn cached_until<T: Clone>(
    cache: &mut HashMap<i32, (Option<u64>, T)>,
    pid: i32,
    started: Option<u64>,
    tick: u64,
    fresh: impl FnOnce() -> T,
) -> T {
    let due = tick % CMD_REFRESH == pid.unsigned_abs() as u64 % CMD_REFRESH;
    if !due
        && let Some((Some(t), v)) = cache.get(&pid)
        && Some(*t) == started
    {
        return v.clone();
    }
    let v = fresh();
    cache.insert(pid, (started, v.clone()));
    v
}

/// How many samples a command line is trusted for before it is rebuilt.
///
/// The `/proc` backend has the same constant for the same reason; it is
/// repeated rather than shared because the two backends are never compiled
/// together and a `cfg`-gated `use` for one number is worse than the number.
const CMD_REFRESH: u64 = 30;

fn cached<T: Clone>(
    cache: &mut HashMap<i32, (Option<u64>, T)>,
    pid: i32,
    started: Option<u64>,
    fresh: impl FnOnce() -> T,
) -> T {
    match cache.get(&pid) {
        Some((Some(t), n)) if Some(*t) == started => n.clone(),
        _ => {
            let n = fresh();
            cache.insert(pid, (started, n.clone()));
            n
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interface_counts_are_turned_into_rates() {
        // sysinfo counts bytes *since the last refresh*, and the two only
        // coincide at a one-second interval — which is why the bug survived a
        // default-configured eyeball. At `interval = 10s` a link doing 4.4K/s
        // was reported at 44K/s.
        //
        // Told apart by measuring over *less* than a second: a rate is then
        // strictly larger than the count it came from, and a raw count is not.
        // Traffic is generated here rather than waited for, so the test does
        // not depend on the machine being busy.
        use std::io::{Read as _, Write as _};
        const BYTES: usize = 4 << 20;

        let mut c = SysinfoCollector::new().unwrap();
        c.collect(Needs::default()).unwrap();
        // The collector's window opens at that refresh, so time it from here.
        let opened = std::time::Instant::now();

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut s, _) = listener.accept().unwrap();
            let mut sink = vec![0u8; 1 << 16];
            let mut seen = 0;
            while seen < BYTES {
                match s.read(&mut sink) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => seen += n,
                }
            }
        });
        let mut client = std::net::TcpStream::connect(addr).unwrap();
        let block = vec![0u8; 1 << 16];
        let mut sent = 0;
        while sent < BYTES {
            client.write_all(&block).unwrap();
            sent += block.len();
        }
        drop(client);
        server.join().unwrap();

        std::thread::sleep(std::time::Duration::from_millis(50));
        let secs = opened.elapsed().as_secs_f64();
        let s = c.collect(Needs::default()).unwrap();
        let net = s.net.expect("no network");
        let busiest = net.busiest().expect("no interface carried anything");

        // Compared against a computed expectation rather than against the count
        // itself. TCP framing puts a few percent more on the wire than was
        // sent, which is enough for a raw count to clear a bare `> BYTES` — the
        // first version of this test passed against the bug for exactly that
        // reason. A rate over a window this short is several times the count,
        // so the band is wide enough for a loaded machine and far too tight for
        // the bug.
        // Compared against the *count*, not against a computed rate. The bug
        // reports the count itself; a rate over a window this short is several
        // times larger, and the multiple only has to beat one to distinguish
        // them. Comparing against `bytes / elapsed` looked tighter and was
        // brittle instead — the collector's window and this one are measured by
        // different clocks, and under a loaded test suite they diverged enough
        // to fail on a correct reading.
        assert!(
            secs < 0.5,
            "the window was {secs:.3}s, too long for a rate to be distinguishable"
        );
        assert!(
            busiest.rx as f64 > BYTES as f64 * 1.5,
            "{} reported {} for {BYTES} bytes over {secs:.3}s — a rate would be \
             several times the count, and a raw count would read about {BYTES}",
            busiest.name,
            busiest.rx
        );
    }

    #[test]
    fn interface_names_are_interned_across_samples() {
        // These live in the ring buffer, which holds up to 86401 samples. One
        // allocation per interface per sample is a million strings a day.
        let mut c = SysinfoCollector::new().unwrap();
        let a = c.collect(Needs::default()).unwrap().net.unwrap();
        let b = c.collect(Needs::default()).unwrap().net.unwrap();
        let (Some(x), Some(y)) = (a.links.first(), b.links.first()) else {
            return;
        };
        assert!(
            Arc::ptr_eq(&x.name, &y.name),
            "the interface name was reallocated"
        );
    }

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
    fn a_platform_limitation_is_documented_rather_than_announced() {
        // It used to be a startup note, on every run of every mode — two
        // hundred characters explaining an implementation cost, printed above
        // three lines of `--once` output and above `--help` itself.
        //
        // `notes` is for what a backend could not *determine*: the Linux
        // collector uses it to say it had to assume a page size, which would
        // make every RSS figure wrong by a factor of four if the guess were
        // wrong. That is a warning. "This platform does not read disks" is an
        // absence — nothing on screen is wrong — and it never changes on a
        // given machine, so repeating it forever is noise.
        let mut c = SysinfoCollector::new().unwrap();
        let notes = c.take_notes();
        assert!(
            notes.is_empty(),
            "an absence is being announced as an assumption: {notes:?}"
        );

        // It is still said, where someone wondering why the figure is missing
        // would look.
        assert!(
            crate::USAGE.contains("\nON MACOS:\n"),
            "the platform's limits are not documented anywhere findable"
        );
        for missing in ["per-device disk", "stall pressure", "network drops"] {
            assert!(
                crate::USAGE.contains(missing),
                "{missing} is absent on this platform and unexplained"
            );
        }
    }

    #[test]
    fn the_per_process_caches_do_not_grow_without_bound() {
        // Every pid that ever ran would otherwise leave an entry behind holding
        // an `Arc<str>` — a real leak on a build box or a CI runner, which is
        // the kind of machine poptop gets left running on. The `/proc` backend
        // prunes its equivalents against the pids it just walked.
        let mut c = SysinfoCollector::new().unwrap();
        c.collect(Needs::default()).unwrap();
        c.cmds.insert(-12345, (Some(1), Some(Arc::from("a ghost"))));
        c.names.insert(-12345, (Some(1), Arc::from("a ghost")));
        c.collect(Needs::default()).unwrap();
        assert!(
            !c.cmds.contains_key(&-12345),
            "the command line cache kept an exited process"
        );
        assert!(
            !c.names.contains_key(&-12345),
            "the name cache kept an exited process"
        );
    }

    #[test]
    fn a_title_rewritten_in_place_is_picked_up() {
        // `setproctitle` rewrites argv without the process restarting, which is
        // how postgres shows `postgres: writer process`. Keyed only on the
        // start time, the cache would freeze the one title worth watching —
        // and here there is not even a syscall saved by holding it, because
        // sysinfo has already read argv by the time this runs.
        let mut cache = HashMap::new();
        let pid: i32 = 7;
        cache.insert(pid, (Some(100), Some(Arc::<str>::from("stale"))));
        let due = pid.unsigned_abs() as u64 % CMD_REFRESH;
        let got = cached_until(&mut cache, pid, Some(100), due, || {
            Some(Arc::<str>::from("postgres: writer process"))
        });
        assert_eq!(got.as_deref(), Some("postgres: writer process"));

        // And on a tick that is not its slot, the cache is still a cache.
        cache.insert(pid, (Some(100), Some(Arc::<str>::from("held"))));
        let quiet = (0..CMD_REFRESH).find(|t| t % CMD_REFRESH != due).unwrap();
        let got = cached_until(&mut cache, pid, Some(100), quiet, || {
            panic!("the cache was not consulted")
        });
        assert_eq!(got.as_deref(), Some("held"));
    }

    #[test]
    fn a_process_seen_again_keeps_the_name_it_already_had() {
        // The whole reason the cache exists — one allocation per process, not
        // one per row per sample.
        let mut names: HashMap<i32, (Option<u64>, Arc<str>)> = HashMap::new();
        let first = cached(&mut names, 7, Some(100), || Arc::from("shell"));
        let again = cached(&mut names, 7, Some(100), || Arc::from("shell"));
        assert!(Arc::ptr_eq(&first, &again), "the name was reallocated");
    }

    #[test]
    fn a_recycled_pid_does_not_inherit_the_dead_process_name() {
        let mut names: HashMap<i32, (Option<u64>, Arc<str>)> = HashMap::new();
        cached(&mut names, 7, Some(100), || Arc::from("shell"));
        let after = cached(&mut names, 7, Some(200), || Arc::from("compiler"));
        assert_eq!(&*after, "compiler");
    }

    #[test]
    fn two_unknown_start_times_are_not_treated_as_the_same_process() {
        // `None == None` is true and would be a cache hit, so a pid recycled
        // while neither occupant could be timed would show the dead one's name.
        // This is the path taken whenever `Kinfo::probe` finds no usable
        // `kinfo_proc` and sysinfo, which cannot time other users' processes,
        // is answering instead.
        let mut names: HashMap<i32, (Option<u64>, Arc<str>)> = HashMap::new();
        cached(&mut names, 7, None, || Arc::from("shell"));
        let after = cached(&mut names, 7, None, || Arc::from("compiler"));
        assert_eq!(
            &*after, "compiler",
            "an unknown start time was taken as proof of sameness"
        );
    }
}
