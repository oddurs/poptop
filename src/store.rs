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

use crate::sample::{
    DiskStat, FsStat, IoRates, Link, MemStat, NetStat, Pressure, ProcSample, Sample, Stall,
};
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
const VERSION: u32 = 13;

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

/// A little-endian writer. Hand-rolled for the same reason the `/proc` parser
/// is: the format is a dozen scalars and a string table, and `serde` would be
/// the largest dependency in the project by an order of magnitude.
/// Bytes the file carries beyond the sample bodies: magic, version, and the
/// two counts.
const HEADER_BYTES: usize = MAGIC.len() + 4 + 4 + 4;

#[derive(Default)]
struct Out {
    bytes: Vec<u8>,
    /// What the string table will occupy, tracked as it grows so the trim loop
    /// can bound the *file* rather than the bodies. Counting only the bodies
    /// made the "ceiling on the file" in the doc comment untrue by the size of
    /// the table, which is exactly the part that varies between machines.
    table_bytes: usize,
    /// Names and users repeat across every process in every retained sample —
    /// the same reason they are `Arc<str>` in memory. Written once and
    /// referenced by index, or the file would be mostly repeated strings.
    strings: Vec<Arc<str>>,
    index: std::collections::HashMap<Arc<str>, u32>,
}

impl Out {
    fn u8(&mut self, v: u8) {
        self.bytes.push(v);
    }
    fn u32(&mut self, v: u32) {
        self.bytes.extend_from_slice(&v.to_le_bytes());
    }
    fn u64(&mut self, v: u64) {
        self.bytes.extend_from_slice(&v.to_le_bytes());
    }
    fn i32(&mut self, v: i32) {
        self.bytes.extend_from_slice(&v.to_le_bytes());
    }
    fn f32(&mut self, v: f32) {
        self.bytes.extend_from_slice(&v.to_le_bytes());
    }
    fn f64(&mut self, v: f64) {
        self.bytes.extend_from_slice(&v.to_le_bytes());
    }
    fn opt_f32(&mut self, v: Option<f32>) {
        self.u8(u8::from(v.is_some()));
        self.f32(v.unwrap_or(0.0));
    }
    fn opt_u32(&mut self, v: Option<u32>) {
        self.u8(u8::from(v.is_some()));
        self.u32(v.unwrap_or(0));
    }
    fn opt_str(&mut self, v: Option<&Arc<str>>) {
        self.u8(u8::from(v.is_some()));
        match v {
            Some(s) => self.str(s),
            // A placeholder, never looked up. Writing nothing would make the
            // record's length depend on its content, which every other optional
            // here avoids.
            None => self.u32(0),
        }
    }
    fn opt_u64(&mut self, v: Option<u64>) {
        // A tagged optional, because `None` and `0` are different answers
        // everywhere else in this codebase and the file must not collapse them.
        self.u8(u8::from(v.is_some()));
        self.u64(v.unwrap_or(0));
    }
    fn str(&mut self, s: &Arc<str>) {
        let id = match self.index.get(s) {
            Some(&id) => id,
            None => {
                let id = self.strings.len() as u32;
                self.table_bytes += 4 + s.len();
                self.strings.push(s.clone());
                self.index.insert(s.clone(), id);
                id
            }
        };
        self.u32(id);
    }
}

/// A bounds-checked reader. Every read can fail, so a truncated or corrupt
/// file produces `None` rather than a panic — it is a cache, and the worst
/// honest outcome is starting with an empty buffer.
struct In<'a> {
    bytes: &'a [u8],
    at: usize,
    strings: Vec<Arc<str>>,
}

