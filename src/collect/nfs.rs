//! NFS, client and server, from what the kernel already publishes.
//!
//! Three files, all world-readable, none of which needs a daemon or a library:
//! `/proc/self/mountstats` for what each mount is doing, `/proc/net/rpc/nfs`
//! for the client totals and `/proc/net/rpc/nfsd` for the server's. See the
//! reading rule in the README — this is the shape of source poptop takes.
//!
//! Everything here is a pure function over text. The counters are cumulative,
//! so [`Prev`] holds the last reading and the collector differences them; a
//! counter that went backwards (a mount that was unmounted and mounted again,
//! or a server restarted) yields nothing for that interval rather than the
//! enormous number an unsigned subtraction would produce.

use crate::sample::{NfsMount, NfsStat};
use std::collections::HashMap;
use std::sync::Arc;

/// Cumulative figures for one mount, straight out of the file.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RawMount {
    pub mount: Arc<str>,
    pub server: Arc<str>,
    pub read: u64,
    pub write: u64,
    pub ops: u64,
    pub trans: u64,
    /// Cumulative round-trip time across every completed call, milliseconds.
    pub rtt: u64,
}

/// Cumulative client and server figures.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Raw {
    pub mounts: Vec<RawMount>,
    pub client_calls: u64,
    pub client_retrans: u64,
    pub server: Option<RawServer>,
}

/// What `/proc/net/rpc/nfsd` says, when this machine is actually serving.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RawServer {
    pub calls: u64,
    pub read: u64,
    pub write: u64,
    pub hits: u64,
    pub misses: u64,
    pub badauth: u64,
}

/// The previous reading, so cumulative counters can be turned into intervals.
pub type Prev = Option<Raw>;

/// Every NFS mount in `/proc/self/mountstats`.
///
/// The file lists every mount on the machine, NFS or not; a section is kept
/// only where the fstype is `nfs` or `nfs4`. `nfsd` is a different filesystem
/// with a similar name — the server's own control interface, mounted on
/// `/proc/fs/nfsd` — and matching it would report the server as a mount.
pub fn parse_mountstats(text: &str) -> Vec<RawMount> {
    let mut out: Vec<RawMount> = Vec::new();
    let mut current: Option<RawMount> = None;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("device ") {
            out.extend(current.take());
            current = mount_header(rest);
            continue;
        }
        let Some(m) = current.as_mut() else { continue };
        let line = line.trim_start();
        if let Some(rest) = line.strip_prefix("bytes:") {
            // Eight counters. The fifth and sixth are what actually crossed the
            // wire; the first two are what the application asked for, and the
            // difference between them is the page cache — which is not a fact
            // about the mount.
            let n: Vec<u64> = nums(rest);
            m.read = n.get(4).copied().unwrap_or(0);
            m.write = n.get(5).copied().unwrap_or(0);
        } else if let Some((_, rest)) = op_line(line) {
            // `ops trans timeouts bytes_sent bytes_recv queue rtt execute`,
            // summed across every operation: the header has room for one
            // figure a mount, not one a call type.
            //
            // Scanned rather than collected. NFSv4.2 writes seventy-odd of
            // these lines per mount and only three of the numbers are wanted,
            // so a `Vec` a line is the whole cost of this file: at four hundred
            // mounts it was 257us a sample and is 61us now.
            let mut it = rest.split_whitespace().map_while(|w| w.parse::<u64>().ok());
            if let (Some(ops), Some(trans), Some(rtt)) = (it.next(), it.next(), it.nth(4)) {
                m.ops += ops;
                m.trans += trans;
                m.rtt += rtt;
            }
        }
    }
    out.extend(current);
    out
}

/// The `device <server> mounted on <mount> with fstype <fs>` line, if it names
/// an NFS mount.
fn mount_header(rest: &str) -> Option<RawMount> {
    let (server, rest) = rest.split_once(" mounted on ")?;
    let (mount, rest) = rest.split_once(" with fstype ")?;
    // `nfs4 statvers=1.1` as well as a bare `nfs`, and never `nfsd`.
    let fs = rest.split_whitespace().next()?;
    (fs == "nfs" || fs == "nfs4").then(|| RawMount {
        mount: Arc::from(mount),
        server: Arc::from(server),
        ..RawMount::default()
    })
}

