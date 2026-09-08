//! Optionally keeping history across restarts.
//!
//! **Off by default, and that is not a shrug.** poptop's position against atop is
//! that nothing has to have been running beforehand — you can install it during
//! an incident and immediately scrub back through the last ten minutes, because
//! the buffer fills from the moment it starts. A tool that needs a recorder
//! primed in advance is a different tool, and it is the one atop already is and
//! does better. So this is a convenience for a machine you sit in front of
//! often, never the path the zero-setup argument rests on.
//!
//! Written on a clean exit and read at startup. Deliberately not a daemon and
//! not a periodic flush: a background writer is the thing that turns a live
//! tool into a recorder, and the item that asked for this said so.
//!
//! The format is hand-rolled and versioned, like everything else here. A store
//! written by a different version is discarded rather than guessed at.

use crate::sample::Sample;
use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Bumped whenever the layout below changes. An old store is dropped, not
/// migrated: it is a cache of something the machine will produce again in
/// minutes, and a migration path for it would cost more than it saves.
// Unmoved by the rename. A store written when this was called `ptop` sits at
// `~/.local/state/ptop/`, which nothing looks in any more, so there is no file
// for a version bump to protect anyone from. The magic changed with the name
// because it spells the name.
const VERSION: u32 = 15;

/// When the machine this sample came from was booted.
///
/// Derived rather than collected: `at - uptime` is already in every sample, on
/// both platforms, and needs no new syscall.
///
/// This matters more than it looks. A process is identified throughout poptop by
/// [`crate::sample::ProcSample::key`], which is `(pid, started)`, and on Linux
/// `started` is clock ticks *since boot* — unique within a boot and nowhere
/// else. `series_for` keys on it precisely so that a recycled pid cannot splice
/// two programs into one graph, and that invariant held only because every
/// sample in the buffer came from one boot. Restoring across a reboot breaks
/// it: early-boot processes land on near-identical starttimes every time, so a
/// live pid 1 would match a restored pid 1 and render the *previous boot's* CPU
/// as this process's own history.
///
/// macOS counts microseconds since the epoch, which does not collide across
/// boots. The check applies to both anyway: the field is deliberately opaque,
/// and a guard that holds only on the platform whose units you happened to
/// check is a guard waiting for a third backend.
pub fn boot_time(s: &Sample) -> SystemTime {
    s.at.checked_sub(s.uptime).unwrap_or(UNIX_EPOCH)
}

/// Whether two boot times are the same boot.
///
/// Uptime is whole seconds while `at` is not, so the derived instant jitters by
/// up to a second between samples of the same boot. A few seconds of tolerance
/// is far below the gap between any two real boots.
pub fn same_boot(a: SystemTime, b: SystemTime) -> bool {
    let delta = a.duration_since(b).or_else(|_| b.duration_since(a));
    delta.is_ok_and(|d| d < Duration::from_secs(5))
}
const MAGIC: &[u8; 10] = b"poptophist";

/// A ceiling on the file, independent of the buffer's own bound.
///
/// The buffer is already bounded by `window` and `interval`, but that bounds
/// *samples*, not bytes — and each sample carries a whole process table, so a
/// build box with four thousand processes writes twenty times what a laptop
/// does at the same settings. The oldest samples are dropped to fit.
const MAX_BYTES: usize = 64 << 20;

/// Where the store lives, honouring `XDG_STATE_HOME`.
///
/// State, not config and not cache: it is neither hand-edited nor trivially
/// regenerated, which is precisely what the XDG state directory is for.
pub fn path() -> Option<PathBuf> {
    path_from(std::env::var_os("XDG_STATE_HOME"), std::env::var_os("HOME"))
}

/// The environment split out, so the rule is testable without mutating a
/// process-global. Same shape as `config::path_from`, for the same reason.
pub fn path_from(
    xdg: Option<std::ffi::OsString>,
    home: Option<std::ffi::OsString>,
) -> Option<PathBuf> {
    let base = xdg
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| {
            home.map(PathBuf::from)
                .filter(|p| p.is_absolute())
                .map(|h| h.join(".local").join("state"))
        })?;
    Some(base.join("poptop").join("history"))
}

use crate::persist::{Codec, In, Out, Raw, Registry, Ty};
use crate::sample::schemas;

/// Bytes the file carries beyond the sample bodies and the schema: magic,
/// version, and the two counts.
const HEADER_BYTES: usize = MAGIC.len() + 4 + 4 + 4;

/// Serialise samples, oldest first, dropping the oldest to fit [`MAX_BYTES`].
pub fn encode(samples: &[&Sample]) -> Vec<u8> {
    encode_within(samples, MAX_BYTES)
}

/// The schema block, which is the same bytes for every file this build writes.
fn schema_bytes() -> Vec<u8> {
    let mut out = Out::default();
    out.schema_block(&schemas());
    out.bytes
}

/// The cap as a parameter, so the trimming rule can be tested without building
/// sixty megabytes of fixture to provoke it.
fn encode_within(samples: &[&Sample], max_bytes: usize) -> Vec<u8> {
    let schema = schema_bytes();
    let fixed = HEADER_BYTES + schema.len();

    // Written newest-first into the body and reversed at the end, so trimming
    // to fit drops the *oldest* — the opposite would throw away the samples
    // most likely to explain whatever made you open poptop.
    let mut kept: Vec<&Sample> = Vec::new();
    let mut out = Out::default();
    for sample in samples.iter().rev() {
        let before = out.bytes.len();
        let table_before = out.table_bytes;
        sample.write(&mut out);
        if fixed + out.table_bytes + out.bytes.len() > max_bytes {
            out.bytes.truncate(before);
            out.table_bytes = table_before;
            break;
        }
        kept.push(sample);
    }

    // Re-encode in chronological order once the set is known. Two passes over
    // at most a few hundred samples on the way out of the program, against the
    // alternative of a length-prefixed reverse-scan format nobody could read.
    let mut out = Out::default();
    for sample in kept.iter().rev() {
        sample.write(&mut out);
    }

    let mut file = Vec::with_capacity(fixed + out.table_bytes + out.bytes.len());
    file.extend_from_slice(MAGIC);
    file.extend_from_slice(&VERSION.to_le_bytes());
    file.extend_from_slice(&schema);
    file.extend_from_slice(&(out.strings.len() as u32).to_le_bytes());
    for s in &out.strings {
        file.extend_from_slice(&(s.len() as u32).to_le_bytes());
        file.extend_from_slice(s.as_bytes());
    }
    file.extend_from_slice(&(kept.len() as u32).to_le_bytes());
    file.extend_from_slice(&out.bytes);
    file
}