impl<'a> In<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let end = self.at.checked_add(n)?;
        let out = self.bytes.get(self.at..end)?;
        self.at = end;
        Some(out)
    }
    fn u8(&mut self) -> Option<u8> {
        Some(self.take(1)?[0])
    }
    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }
    fn u64(&mut self) -> Option<u64> {
        Some(u64::from_le_bytes(self.take(8)?.try_into().ok()?))
    }
    fn i32(&mut self) -> Option<i32> {
        Some(i32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }
    fn f32(&mut self) -> Option<f32> {
        Some(f32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }
    fn f64(&mut self) -> Option<f64> {
        Some(f64::from_le_bytes(self.take(8)?.try_into().ok()?))
    }
    fn opt_f32(&mut self) -> Option<Option<f32>> {
        let present = self.u8()? != 0;
        let v = self.f32()?;
        Some(present.then_some(v))
    }
    fn opt_u32(&mut self) -> Option<Option<u32>> {
        let present = self.u8()? != 0;
        let v = self.u32()?;
        Some(present.then_some(v))
    }
    fn opt_str(&mut self) -> Option<Option<Arc<str>>> {
        let some = self.u8()? != 0;
        if !some {
            // Consume the placeholder without resolving it. Looking it up
            // instead worked only because `name` and `user` are written before
            // `cmd`, so index 0 always existed by the time a `None` was read —
            // load-bearing coupling between two fields that have no reason to
            // know about each other.
            self.u32()?;
            return Some(None);
        }
        Some(Some(self.str()?))
    }
    fn opt_u64(&mut self) -> Option<Option<u64>> {
        let present = self.u8()? != 0;
        let v = self.u64()?;
        Some(present.then_some(v))
    }
    fn str(&mut self) -> Option<Arc<str>> {
        let id = self.u32()? as usize;
        self.strings.get(id).cloned()
    }
}

/// Serialise samples, oldest first, dropping the oldest to fit [`MAX_BYTES`].
pub fn encode(samples: &[&Sample]) -> Vec<u8> {
    encode_within(samples, MAX_BYTES)
}

/// The cap as a parameter, so the trimming rule can be tested without building
/// sixty megabytes of fixture to provoke it.
fn encode_within(samples: &[&Sample], max_bytes: usize) -> Vec<u8> {
    // Written newest-first into the body and reversed at the end, so trimming
    // to fit drops the *oldest* — the opposite would throw away the samples
    // most likely to explain whatever made you open poptop.
    let mut kept: Vec<&Sample> = Vec::new();
    let mut out = Out::default();
    for sample in samples.iter().rev() {
        let before = out.bytes.len();
        let table_before = out.table_bytes;
        write_sample(&mut out, sample);
        if HEADER_BYTES + out.table_bytes + out.bytes.len() > max_bytes {
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
        write_sample(&mut out, sample);
    }

    let mut file = Vec::with_capacity(HEADER_BYTES + out.table_bytes + out.bytes.len());
    file.extend_from_slice(MAGIC);
    file.extend_from_slice(&VERSION.to_le_bytes());
    file.extend_from_slice(&(out.strings.len() as u32).to_le_bytes());
    for s in &out.strings {
        file.extend_from_slice(&(s.len() as u32).to_le_bytes());
        file.extend_from_slice(s.as_bytes());
    }
    file.extend_from_slice(&(kept.len() as u32).to_le_bytes());
    file.extend_from_slice(&out.bytes);
    file
}

fn write_sample(out: &mut Out, s: &Sample) {
    let since_epoch = s.at.duration_since(UNIX_EPOCH).unwrap_or_default();
    out.u64(since_epoch.as_secs());
    out.u32(since_epoch.subsec_nanos());
    out.f32(s.cpu_total);
    out.u32(s.cpu_per_core.len() as u32);
    for c in &s.cpu_per_core {
        out.f32(*c);
    }
    for v in [
        s.mem.total,
        s.mem.used,
        s.mem.available,
        s.mem.swap_total,
        s.mem.swap_used,
    ] {
        out.u64(v);
    }
    out.opt_u64(s.mem.free);
    for v in s.load {
        out.f64(v);
    }
    // Tagged, like every other optional here: a platform that cannot see a
    // figure and a platform that sees zero are different answers on disk too.
    out.opt_f32(s.iowait);
    out.opt_u32(s.running);
    out.opt_u32(s.blocked);
    out.u64(s.uptime.as_secs());
    out.opt_u64(s.forks);
    out.opt_f32(s.clock_ceiling);
    out.u8(u8::from(s.io_supported));
    out.u8(u8::from(s.io_collected));
    out.u64(s.io_denied as u64);
    out.u32(s.procs.len() as u32);
    for p in &s.procs {
        out.i32(p.pid);
        out.i32(p.ppid);
        out.str(&p.name);
        out.str(&p.user);
        out.f32(p.cpu);
        out.u64(p.rss);
        out.opt_u32(p.threads);
        out.u8(p.state as u8);
        out.opt_u64(p.started);
        // Through the string table like every other string, so the hundreds of
        // samples that retain the same process cost four bytes each rather than
        // a copy of its command line.
        out.opt_str(p.cmd.as_ref());
        out.u8(u8::from(p.io.is_some()));
        let io = p.io.unwrap_or_default();
        out.u64(io.read);
        out.u64(io.write);
    }
    // Tagged as a whole, then per device. `None` is "this platform does not
    // read disks"; an empty list is "it looked and found none that have ever
    // done IO". The restore has to keep those apart or a macOS buffer comes
    // back claiming the machine has no disks.
    out.u8(u8::from(s.disks.is_some()));
    let disks = s.disks.as_deref().unwrap_or_default();
    out.u32(disks.len() as u32);
    for d in disks {
        out.str(&d.name);
        out.u64(d.read);
        out.u64(d.write);
        out.u64(d.reads);
        out.u64(d.writes);
        out.f32(d.util);
        out.opt_f32(d.await_ms);
        out.f32(d.queue);
    }
    // Tagged, because a kernel that does not publish pressure and one reporting
    // a machine that never stalled are opposite answers.
    out.u8(u8::from(s.pressure.is_some()));
    let p = s.pressure.unwrap_or_default();
    for stall in [p.cpu, p.io, p.memory] {
        out.f32(stall.some);
        out.f32(stall.full);
    }

    out.u8(u8::from(s.net.is_some()));
    // Borrowed, not cloned: `write_sample` only reads it, and a deep copy of
    // the link vector per sample is an allocation per sample per persist.
    let empty = NetStat::default();
    let net = s.net.as_ref().unwrap_or(&empty);
    out.u32(net.links.len() as u32);
    for l in &net.links {
        out.str(&l.name);
        out.u64(l.rx);
        out.u64(l.tx);
        out.u64(l.rx_packets);
        out.u64(l.tx_packets);
    }
    out.opt_u64(net.errors);
    out.opt_u64(net.drops);
    out.opt_u64(net.retrans);
    out.opt_u64(net.listen_drops);

    out.u8(u8::from(s.filesystems.is_some()));
    let fs = s.filesystems.as_deref().unwrap_or_default();
    out.u32(fs.len() as u32);
    for f in fs {
        out.str(&f.mount);
        out.u64(f.total);
        out.u64(f.avail);
    }
}

/// Parse a store, or `None` if it is not one this version understands.
///
/// Every failure mode lands here: wrong magic, wrong version, truncation,
/// corruption. All produce `None`, because the file is a cache of something
/// the machine will produce again within minutes — refusing to start over it,
/// or worse guessing at it, would both be wrong.
pub fn decode(bytes: &[u8]) -> Option<Vec<Sample>> {
    let mut r = In {
        bytes,
        at: 0,
        strings: Vec::new(),
    };
    if r.take(MAGIC.len())? != MAGIC || r.u32()? != VERSION {
        return None;
    }
    let n_strings = r.u32()?;
    let mut strings = Vec::with_capacity(n_strings.min(1 << 20) as usize);
    for _ in 0..n_strings {
        let len = r.u32()? as usize;
        strings.push(Arc::from(std::str::from_utf8(r.take(len)?).ok()?));
    }
    r.strings = strings;

    let n_samples = r.u32()?;
    let mut samples = Vec::with_capacity(n_samples.min(1 << 20) as usize);
    for _ in 0..n_samples {
        samples.push(read_sample(&mut r)?);
    }
    Some(samples)
}

fn read_sample(r: &mut In<'_>) -> Option<Sample> {
    let at = UNIX_EPOCH + Duration::new(r.u64()?, r.u32()?);
    let cpu_total = r.f32()?;
    let cores = r.u32()? as usize;
    // Bounded before allocating: a corrupt length must not be an allocation
    // request. Every count in this format is checked the same way.
    let mut cpu_per_core = Vec::with_capacity(cores.min(4096));
    for _ in 0..cores {
        cpu_per_core.push(r.f32()?);
    }
    let mem = MemStat {
        total: r.u64()?,
        used: r.u64()?,
        available: r.u64()?,
        swap_total: r.u64()?,
        swap_used: r.u64()?,
        free: r.opt_u64()?,
    };
    let load = [r.f64()?, r.f64()?, r.f64()?];
    let iowait = r.opt_f32()?;
    let running = r.opt_u32()?;
    let blocked = r.opt_u32()?;
    let uptime = Duration::from_secs(r.u64()?);
    let forks = r.opt_u64()?;
    let clock_ceiling = r.opt_f32()?;
    let io_supported = r.u8()? != 0;
    let io_collected = r.u8()? != 0;
    let io_denied = r.u64()? as usize;
    let n_procs = r.u32()? as usize;
    let mut procs = Vec::with_capacity(n_procs.min(1 << 16));
    for _ in 0..n_procs {
        let pid = r.i32()?;
        let ppid = r.i32()?;
        let name = r.str()?;
        let user = r.str()?;
        let cpu = r.f32()?;
        let rss = r.u64()?;
        let threads = r.opt_u32()?;
        let state = r.u8()? as char;
        let started = r.opt_u64()?;
        let cmd = r.opt_str()?;
        let has_io = r.u8()? != 0;
        let read = r.u64()?;
        let write = r.u64()?;
        procs.push(ProcSample {
            pid,
            ppid,
            name,
            user,
            cpu,
            rss,
            threads,
            state,
            started,
            cmd,
            io: has_io.then_some(IoRates { read, write }),
        });
    }
    let has_disks = r.u8()? != 0;
    let n_disks = r.u32()? as usize;
    let mut disks = Vec::with_capacity(n_disks.min(1 << 10));
    for _ in 0..n_disks {
        disks.push(DiskStat {
            name: r.str()?,
            read: r.u64()?,
            write: r.u64()?,
            reads: r.u64()?,
            writes: r.u64()?,
            util: r.f32()?,
            await_ms: r.opt_f32()?,
            queue: r.f32()?,
        });
    }
    let has_pressure = r.u8()? != 0;
    let mut stalls = [Stall::default(); 3];
    for stall in &mut stalls {
        stall.some = r.f32()?;
        stall.full = r.f32()?;
    }
    let has_net = r.u8()? != 0;
    let n_links = r.u32()? as usize;
    let mut links = Vec::with_capacity(n_links.min(1 << 10));
    for _ in 0..n_links {
        links.push(Link {
            name: r.str()?,
            rx: r.u64()?,
            tx: r.u64()?,
            rx_packets: r.u64()?,
            tx_packets: r.u64()?,
        });
    }
    let net = NetStat {
        links,
        errors: r.opt_u64()?,
        drops: r.opt_u64()?,
        retrans: r.opt_u64()?,
        listen_drops: r.opt_u64()?,
    };
    let has_fs = r.u8()? != 0;
    let n_fs = r.u32()? as usize;
    let mut filesystems = Vec::with_capacity(n_fs.min(1 << 10));
    for _ in 0..n_fs {
        filesystems.push(FsStat {
            mount: r.str()?,
            total: r.u64()?,
            avail: r.u64()?,
        });
    }
    Some(Sample {
        at,
        cpu_total,
        cpu_per_core,
        iowait,
        running,
        blocked,
        mem,
        load,
        procs,
        uptime,
        forks,
        clock_ceiling,
        io_supported,
        io_collected,
        io_denied,
        disks: has_disks.then_some(disks),
        net: has_net.then_some(net),
        filesystems: has_fs.then_some(filesystems),
        pressure: has_pressure.then(|| Pressure {
            cpu: stalls[0],
            io: stalls[1],
            memory: stalls[2],
        }),
    })
}

/// Read the store, if there is one and it is readable.
pub fn load() -> Option<Vec<Sample>> {
    decode(&std::fs::read(path()?).ok()?)
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
            running: Some(3),
            blocked: Some(17),
            mem: MemStat {
                total: 16 << 30,
                used: 8 << 30,
                available: 8 << 30,
                free: Some(5 << 30),
                swap_total: 2 << 30,
                swap_used: 1 << 30,
            },
            load: [1.5, 2.5, 3.5],
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
        // `opt_str` writes a placeholder index for `None`, and resolving it
        // worked only because every process writes its name and user first. A
        // record whose only string is optional and absent has an empty table,
        // and there is no index 0 to find.
        let mut out = Out::default();
        out.opt_str(None);
        let mut r = In {
            bytes: &out.bytes,
            at: 0,
            strings: Vec::new(),
        };
        assert_eq!(r.opt_str(), Some(None), "an absent string did not decode");
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
            "{} samples x 400 procs: {:.1} MB, encode {write:?}, decode {read:?}",
            back.len(),
            bytes.len() as f64 / 1e6
        );
    }
}

#[cfg(test)]
mod tests_support {
    use super::*;

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
            pressure: None,
            net: None,
            filesystems: None,
            iowait: None,
            running: None,
            blocked: None,
            mem: MemStat::default(),
            load: [1.0, 2.0, 3.0],
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