/// A per-operation line — `GETATTR: 100 100 0 …` — as its name and figures.
///
/// Matched on shape rather than on a list of operation names: NFSv4.2 has
/// seventy-odd, they differ by version, and a table of them here would be a
/// table to keep up to date for no gain.
fn op_line(line: &str) -> Option<(&str, &str)> {
    let (name, rest) = line.split_once(':')?;
    let ok = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_');
    ok.then_some((name, rest))
}

/// `/proc/net/rpc/nfs` — the client's own totals.
///
/// `rpc <calls> <retrans> <authrefresh>`. Present on any kernel with the `nfs`
/// module whether or not anything is mounted, which is why the caller decides
/// there is NFS here from the mounts rather than from this file existing.
pub fn parse_rpc_nfs(text: &str) -> Option<(u64, u64)> {
    let n: Vec<u64> = nums(text.lines().find_map(|l| l.strip_prefix("rpc "))?);
    Some((*n.first()?, n.get(1).copied().unwrap_or(0)))
}

/// `/proc/net/rpc/nfsd` — the server's, and `None` unless it is running.
///
/// The test is `th`, the count of `nfsd` threads. The file exists on any kernel
/// with the module loaded and reads as zeroes, so presence would report every
/// machine as an idle NFS server — and a box that does not serve NFS is a
/// different machine from one whose server is quiet.
pub fn parse_rpc_nfsd(text: &str) -> Option<RawServer> {
    let field = |key: &str| -> Vec<u64> {
        text.lines()
            .find_map(|l| l.strip_prefix(key))
            .map(nums)
            .unwrap_or_default()
    };
    let threads = *field("th ").first()?;
    if threads == 0 {
        return None;
    }
    let rc = field("rc ");
    let io = field("io ");
    // `rpc <calls> <badcalls> <badfmt> <badauth> <badclnt>`.
    let rpc = field("rpc ");
    Some(RawServer {
        calls: rpc.first().copied().unwrap_or(0),
        read: io.first().copied().unwrap_or(0),
        write: io.get(1).copied().unwrap_or(0),
        hits: rc.first().copied().unwrap_or(0),
        misses: rc.get(1).copied().unwrap_or(0),
        badauth: rpc.get(3).copied().unwrap_or(0),
    })
}

/// The whitespace-separated integers of a line, stopping at the first thing
/// that is not one.
///
/// `th` is followed by ten decimal histogram buckets and `rpc` is not, so
/// parsing what is there and ignoring the rest is what keeps one shape of line
/// from needing a second parser.
fn nums(rest: &str) -> Vec<u64> {
    rest.split_whitespace()
        .map_while(|w| w.parse::<u64>().ok())
        .collect()
}

/// One sample: what to report, and what to remember.
///
/// `None` for `now` is "this machine has no NFS", which **forgets** the last
/// reading rather than keeping it. Left standing, a machine whose mounts all
/// went away and later came back would difference against a reading from
/// arbitrarily long ago and report that whole span as one interval —
/// `/proc/net/rpc/nfs` keeps accumulating whether or not anything is mounted.
///
/// Split from the reads so that reset is testable without a filesystem.
pub fn step(prev: &mut Prev, now: Option<Raw>, secs: f64) -> Option<NfsStat> {
    let Some(now) = now else {
        *prev = None;
        return None;
    };
    let out = since(prev, &now, secs);
    *prev = Some(now);
    out
}