/// Parse a store, and whatever the reader had to say about it.
///
/// Failures land here rather than propagating: a wrong magic, a framing version
/// that is not [`VERSION`], a truncated body, a string index with no table
/// entry. This is a cache, and the honest outcome for all of them is starting
/// with an empty buffer.
///
/// A *field* the file has and this build does not is not a failure — it is the
/// case the schema block exists for. It is skipped, and said once, because a
/// metric quietly vanishing from a graph is how a reader stops being able to
/// trust the graph.
pub fn decode_reporting(bytes: &[u8]) -> (Option<Vec<Sample>>, Vec<String>) {
    let mut notes = Vec::new();
    let samples = read_file(bytes, &mut notes);
    (samples, notes)
}

fn read_file(bytes: &[u8], notes: &mut Vec<String>) -> Option<Vec<Sample>> {
    read_file_as(bytes, &schemas(), notes)
}

/// The reader's own schema as a parameter, so a test can read a real store
/// while claiming to be a build that disagrees with it.
fn read_file_as(
    bytes: &[u8],
    mine: &[(&'static str, Vec<crate::persist::Field>)],
    notes: &mut Vec<String>,
) -> Option<Vec<Sample>> {
    let mut r = In::new(bytes, 0, Vec::new());
    if r.take(MAGIC.len())? != MAGIC {
        return None;
    }
    let found = u32::read_raw(&mut r)?;
    if found != VERSION {
        // Named, because "your history is gone" is a thing a user should be
        // able to look up rather than guess at. Only reachable for a file
        // older than the schema block itself; from `VERSION` 15 on, a
        // difference in fields is merged rather than refused.
        notes.push(format!(
            "the stored history is format {found}, this poptop reads {VERSION}; discarded"
        ));
        return None;
    }

    let reg = r.schema_block(mine)?;
    for note in schema_notes(&reg, mine) {
        notes.push(note);
    }
    if reg.fields("Sample").is_none() {
        // Nothing else in the file can be reached without it. Said out loud,
        // because otherwise `unwrap_or_default` at the call site turns the
        // user's whole history into an empty buffer with no explanation.
        notes.push("the stored history declares no samples; it was discarded".to_string());
        return None;
    }

    let n_strings = u32::read_raw(&mut r)?;
    let mut strings = Vec::with_capacity(n_strings.min(1 << 20) as usize);
    for _ in 0..n_strings {
        let len = u32::read_raw(&mut r)? as usize;
        strings.push(Arc::from(std::str::from_utf8(r.take(len)?).ok()?));
    }
    r.strings = strings;

    let n_samples = u32::read_raw(&mut r)?;
    let mut samples = Vec::with_capacity(n_samples.min(1 << 20) as usize);
    let sample_ty = Ty::Rec("Sample".into());
    for _ in 0..n_samples {
        samples.push(if reg.exact {
            Sample::read_exact(&reg, &mut r)?
        } else {
            Sample::read(&sample_ty, &reg, &mut r)?
        });
    }
    Some(samples)
}

/// Fields the file carries that this build has no home for, named once each.
fn schema_notes(
    reg: &Registry,
    mine: &[(&'static str, Vec<crate::persist::Field>)],
) -> Vec<String> {
    let mut out = Vec::new();
    for (rec, want) in mine {
        // A record this build knows and the file does not is not by itself a
        // problem: either nothing refers to it, or the field that does will
        // fail the type-hash check below and be reported there. The one that is
        // fatal is the top-level record, which `read_file_as` checks.
        let Some(got) = reg.fields(rec) else { continue };
        let mut dropped: Vec<&str> = Vec::new();
        let mut retyped: Vec<&str> = Vec::new();
        for f in got {
            match want.iter().find(|w| w.name == f.name) {
                None => dropped.push(&f.name),
                // Same name, different type. The reader skips it rather than
                // misreading it — which is right, and silent, which is not: the
                // column would just empty out. This is the case a downgrade
                // hits after a metric changes precision.
                Some(w) if w.hash != f.hash => retyped.push(&f.name),
                Some(_) => {}
            }
        }
        if !dropped.is_empty() {
            out.push(format!(
                "the stored history has {} this poptop does not read: {}",
                if dropped.len() == 1 {
                    "a field"
                } else {
                    "fields"
                },
                dropped.join(", ")
            ));
        }
        if !retyped.is_empty() {
            out.push(format!(
                "the stored history measures {} differently than this poptop, so {} skipped: {}",
                if retyped.len() == 1 {
                    "a field"
                } else {
                    "fields"
                },
                if retyped.len() == 1 {
                    "it was"
                } else {
                    "they were"
                },
                retyped.join(", ")
            ));
        }
    }
    out
}

/// Parse a store, discarding what the reader had to say.
///
/// Only tests use this: the program has somewhere to put a note, and a reader
/// that throws away what it noticed is the thing this item exists to stop.
#[cfg(test)]
pub fn decode(bytes: &[u8]) -> Option<Vec<Sample>> {
    decode_reporting(bytes).0
}

/// Read the store, if there is one and it is readable.
pub fn load(notes: &mut Vec<String>) -> Option<Vec<Sample>> {
    let (samples, said) = decode_reporting(&std::fs::read(path()?).ok()?);
    notes.extend(said);
    samples
}

/// Write the store, creating its directory.
///
/// Written to a temporary file and renamed, so an interrupted write leaves the
/// previous store rather than a half one — the reader would reject a truncated
/// file anyway, but silently losing yesterday's history to today's crash is a
/// poor trade for two lines.
pub fn save(samples: &[&Sample]) -> io::Result<()> {
    let Some(path) = path() else {
        return Ok(());
    };
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    // Per-process, not a fixed `history.tmp`. Two terminals running poptop is a
    // normal thing to do, and on a shared temporary both writes interleave
    // before either rename — publishing a mixed file that `decode` rejects,
    // losing the whole history. Which is precisely what writing through a
    // temporary was supposed to prevent.
    //
    // The rename still means last-writer-wins between instances: the second to
    // exit replaces the first's history rather than merging it. Merging two
    // buffers is a different feature and not one this item asked for.
    let tmp = path.with_extension(format!("{}.tmp", std::process::id()));
    std::fs::write(&tmp, encode(samples))?;
    std::fs::rename(&tmp, &path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sample::{
        DiskStat, FsStat, IoRates, Link, MemStat, NetStat, Pressure, ProcSample, Stall,
    };

    fn proc_of(pid: i32, name: &str) -> ProcSample {
        ProcSample {
            pid,
            ppid: 1,
            name: Arc::from(name),
            user: Arc::from("oddurs"),
            cpu: 12.5,
            rss: 4 << 20,
            threads: Some(3),
            state: 'S',
            started: Some(987),
            cmd: None,
            io: Some(IoRates {
                read: 100,
                write: 200,
            }),
            container: None,
            minflt: None,
            majflt: None,
            vsize: None,
            nice: None,
            pss: None,
        }
    }

    fn sample_of(cpu: f32, procs: usize) -> Sample {
        Sample {
            at: UNIX_EPOCH + Duration::new(1_700_000_000, 123_456_789),
            cpu_total: cpu,
            cpu_per_core: vec![1.0, 2.5, 99.0],
            // Distinct on purpose. Equal values would let a read that swapped
            // `running` and `blocked` round-trip cleanly, and the field a user
            // scrubs back to is the one that says whether the box was stuck.
            filesystems: Some(vec![FsStat {
                mount: Arc::from("/"),
                total: 500 << 30,
                avail: 42 << 30,
            }]),
            net: Some(NetStat {
                links: vec![Link {
                    name: Arc::from("en0"),
                    rx: 1 << 20,
                    tx: 3 << 18,
                    rx_packets: 900,
                    tx_packets: 400,
                }],
                errors: Some(3),
                // A platform that counts errors and not drops, so the round
                // trip carries both answers rather than only the easy one.
                drops: None,
                retrans: Some(11),
                listen_drops: Some(0),
            }),
            clock_ceiling: None,
            pgin: None,
            pgout: None,
            swin: None,
            swout: None,
            oom_kills: None,
            pressure: Some(Pressure {
                cpu: Stall {
                    some: 1.5,
                    full: 0.0,
                },
                io: Stall {
                    some: 9.25,
                    full: 4.75,
                },
                memory: Stall {
                    some: 0.5,
                    full: 0.25,
                },
            }),
            // Two devices, one with no completed operation in the interval, so
            // the round trip is made to carry a `None` await as well as a real
            // one — the pair this format must not collapse.
            disks: Some(vec![
                DiskStat {
                    name: Arc::from("nvme0n1"),
                    read: 1 << 20,
                    write: 3 << 20,
                    reads: 40,
                    writes: 120,
                    util: 62.5,
                    await_ms: Some(7.75),
                    queue: 3.25,
                },
                DiskStat {
                    name: Arc::from("sdb"),
                    read: 0,
                    write: 0,
                    reads: 0,
                    writes: 0,
                    util: 0.0,
                    await_ms: None,
                    queue: 0.0,
                },
            ]),
            iowait: Some(61.25),
            // Distinct values, so a codec that swapped two of them cannot
            // round-trip cleanly. `guest` is deliberately zero and `intr`
            // absent: the pair a reader has to be able to tell apart.
            steal: Some(12.5),
            guest: Some(0.0),
            irq: Some(3.25),
            softirq: Some(7.75),
            ctxt: Some(48_000),
            intr: None,
            running: Some(3),
            blocked: Some(17),
            mem: MemStat {
                total: 16 << 30,
                used: 8 << 30,
                available: 8 << 30,
                free: Some(5 << 30),
                swap_total: 2 << 30,
                swap_used: 1 << 30,
                dirty: None,
                slab: None,
                slab_reclaimable: None,
                shmem: None,
                page_tables: None,
                huge_total: None,
                huge_used: None,
            },
            load: [1.5, 2.5, 3.5],
            tasks: None,
            exited: None,
            cgroups: None,
            procs: (0..procs).map(|i| proc_of(i as i32, "postgres")).collect(),
            uptime: Duration::from_secs(90_000),
            forks: Some(4242),
            io_supported: true,
            io_collected: true,
            io_denied: 7,
        }
    }

    /// Compare the fields that have to survive, since `Sample` is not `Eq`.
    fn same(a: &Sample, b: &Sample) {
        assert_eq!(a.at, b.at);
        assert_eq!(a.cpu_total, b.cpu_total);
        assert_eq!(a.cpu_per_core, b.cpu_per_core);
        assert_eq!(a.mem.total, b.mem.total);
        assert_eq!(a.mem.available, b.mem.available);
        assert_eq!(a.mem.free, b.mem.free);
        assert_eq!(a.mem.swap_used, b.mem.swap_used);
        assert_eq!(a.load, b.load);
        assert_eq!(a.iowait, b.iowait);
        // The rest of the CPU line. Compared as `Option`s and given real values
        // in the fixture: with every one of them `None` on both sides, a codec
        // that dropped or reordered them round-trips green.
        assert_eq!(a.steal, b.steal);
        assert_eq!(a.guest, b.guest);
        assert_eq!(a.irq, b.irq);
        assert_eq!(a.softirq, b.softirq);
        assert_eq!(a.ctxt, b.ctxt);
        assert_eq!(a.intr, b.intr);
        assert_eq!(a.running, b.running);
        assert_eq!(a.blocked, b.blocked);
        assert_eq!(a.uptime, b.uptime);
        assert_eq!(a.forks, b.forks);
        assert_eq!(a.io_supported, b.io_supported);
        assert_eq!(a.io_collected, b.io_collected);
        assert_eq!(a.io_denied, b.io_denied);
        // Compared as `Option<Vec<_>>`, so "this platform does not read disks"
        // and "it looked and found none" stay distinguishable across the file.
        assert_eq!(a.disks, b.disks);
        assert_eq!(a.pressure, b.pressure);
        assert_eq!(a.net, b.net);
        assert_eq!(a.filesystems, b.filesystems);
        assert_eq!(a.procs.len(), b.procs.len());
        for (x, y) in a.procs.iter().zip(&b.procs) {
            assert_eq!(x.pid, y.pid);
            assert_eq!(x.ppid, y.ppid);
            assert_eq!(&*x.name, &*y.name);
            assert_eq!(&*x.user, &*y.user);
            assert_eq!(x.cpu, y.cpu);
            assert_eq!(x.rss, y.rss);
            assert_eq!(x.threads, y.threads);
            assert_eq!(x.state, y.state);
            assert_eq!(x.started, y.started);
            assert_eq!(
                x.io.map(|i| (i.read, i.write)),
                y.io.map(|i| (i.read, i.write))
            );
        }
    }

    #[test]
    fn a_platform_that_reads_no_disks_does_not_come_back_owning_none() {
        // macOS writes `None` here. Restored as an empty list it would read as
        // "this machine has no disks", which is a claim rather than a silence —
        // and the panel would draw an empty table instead of saying why.
        let mut s = sample_of(1.0, 1);
        s.disks = None;
        assert_eq!(decode(&encode(&[&s])).unwrap()[0].disks, None);

        // And the other way: a machine that looked and found nothing keeps
        // saying so, rather than being turned back into "cannot tell".
        s.disks = Some(Vec::new());
        assert_eq!(
            decode(&encode(&[&s])).unwrap()[0].disks,
            Some(Vec::new()),
            "an empty device list came back as an absent one"
        );
    }

    #[test]
    fn an_unknown_thread_count_does_not_come_back_as_one() {
        // Restoring `1` for a process whose count was never known would put the
        // fabricated figure back on screen, one restart later.
        let mut s = sample_of(1.0, 2);
        s.procs[0].threads = None;
        s.procs[1].threads = Some(36);
        let back = decode(&encode(&[&s])).unwrap();
        assert_eq!(
            back[0].procs[0].threads, None,
            "a thread count was invented"
        );
        assert_eq!(back[0].procs[1].threads, Some(36));
    }

    #[test]
    fn a_process_with_no_start_time_does_not_come_back_with_one() {
        // `None` and `Some(0)` are different answers: the second is a token
        // that compares equal to another zero, which is how two unrelated
        // processes on a recycled pid get spliced into one line. A store that
        // flattened one into the other would reintroduce that on restore.
        let mut s = sample_of(1.0, 2);
        s.procs[0].started = None;
        s.procs[1].started = Some(0);
        let back = decode(&encode(&[&s])).unwrap();
        assert_eq!(
            back[0].procs[0].started, None,
            "an unknown start time was invented"
        );
        assert_eq!(
            back[0].procs[1].started,
            Some(0),
            "a real zero was discarded"
        );
    }

    #[test]
    fn a_sample_survives_the_round_trip_whole() {
        // Every field, because a store that silently drops one gives you
        // history that disagrees with the run that produced it.
        let samples = [sample_of(11.0, 3), sample_of(93.5, 1)];
        let refs: Vec<&Sample> = samples.iter().collect();
        let back = decode(&encode(&refs)).expect("a store this version wrote");
        assert_eq!(back.len(), 2);
        for (a, b) in samples.iter().zip(&back) {
            same(a, b);
        }
    }

    #[test]
    fn an_absent_fork_counter_does_not_come_back_as_zero() {
        // macOS writes `None`. "I do not know" and "none happened" are
        // opposite answers, and a format that collapsed them would turn every
        // restored macOS sample into a claim that nothing was created.
        let mut s = sample_of(1.0, 1);
        s.forks = None;
        s.iowait = None;
        s.running = None;
        s.blocked = None;
        s.procs[0].io = None;
        let back = decode(&encode(&[&s])).unwrap();
        assert_eq!(back[0].forks, None);
        assert_eq!(back[0].iowait, None);
        assert_eq!(back[0].running, None);
        assert_eq!(back[0].blocked, None);
        assert!(back[0].procs[0].io.is_none());
    }

    #[test]
    fn a_store_from_another_version_is_discarded_not_guessed_at() {
        let bytes = encode(&[&sample_of(1.0, 1)]);
        let mut wrong = bytes.clone();
        wrong[MAGIC.len()] = wrong[MAGIC.len()].wrapping_add(1);
        assert!(decode(&wrong).is_none(), "a foreign version was parsed");

        let mut alien = bytes.clone();
        alien[0] = b'x';
        assert!(decode(&alien).is_none(), "a foreign file was parsed");
    }

    #[test]
    fn a_truncated_or_corrupt_store_yields_nothing_rather_than_panicking() {
        // It is a cache of something the machine will produce again in
        // minutes. Every failure has to land on an empty buffer, because the
        // alternatives are refusing to start and inventing history.
        let bytes = encode(&[&sample_of(1.0, 4), &sample_of(2.0, 4)]);
        for cut in 0..bytes.len() {
            let _ = decode(&bytes[..cut]);
        }
        // …including lengths that would otherwise be an allocation request.
        let mut lying = bytes.clone();
        let n = MAGIC.len() + 4;
        lying[n..n + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(decode(&lying).is_none());
    }

    #[test]
    fn the_file_is_bounded_and_drops_the_oldest_to_fit() {
        // The buffer is bounded in samples; this bounds bytes, which is a
        // different thing on a box with four thousand processes.
        let all: Vec<Sample> = (0..40).map(|i| sample_of(i as f32, 50)).collect();
        let refs: Vec<&Sample> = all.iter().collect();
        let whole = encode_within(&refs, usize::MAX).len();

        let bytes = encode_within(&refs, whole / 4);
        let back = decode(&bytes).unwrap();
        assert!(back.len() < all.len(), "nothing was trimmed");
        assert!(!back.is_empty(), "everything was trimmed");
        // The newest survive: they are the ones most likely to explain
        // whatever made you open poptop, and dropping them to keep an hour-old
        // sample would be the wrong way round.
        assert_eq!(back.last().unwrap().cpu_total, 39.0);
        assert_eq!(
            back.first().unwrap().cpu_total,
            (40 - back.len()) as f32,
            "the surviving samples are not the newest contiguous run"
        );
    }

    #[test]
    fn a_missing_string_decodes_without_a_string_table_to_look_it_up_in() {
        // An absent string used to write a placeholder index that nothing put
        // in the table. Resolving it worked only because every process writes
        // its name and user first, so index 0 happened to exist by the time a
        // `None` was read — load-bearing coupling between fields with no reason
        // to know about each other. A record whose only string is optional and
        // absent had an empty table and no index 0 to find.
        //
        // The generic `Option` writes `T::default()` for the absent case, so
        // the placeholder is now a real interned empty string rather than a
        // fiction, and it is in the table because writing it is what put it
        // there. Nothing has to be written first.
        let mut out = Out::default();
        Codec::write(&None::<Arc<str>>, &mut out);
        let mut r = In::new(&out.bytes, 0, out.strings.clone());
        assert_eq!(
            <Option<Arc<str>>>::read_exact(&Registry::default(), &mut r),
            Some(None),
            "an absent string did not decode"
        );
    }

    /// A file with intact magic and version, whose body is `at` and nothing
    /// else, with the seconds set as high as the field goes.
    fn a_file_claiming_a_timestamp_of(secs: u64, nanos: u32) -> Vec<u8> {
        let mut f = Vec::new();
        f.extend_from_slice(MAGIC);
        f.extend_from_slice(&VERSION.to_le_bytes());
        f.extend_from_slice(&0u32.to_le_bytes()); // no strings
        f.extend_from_slice(&1u32.to_le_bytes()); // one sample
        f.extend_from_slice(&secs.to_le_bytes());
        f.extend_from_slice(&nanos.to_le_bytes());
        f
    }

    #[test]
    fn an_impossible_timestamp_empties_the_buffer_instead_of_killing_the_program() {
        // `at` is the first field of a sample, so this is the first thing read
        // out of a corrupt file — and a panic here is a crash on startup, from
        // a cache, which is the one place the program has no reason to fail.
        //
        // Two ways to overflow: the seconds alone, and a nanosecond carry that
        // tips a seconds count that would otherwise have fit. `Duration::new`
        // panics on the second and `UNIX_EPOCH + d` on the first.
        for (secs, nanos) in [
            (u64::MAX, 0),
            (u64::MAX, 999_999_999),
            (u64::MAX - 1, 999_999_999),
        ] {
            assert!(
                decode(&a_file_claiming_a_timestamp_of(secs, nanos)).is_none(),
                "a timestamp of {secs}s {nanos}ns was not refused"
            );
        }
    }

    #[test]
    fn no_single_corrupt_byte_can_panic_the_reader() {
        // The targeted test above covers the field this was found in. This
        // covers the ones nobody has thought of: every byte of a real store,
        // flipped, must still land in "not a store I understand" rather than
        // taking the process down.
        let mut s = sample_of(7.5, 2);
        s.procs.truncate(1);
        let good = encode(&[&s]);
        for i in 0..good.len() {
            for mask in [0xFF, 0x80, 0x01] {
                let mut bad = good.clone();
                bad[i] ^= mask;
                // The result is uninteresting — a flipped byte may still decode
                // to a valid, wrong sample. Not panicking is the assertion.
                let _ = decode(&bad);
            }
        }
    }

    /// A file written by a poptop that measures one thing more than this one.
    ///
    /// The schema gains a field and every body gains its value, which is
    /// exactly what a later release would produce.
    fn file_from_a_later_poptop(samples: &[&Sample]) -> Vec<u8> {
        use crate::persist::{Field, Typed};
        let mut mine = schemas();
        let sample = mine
            .iter_mut()
            .find(|(n, _)| *n == "Sample")
            .expect("no Sample");
        sample.1.push(Field {
            name: "cosmic_rays".into(),
            hash: <u64 as Typed>::HASH,
            ty: <u64 as Typed>::ty(),
        });
        // A second one whose type is a list of records, because that is the
        // hardest thing to step over without reading: the reader has to walk
        // the file's schema for that record type to know how wide each element
        // is. A scalar alone lets a broken `skip` pass.
        sample.1.push(Field {
            name: "gpus".into(),
            hash: <Option<Vec<DiskStat>> as Typed>::HASH,
            ty: <Option<Vec<DiskStat>> as Typed>::ty(),
        });

        let mut head = Out::default();
        head.schema_block(&mine);
        let mut body = Out::default();
        for s in samples {
            s.write(&mut body);
            Codec::write(&99u64, &mut body);
            Codec::write(
                &Some(vec![
                    DiskStat {
                        name: Arc::from("gpu0"),
                        ..DiskStat::default()
                    },
                    DiskStat {
                        name: Arc::from("gpu1"),
                        await_ms: Some(1.5),
                        ..DiskStat::default()
                    },
                ]),
                &mut body,
            );
        }

        let mut f = head.bytes;
        // MAGIC and VERSION go in front of the schema, as `encode` writes them.
        let mut file = MAGIC.to_vec();
        file.extend(VERSION.to_le_bytes());
        file.append(&mut f);
        file.extend((body.strings.len() as u32).to_le_bytes());
        for s in &body.strings {
            file.extend((s.len() as u32).to_le_bytes());
            file.extend(s.as_bytes());
        }
        file.extend((samples.len() as u32).to_le_bytes());
        file.extend(body.bytes);
        file
    }

    #[test]
    fn a_file_from_a_later_poptop_keeps_every_field_this_one_understands() {
        // The whole point of the schema block. A user who upgrades, downgrades,
        // and upgrades again keeps their history through all of it.
        // More than one sample, deliberately. The extra fields land at the end
        // of a sample's body, so with a single sample a `skip` that consumes
        // the wrong number of bytes has nothing left to corrupt and the test
        // passes anyway — which is exactly what happened the first time this
        // was written. The second sample is what makes the skip load-bearing.
        let all: Vec<Sample> = (0..3).map(|i| sample_of(37.5 + i as f32, 3)).collect();
        let refs: Vec<&Sample> = all.iter().collect();
        let (back, notes) = decode_reporting(&file_from_a_later_poptop(&refs));
        let back = back.expect("a file with two extra fields was refused");

        assert_eq!(back.len(), 3, "the samples did not survive");
        assert_eq!(
            back.iter().map(|s| s.cpu_total).collect::<Vec<_>>(),
            vec![37.5, 38.5, 39.5],
            "a sample after a skipped field was misread"
        );
        let got = &back[2];
        assert_eq!(got.cpu_total, 39.5, "a scalar after the schema was misread");
        assert_eq!(got.procs.len(), 3, "the process table was misread");
        assert_eq!(got.iowait, Some(61.25), "an optional was misread");
        assert_eq!(got.mem.total, 16 << 30, "a nested record was misread");
        assert_eq!(
            got.disks.as_ref().map(Vec::len),
            Some(2),
            "a list of nested records was misread"
        );
        assert_eq!(
            notes,
            vec![
                "the stored history has fields this poptop does not read: cosmic_rays, gpus"
                    .to_string()
            ],
            "the skipped field was not reported"
        );
    }

    #[test]
    fn a_field_whose_type_changed_is_skipped_and_said_out_loud() {
        // The case a downgrade hits after a metric changes precision. Reading
        // it as the current type would put a number in the column that was
        // never measured, so it is skipped — and skipping it silently would
        // empty the column with no explanation, which is the outcome the
        // schema block exists to prevent.
        use crate::persist::{Field, Typed};
        let s = sample_of(12.0, 2);
        let file = encode(&[&s]);

        // This build, but claiming `clock_ceiling` is an `Option<f64>`.
        let mut mine = schemas();
        let sample = mine
            .iter_mut()
            .find(|(n, _)| *n == "Sample")
            .expect("no Sample");
        let field = sample
            .1
            .iter_mut()
            .find(|f| &*f.name == "clock_ceiling")
            .expect("no clock_ceiling");
        *field = Field {
            name: "clock_ceiling".into(),
            hash: <Option<f64> as Typed>::HASH,
            ty: <Option<f64> as Typed>::ty(),
        };

        let mut notes = Vec::new();
        let back = read_file_as(&file, &mine, &mut notes).expect("the file was refused");
        assert_eq!(back.len(), 1, "the sample did not survive");
        assert_eq!(
            back[0].cpu_total, 12.0,
            "the field after the retyped one was misread"
        );
        assert_eq!(
            notes,
            vec![
                "the stored history measures a field differently than this poptop, \
                 so it was skipped: clock_ceiling"
                    .to_string()
            ],
            "the retyped field was not reported"
        );
    }

    #[test]
    fn a_file_that_declares_no_samples_says_so_rather_than_going_quiet() {
        // A schema that parses, passes validation, and describes nothing this
        // reader can start from. Without the note the read returns `None`,
        // `unwrap_or_default` turns it into an empty buffer, and the user is
        // told nothing about where their history went.
        let mut head = Out::default();
        head.schema_block(&[(
            "Elsewhere",
            vec![crate::persist::Field {
                name: "n".into(),
                hash: <u64 as crate::persist::Typed>::HASH,
                ty: <u64 as crate::persist::Typed>::ty(),
            }],
        )]);
        let mut file = MAGIC.to_vec();
        file.extend(VERSION.to_le_bytes());
        file.extend(head.bytes);
        file.extend(0u32.to_le_bytes()); // no strings
        file.extend(0u32.to_le_bytes()); // no samples

        let (back, notes) = decode_reporting(&file);
        assert!(back.is_none(), "a file with no Sample record was read");
        assert_eq!(
            notes,
            vec!["the stored history declares no samples; it was discarded".to_string()],
            "a file with nothing to read from went quiet"
        );
    }

    #[test]
    fn the_merge_path_and_the_fast_path_agree_on_a_real_store() {
        // The two tests above use a three-field record. This runs the merge
        // over the actual nineteen-field `Sample`, with its nested records,
        // its lists of nested records and its four hundred processes — and
        // asserts the slow path produces the same bytes as the fast one.
        let all: Vec<Sample> = (0..3).map(|i| sample_of(i as f32, 5)).collect();
        let refs: Vec<&Sample> = all.iter().collect();
        let file = encode(&refs);

        let fast = decode(&file).expect("the fast path failed");

        // A record this build claims to have and the file does not, so the
        // exactness check fails and every sample takes the merge.
        let mut mine = schemas();
        mine.push(("Ghost", Vec::new()));
        let mut notes = Vec::new();
        let merged = read_file_as(&file, &mine, &mut notes).expect("the merge path failed");

        let fast_refs: Vec<&Sample> = fast.iter().collect();
        let merged_refs: Vec<&Sample> = merged.iter().collect();
        assert_eq!(
            encode(&merged_refs),
            encode(&fast_refs),
            "the merge path read a different store than the fast path"
        );
    }

    #[test]
    fn a_file_from_before_the_schema_block_says_which_version_wrote_it() {
        let mut file = encode(&[&sample_of(1.0, 1)]);
        file[MAGIC.len()..MAGIC.len() + 4].copy_from_slice(&13u32.to_le_bytes());
        let (back, notes) = decode_reporting(&file);
        assert!(
            back.is_none(),
            "a file from before the schema block was read"
        );
        assert_eq!(
            notes,
            vec![format!(
                "the stored history is format 13, this poptop reads {VERSION}; discarded"
            )],
            "the version that wrote it was not named"
        );
    }

    #[test]
    fn every_reachable_record_has_a_schema() {
        // `records!` is a hand-written list, and a record reachable from
        // `Sample` but missing from it has no schema in the file and cannot be
        // read back. Walk the types rather than trusting the list.
        use crate::persist::Ty;
        fn walk(ty: &Ty, out: &mut Vec<String>) {
            match ty {
                Ty::Opt(i) | Ty::List(i) | Ty::Arr(i, _) => walk(i, out),
                Ty::Rec(n) => out.push(n.to_string()),
                _ => {}
            }
        }
        let all = schemas();
        let mut reachable = vec!["Sample".to_string()];
        let mut seen = 0;
        while seen < reachable.len() {
            let name = reachable[seen].clone();
            seen += 1;
            let fields = all
                .iter()
                .find(|(n, _)| *n == name)
                .unwrap_or_else(|| panic!("`{name}` is reachable but has no schema"))
                .1
                .clone();
            for f in &fields {
                let mut found = Vec::new();
                walk(&f.ty, &mut found);
                for n in found {
                    if !reachable.contains(&n) {
                        reachable.push(n);
                    }
                }
            }
        }
        assert_eq!(
            reachable.len(),
            all.len(),
            "the record list has entries nothing reaches, or the walk missed one: \
             reachable {reachable:?}"
        );
    }

    #[test]
    fn threads_survive_a_round_trip_and_cost_seventeen_bytes_each() {
        use crate::sample::ThreadSample;
        let mut s = sample_of(5.0, 2);
        s.tasks = Some(vec![
            ThreadSample {
                pid: 1,
                tid: 1,
                name: Arc::from("postgres"),
                state: 'S',
                cpu: 1.5,
            },
            ThreadSample {
                pid: 1,
                tid: 4098,
                name: Arc::from("bgwriter"),
                state: 'D',
                cpu: 0.0,
            },
        ]);
        let bytes = encode(&[&s]);
        let back = decode(&bytes).expect("did not decode");
        let got = back[0]
            .tasks
            .as_ref()
            .expect("the threads were not retained");
        assert_eq!(got.len(), 2, "a thread was lost");
        assert_eq!(got[1].tid, 4098);
        assert_eq!(&*got[1].name, "bgwriter", "a thread's own name was lost");
        assert_eq!(
            got[1].state, 'D',
            "a blocked thread came back as something else"
        );

        // `None` and an empty list are different answers: nobody asked, versus
        // asked and the box has no multi-threaded process.
        let mut none = sample_of(5.0, 2);
        none.tasks = None;
        assert!(
            decode(&encode(&[&none])).expect("did not decode")[0]
                .tasks
                .is_none(),
            "an uncollected thread list came back as an empty one"
        );

        // The retention cost, pinned rather than described: pid 4, tid 4,
        // an interned name 4, state 1, cpu 4. At four thousand threads that is
        // 68 KB a sample, which is why collecting them is a decision and not a
        // default.
        let mut wider = s.clone();
        let more: Vec<ThreadSample> = (0..10)
            .map(|i| ThreadSample {
                tid: 5000 + i,
                ..got[1].clone()
            })
            .collect();
        wider.tasks.as_mut().unwrap().extend(more);
        let per_thread = (encode(&[&wider]).len() - bytes.len()) / 10;
        assert_eq!(per_thread, 17, "a retained thread changed size");
    }

    #[test]
    fn a_process_state_survives_a_round_trip_and_costs_one_byte() {
        // Written as a byte rather than a full `char` scalar, which is worth a
        // test because the saving is the point: three bytes per process per
        // sample is 4% of the store at 400 processes.
        let mut s = sample_of(1.0, 0);
        s.procs = "RSDZTI"
            .chars()
            .map(|c| ProcSample {
                state: c,
                ..proc_of(1, "x")
            })
            .collect();
        let bytes = encode(&[&s]);
        let back = decode(&bytes).expect("did not decode");
        let got: String = back[0].procs.iter().map(|p| p.state).collect();
        assert_eq!(got, "RSDZTI", "the states did not survive");

        // The saving, asserted rather than described. Six more processes cost
        // six more records, and a record is:
        //
        //   pid 4, ppid 4, name 4, user 4, cpu 4, rss 8, threads 1+4,
        //   state 1, started 1+8, cmd 1+4, io 1+16, container 1+4,
        //   minflt 1+4, majflt 1+4, vsize 1+8, nice 1+4, pss 1+8  =  103
        //
        // Every optional costs its tag byte whether or not it is answered, so
        // `pss` occupies nine bytes on a machine that never reads it. That is
        // the price of a positional format, and the alternative — a length that
        // depends on the content — is one `skip` cannot step over.
        //
        // Names and users are interned, so a repeated one costs its index and
        // nothing else — which is why this is the marginal cost of a process
        // and not the cost of the first one.
        //
        // Pinning the total rather than a bound makes this the format's size
        // test: a `char` written as a `u32` scalar reads 68 here, and so does
        // any field that quietly grows.
        let mut wider = s.clone();
        wider.procs.extend(s.procs.iter().cloned());
        let per_proc = (encode(&[&wider]).len() - bytes.len()) / 6;
        assert_eq!(per_proc, 103, "a retained process changed size");
    }

    #[test]
    fn a_state_outside_ascii_decodes_as_unknown_rather_than_corrupting_the_row() {
        // No backend produces one — `status_char` maps everything it does not
        // recognise to `?` already. This pins what happens if one ever does,
        // because the alternative to a documented `?` is a byte that silently
        // becomes a different letter.
        let mut s = sample_of(1.0, 0);
        s.procs = vec![ProcSample {
            state: 'π',
            ..proc_of(1, "x")
        }];
        let back = decode(&encode(&[&s])).expect("did not decode");
        assert_eq!(back[0].procs[0].state, '?', "a wide state was not flagged");
    }

    #[test]
    fn a_command_line_survives_a_round_trip_and_a_missing_one_stays_missing() {
        // The two are different answers and the file has to keep them apart: a
        // kernel thread has no command line, and restoring it as an empty
        // string would put a blank where `[kworker/3:1]` belongs.
        let mut s = sample_of(1.0, 1);
        s.procs = vec![
            ProcSample {
                cmd: Some(Arc::from("node /srv/api/server.js --port 3000")),
                ..proc_of(10, "node")
            },
            ProcSample {
                cmd: None,
                ..proc_of(2, "[kworker/3:1]")
            },
        ];
        let back = decode(&encode(&[&s])).expect("did not decode");
        let got: Vec<Option<&str>> = back[0].procs.iter().map(|p| p.cmd.as_deref()).collect();
        assert_eq!(
            got,
            vec![Some("node /srv/api/server.js --port 3000"), None],
            "the command lines did not survive"
        );
    }

    #[test]
    fn a_repeated_command_line_is_written_once() {
        // Through the string table like every other string. Without it, a
        // Chrome renderer's arguments are copied into all six hundred retained
        // samples.
        let mut s = sample_of(1.0, 1);
        let long = "node /srv/api/server.js --port 3000 --cluster --inspect";
        s.procs = (0..5)
            .map(|i| ProcSample {
                cmd: Some(Arc::from(long)),
                ..proc_of(i, "node")
            })
            .collect();
        let bytes = encode(&[&s]);
        let count = bytes
            .windows(long.len())
            .filter(|w| *w == long.as_bytes())
            .count();
        assert_eq!(count, 1, "the command line was written {count} times");
    }

    #[test]
    fn the_string_table_holds_each_name_once() {
        // Names and users repeat across every process in every sample — the
        // same reason they are `Arc<str>` in memory. Without the table, five
        // hundred processes would write "postgres" five hundred times.
        let s = sample_of(1.0, 500);
        let bytes = encode(&[&s]);
        let count = bytes
            .windows(b"postgres".len())
            .filter(|w| *w == b"postgres")
            .count();
        assert_eq!(count, 1, "the name was written {count} times");
        let back = decode(&bytes).unwrap();
        assert_eq!(back[0].procs.len(), 500);
        assert!(back[0].procs.iter().all(|p| &*p.name == "postgres"));
    }

    #[test]
    fn the_store_lives_in_the_state_directory() {
        let os = |s: &str| Some(std::ffi::OsString::from(s));
        assert_eq!(
            path_from(None, os("/home/someone")),
            Some(PathBuf::from("/home/someone/.local/state/poptop/history"))
        );
        assert_eq!(
            path_from(os("/state"), os("/home/someone")),
            Some(PathBuf::from("/state/poptop/history"))
        );
        // Relative is the same hazard as everywhere else: it would put the
        // store wherever poptop happened to be launched from.
        assert_eq!(path_from(os("relative"), os("also/relative")), None);
        assert_eq!(path_from(None, None), None);
    }
}

#[cfg(test)]
mod cost {
    use super::tests_support::*;
    use super::*;

    #[test]
    #[ignore]
    fn show_startup_cost_with_a_full_store() {
        // The item asks for this measured. A full store is the whole ring
        // buffer at a realistic process count: 600 samples of 400 processes,
        // which is the default window at one sample a second.
        let all: Vec<Sample> = (0..600).map(|i| big_sample(i as f32, 400)).collect();
        let refs: Vec<&Sample> = all.iter().collect();

        let t0 = std::time::Instant::now();
        let bytes = encode(&refs);
        let write = t0.elapsed();

        let t0 = std::time::Instant::now();
        let back = decode(&bytes).expect("round trip");
        let read = t0.elapsed();

        println!(
            "{} samples x 400 procs: {:.1} MB, encode {write:?}, decode {read:?}, \
             schema {} bytes",
            back.len(),
            bytes.len() as f64 / 1e6,
            schema_bytes().len()
        );
    }
}

#[cfg(test)]
mod tests_support {
    use super::*;
    use crate::sample::{MemStat, ProcSample};

    /// A sample with distinct names, so the string table is exercised the way
    /// a real machine would rather than by one repeated word.
    pub fn big_sample(cpu: f32, procs: usize) -> Sample {
        let names = [
            "postgres", "nginx", "systemd", "node", "python3", "rustc", "cargo", "ssh",
        ];
        Sample {
            at: UNIX_EPOCH + Duration::from_secs(1_700_000_000),
            cpu_total: cpu,
            cpu_per_core: vec![1.0; 16],
            disks: None,
            clock_ceiling: None,
            pgin: None,
            pgout: None,
            swin: None,
            swout: None,
            oom_kills: None,
            pressure: None,
            net: None,
            filesystems: None,
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
            load: [1.0, 2.0, 3.0],
            tasks: None,
            exited: None,
            cgroups: None,
            procs: (0..procs)
                .map(|i| ProcSample {
                    pid: i as i32,
                    ppid: 1,
                    name: Arc::from(names[i % names.len()]),
                    user: Arc::from(if i % 3 == 0 { "root" } else { "oddurs" }),
                    cpu: 1.0,
                    rss: 1 << 20,
                    threads: Some(4),
                    state: 'S',
                    started: Some(i as u64),
                    cmd: None,
                    io: None,

                    container: None,
                    minflt: None,
                    majflt: None,
                    vsize: None,
                    nice: None,
                    pss: None,
                })
                .collect(),
            uptime: Duration::from_secs(90_000),
            forks: Some(1),
            io_supported: true,
            io_collected: false,
            io_denied: 0,
        }
    }
}

#[cfg(test)]
mod boot {
    use super::tests_support::*;
    use super::*;

    fn at_boot(boot: SystemTime, age: u64) -> Sample {
        let mut s = big_sample(1.0, 2);
        s.uptime = Duration::from_secs(3_600 - age);
        s.at = boot + s.uptime;
        s
    }

    #[test]
    fn a_boot_is_identified_without_collecting_anything_new() {
        // `at - uptime` is already in every sample on both platforms.
        let boot = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let s = at_boot(boot, 0);
        assert!(same_boot(boot_time(&s), boot));
    }

    #[test]
    fn samples_of_one_boot_agree_despite_whole_second_uptime() {
        // Uptime is whole seconds while `at` is not, so the derived instant
        // jitters between samples of the same boot. Without tolerance every
        // restore would discard everything.
        let boot = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let mut a = at_boot(boot, 0);
        a.at += Duration::from_millis(900);
        let b = at_boot(boot, 30);
        assert!(
            same_boot(boot_time(&a), boot_time(&b)),
            "two samples of one boot were read as different boots"
        );
    }

    #[test]
    fn a_reboot_is_not_mistaken_for_the_same_boot() {
        // The invariant this protects: `started` is ticks since boot, so
        // `(pid, started)` only identifies a process within one boot. Across a
        // reboot a live pid 1 matches a restored pid 1, and its history column
        // would render the previous boot's CPU as this process's own.
        let boot = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let later = boot + Duration::from_secs(86_400);
        assert!(!same_boot(boot, later));
        // Even a reboot a minute later is a different boot, because pid 1's
        // start time is near-identical every time.
        assert!(!same_boot(boot, boot + Duration::from_secs(60)));
    }

    #[test]
    fn the_cap_bounds_the_file_and_not_just_the_bodies() {
        // The doc comment calls it a ceiling on the file. Counting only the
        // sample bodies left the header and the whole string table outside it,
        // and the table is exactly the part that varies between machines.
        let all: Vec<Sample> = (0..40).map(|i| big_sample(i as f32, 30)).collect();
        let refs: Vec<&Sample> = all.iter().collect();
        let whole = encode_within(&refs, usize::MAX).len();
        for divisor in [2, 3, 4, 8] {
            let cap = whole / divisor;
            let bytes = encode_within(&refs, cap);
            assert!(
                bytes.len() <= cap,
                "asked for {cap} bytes, produced {}",
                bytes.len()
            );
            assert!(decode(&bytes).is_some(), "the trimmed file does not parse");
        }
    }
}