/// Two readings as one interval.
///
/// `None` for the very first sample, which has nothing to difference against —
/// the same rule the disk and network rates follow, and better than a first
/// frame reporting everything the machine has done since boot as though it
/// happened in one second.
pub fn since(prev: &Prev, now: &Raw, secs: f64) -> Option<NfsStat> {
    let before = prev.as_ref()?;
    if secs <= 0.0 {
        return None;
    }
    // Per second, like the disk and network figures beside it on the header.
    // As raw deltas these were labelled `op/s` and were not: at
    // `--interval=10s` a mount doing 1.2k a second read as 12k, and at 500ms it
    // read as half what it was doing.
    let rate = |n: u64| (n as f64 / secs) as u64;
    // Keyed on the mount point *and* the server: a mount point reused for a
    // different export is a different mount, and differencing across the swap
    // would report one enormous interval.
    let was: HashMap<(&str, &str), &RawMount> = before
        .mounts
        .iter()
        .map(|m| ((&*m.mount, &*m.server), m))
        .collect();

    let mounts = now
        .mounts
        .iter()
        .map(|m| {
            let old = was.get(&(&*m.mount, &*m.server));
            // A mount seen for the first time contributes nothing this
            // interval rather than everything since it was mounted.
            let d = |f: fn(&RawMount) -> u64| old.map_or(0, |o| f(m).saturating_sub(f(o)));
            let ops = d(|m| m.ops);
            let rtt = d(|m| m.rtt);
            NfsMount {
                mount: m.mount.clone(),
                server: m.server.clone(),
                read: rate(d(|m| m.read)),
                write: rate(d(|m| m.write)),
                ops: rate(ops),
                // `trans` counts transmissions and `ops` completions, so the
                // difference is what had to be sent again.
                retrans: rate(d(|m| m.trans).saturating_sub(ops)),
                // A mean over the interval, not a rate — it is already a
                // duration, and dividing it by the interval would make the
                // same mount look faster at a longer one.
                //
                // A mount that completed no call has no latency. Zero would
                // say every call was instant, which is the opposite of what an
                // idle mount means.
                rtt_ms: (ops > 0).then(|| rtt as f32 / ops as f32),
            }
        })
        .collect();

    // Both halves, like the mounts above: a server that was not running last
    // sample has no interval to report, only a life. `nfsd`'s counters survive
    // `rpc.nfsd 0` — the thread count is what goes to zero, and the thread
    // count is what this is gated on — so treating an absent previous reading
    // as a zero one would put everything since boot on the header the moment
    // somebody restarted the server.
    //
    // A server restarted rather than stopped has counters that went backwards;
    // `saturating_sub` reports that interval as quiet rather than as the whole
    // of a `u64`.
    let server = now.server.zip(before.server).map(|(s, old)| RawServer {
        calls: rate(s.calls.saturating_sub(old.calls)),
        read: rate(s.read.saturating_sub(old.read)),
        write: rate(s.write.saturating_sub(old.write)),
        hits: rate(s.hits.saturating_sub(old.hits)),
        misses: rate(s.misses.saturating_sub(old.misses)),
        badauth: rate(s.badauth.saturating_sub(old.badauth)),
    });

    Some(NfsStat {
        mounts,
        client_calls: rate(now.client_calls.saturating_sub(before.client_calls)),
        client_retrans: rate(now.client_retrans.saturating_sub(before.client_retrans)),
        server_calls: server.map(|s| s.calls),
        server_read: server.map(|s| s.read),
        server_write: server.map(|s| s.write),
        server_hits: server.map(|s| s.hits),
        server_misses: server.map(|s| s.misses),
        server_badauth: server.map(|s| s.badauth),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two NFS mounts and two things that are not, in the shape the kernel
    /// writes them. The second mount is the one under load.
    const MOUNTSTATS: &str = "\
device /dev/vda1 mounted on / with fstype ext4
device nfsd mounted on /proc/fs/nfsd with fstype nfsd
device 10.0.0.1:/export mounted on /mnt/data with fstype nfs4 statvers=1.1
\topts:\trw,vers=4.2,rsize=1048576,wsize=1048576
\tage:\t12345
\tbytes:\t900 800 0 0 4096 8192 1 2
\tRPC iostats version: 1.1  p/v: 100003/4 (nfs)
\txprt:\ttcp 0 0 1 0 5 1000 1000 0 1000 0 2 3 4
\tper-op statistics
\t        NULL: 1 1 0 44 24 0 4 8 0
\t     GETATTR: 100 104 0 1000 2000 0 296 400 0
\t       WRITE: 9 9 0 500 100 0 100 120 0
device 10.0.0.2:/other mounted on /mnt/other with fstype nfs statvers=1.1
\tbytes:\t1 2 3 4 5 6 7 8
\tper-op statistics
\t     GETATTR: 2 2 0 10 20 0 6 8 0
device tmpfs mounted on /run with fstype tmpfs
";

    #[test]
    fn only_the_nfs_mounts_are_taken_and_nfsd_is_not_one() {
        let m = parse_mountstats(MOUNTSTATS);
        let names: Vec<&str> = m.iter().map(|m| &*m.mount).collect();
        assert_eq!(
            names,
            ["/mnt/data", "/mnt/other"],
            "a non-NFS mount was taken, or an NFS one was missed"
        );
        assert_eq!(&*m[0].server, "10.0.0.1:/export");

        // The fifth and sixth `bytes:` counters, not the first two: what
        // crossed the wire, not what the application asked for.
        assert_eq!(m[0].read, 4096);
        assert_eq!(m[0].write, 8192);

        // Summed across every operation, and only the operations.
        assert_eq!(m[0].ops, 1 + 100 + 9);
        assert_eq!(m[0].trans, 1 + 104 + 9);
        assert_eq!(m[0].rtt, 4 + 296 + 100);

        // The second mount's figures are its own.
        assert_eq!(m[1].read, 5);
        assert_eq!(m[1].ops, 2);
    }

    #[test]
    fn a_mounts_interval_is_the_difference_and_never_a_first_reading() {
        let now = Raw {
            mounts: parse_mountstats(MOUNTSTATS),
            client_calls: 500,
            client_retrans: 12,
            server: None,
        };
        // Nothing to difference against: the first sample reports no NFS
        // rather than everything since the mount was made.
        assert!(since(&None, &now, 1.0).is_none());

        let mut before = now.clone();
        before.mounts[0].read = 96;
        before.mounts[0].ops = 10;
        before.mounts[0].trans = 10;
        before.mounts[0].rtt = 100;
        before.client_calls = 400;
        before.client_retrans = 10;

        let d = since(&Some(before.clone()), &now, 1.0).expect("two readings is an interval");
        assert_eq!(d.mounts[0].read, 4000);
        assert_eq!(d.mounts[0].ops, 100);
        // 114 transmissions for 110 completions in total, less the 10 already
        // seen: four had to be sent again.
        assert_eq!(d.mounts[0].retrans, 4);
        assert_eq!(d.mounts[0].rtt_ms, Some(3.0));
        assert_eq!(d.client_calls, 100);
        assert_eq!(d.client_retrans, 2);

        // A mount that appeared since the last reading contributes nothing,
        // rather than its whole life as one interval.
        assert_eq!(d.mounts[1].ops, 0, "a new mount reported its whole life");
        assert_eq!(d.mounts[1].rtt_ms, None, "an idle mount claimed a latency");

        // Per second, not per interval. Labelled `op/s` and computed as a raw
        // delta, the same mount read as ten times busier at `--interval=10s`.
        let ten = since(&Some(before), &now, 10.0).unwrap();
        assert_eq!(
            ten.mounts[0].ops, 10,
            "the call rate scaled with the interval"
        );
        assert_eq!(ten.mounts[0].read, 400);
        assert_eq!(ten.client_calls, 10);
        // …except the round trip, which is already a duration: the same mount
        // is not faster because it was watched for longer.
        assert_eq!(ten.mounts[0].rtt_ms, Some(3.0));
    }

    #[test]
    fn a_machine_that_stops_mounting_nfs_forgets_what_it_read() {
        // `/proc/net/rpc/nfs` accumulates whether or not anything is mounted,
        // so a reading kept across a period with no mounts would come back as
        // one interval covering the whole gap.
        let raw = |calls| Raw {
            mounts: parse_mountstats(MOUNTSTATS),
            client_calls: calls,
            client_retrans: 0,
            server: None,
        };
        let mut prev: Prev = None;
        assert!(
            step(&mut prev, Some(raw(100)), 1.0).is_none(),
            "a first reading"
        );
        assert_eq!(
            step(&mut prev, Some(raw(150)), 1.0).unwrap().client_calls,
            50
        );

        // Every mount goes away.
        assert!(step(&mut prev, None, 1.0).is_none());
        assert!(
            prev.is_none(),
            "the reading survived the machine losing its mounts"
        );

        // …and comes back an hour later. The first sample after that has
        // nothing to difference against, which is the same rule a fresh start
        // follows — not an hour of calls attributed to one second.
        assert!(
            step(&mut prev, Some(raw(9_000_000)), 1.0).is_none(),
            "an hour of calls was reported as one interval"
        );
        assert_eq!(
            step(&mut prev, Some(raw(9_000_010)), 1.0)
                .unwrap()
                .client_calls,
            10
        );
    }

    #[test]
    fn a_server_that_appeared_between_samples_reports_no_interval() {
        // `nfsd`'s counters survive `rpc.nfsd 0` — the thread count is what
        // goes to zero, and the thread count is what this is gated on. So a
        // server that was stopped and started again has a full life of calls
        // behind it, and treating "no reading last time" as "a reading of
        // zero" would put all of it on the header for one sample.
        let mut now = Raw {
            mounts: Vec::new(),
            client_calls: 0,
            client_retrans: 0,
            server: Some(RawServer {
                calls: 8_000_000,
                ..RawServer::default()
            }),
        };
        let before = Raw {
            server: None,
            ..now.clone()
        };
        let d = since(&Some(before.clone()), &now, 1.0).unwrap();
        assert_eq!(
            d.server_calls, None,
            "a server seen for the first time reported everything since boot"
        );

        // And once there is a reading to difference against, it is an interval.
        now.server.as_mut().unwrap().calls += 500;
        let d = since(&Some(now.clone()), &now, 1.0).unwrap();
        assert_eq!(d.server_calls, Some(0));
    }

    #[test]
    fn a_counter_that_went_backwards_reports_a_quiet_interval() {
        // A server restart, or a mount point reused for a different export.
        // Unsigned subtraction would put most of a `u64` on the header.
        let now = Raw {
            mounts: parse_mountstats(MOUNTSTATS),
            client_calls: 1,
            client_retrans: 0,
            server: Some(RawServer {
                calls: 5,
                read: 1,
                write: 1,
                hits: 1,
                misses: 1,
                badauth: 0,
            }),
        };
        let mut before = now.clone();
        before.client_calls = 9_000;
        before.mounts[0].ops = 9_000;
        before.server = Some(RawServer {
            calls: 9_000,
            ..RawServer::default()
        });

        let d = since(&Some(before), &now, 1.0).unwrap();
        assert_eq!(d.client_calls, 0);
        assert_eq!(d.mounts[0].ops, 0);
        assert_eq!(d.server_calls, Some(0));
    }

    #[test]
    fn a_server_with_no_threads_is_not_a_quiet_server() {
        // Every kernel with the module loaded publishes this file, reading as
        // zeroes. Taking presence as the test would report every machine as an
        // NFS server that happens to be idle.
        const IDLE: &str = "rc 0 0 0\nfh 0 0 0 0 0\nio 0 0\n\
            th 0 0 0.000 0.000\nra 0 0\nnet 0 0 0 0\nrpc 0 0 0 0 0\n";
        assert!(parse_rpc_nfsd(IDLE).is_none());

        const SERVING: &str = "rc 100 20 8\nfh 0 0 0 0 0\nio 4096 8192\n\
            th 4 0 0.000 0.000\nra 0 0\nnet 8 0 8 1\nrpc 512 3 0 7 1\n";
        let s = parse_rpc_nfsd(SERVING).expect("four threads is a running server");
        assert_eq!(s.calls, 512);
        assert_eq!(s.read, 4096);
        assert_eq!(s.write, 8192);
        assert_eq!(s.hits, 100);
        assert_eq!(s.misses, 20);
        // The fourth field of `rpc`, not the second: `badcalls` and `badauth`
        // are different failures and only one of them is a misconfiguration.
        assert_eq!(s.badauth, 7);

        // A file without a `th` line at all says nothing.
        assert!(parse_rpc_nfsd("rc 1 2 3\n").is_none());
    }

    #[test]
    fn the_clients_totals_are_calls_and_retransmissions() {
        const NFS: &str = "net 0 0 0 0\nrpc 4096 17 4096\n\
            proc3 22 0 0 0\nproc4 69 0 0\n";
        assert_eq!(parse_rpc_nfs(NFS), Some((4096, 17)));
        // `rpc` absent — a file poptop cannot read is not a client doing
        // nothing.
        assert_eq!(parse_rpc_nfs("net 0 0 0 0\n"), None);
    }

    /// What one sample spends on NFS, measured rather than assumed.
    ///
    /// `cargo test --release -- --ignored --nocapture cost_of_nfs`, on a
    /// machine that actually mounts some. The question is whether this belongs
    /// behind the cost model with threads, PSS and cgroups; the answer is in
    /// the numbers it prints.
    #[test]
    #[ignore]
    fn cost_of_nfs() {
        let real = std::fs::read_to_string("/proc/self/mountstats").unwrap_or_default();
        // A machine with many mounts, built by repeating this one's file
        // rather than the fixture above: NFSv4.2 writes seventy-odd per-op
        // lines a mount and the fixture has three, so the fixture would make
        // the parse look twenty times cheaper than it is.
        let many: String = std::iter::repeat_n(real.as_str(), 100).collect();
        for (what, text) in [("this machine", real.as_str()), ("×100", many.as_str())] {
            let n = 200;
            let t = std::time::Instant::now();
            let mut kept = 0;
            for _ in 0..n {
                kept += parse_mountstats(text).len();
            }
            let each = t.elapsed() / n;
            println!(
                "{what}: {} bytes, {} of {} mounts are NFS, {each:?} a sample",
                text.len(),
                kept / n as usize,
                text.lines().filter(|l| l.starts_with("device ")).count(),
            );
        }
    }
}
