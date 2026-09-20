//! Processes that lived and died between two samples.
//!
//! poptop reads `/proc` once a second, so a process that lives 200ms never
//! existed as far as it is concerned. That is not an edge case: a burst of
//! short-lived processes is one of the commonest causes of exactly the spike
//! poptop is opened to explain, and scrubbing back to it showed a process table
//! that could not account for the graph above it.
//!
//! The kernel will tell you. `taskstats` emits a record for every task that
//! exits, over generic netlink, to any socket registered as a listener — which
//! is how atop reports "resource consumption by those processes that have
//! finished during the interval".
//!
//! # What it takes, and what each requirement actually is
//!
//! Registration is refused unless all three hold, and the failures are
//! distinguishable, which matters because the remedies are different:
//!
//! - **`CAP_NET_ADMIN`**, or `EPERM`. Root, or the capability.
//! - **The initial PID namespace.** `add_del_listener` refuses anything else
//!   with `EINVAL`. A container is by definition not it.
//! - **The initial *network* namespace**, which is the one that is easy to
//!   miss: registration succeeds anywhere, but delivery is
//!   `genlmsg_unicast(&init_net, ...)`, so records go to a port in the initial
//!   net namespace and a listener in its own never hears anything. It looks
//!   exactly like a kernel that does not implement the feature.
//!
//! Verified against a live kernel rather than reasoned about: with all three,
//! 41 exit records arrived in eight seconds for processes that lived 224 to 924
//! microseconds.

use super::at;
use crate::sample::{IoRates, ProcSample};
use std::io;
use std::sync::Arc;

const AF_NETLINK: i32 = 16;
const SOCK_RAW: i32 = 3;
const NETLINK_GENERIC: i32 = 16;
const GENL_ID_CTRL: u16 = 16;
const CTRL_CMD_GETFAMILY: u8 = 3;
const CTRL_ATTR_FAMILY_ID: u16 = 1;
const CTRL_ATTR_FAMILY_NAME: u16 = 2;
const TASKSTATS_CMD_GET: u8 = 1;
const REGISTER_CPUMASK: u16 = 3;
const NLM_F_REQUEST: u16 = 1;
const NLM_F_ACK: u16 = 4;
const NLMSG_ERROR: u16 = 2;
const SOL_SOCKET: i32 = 1;
const SO_RCVBUF: i32 = 8;
/// Sets the receive buffer past `net.core.rmem_max`. Needs `CAP_NET_ADMIN`,
/// which registering as a listener needed anyway.
const SO_RCVBUFFORCE: i32 = 33;
const EAGAIN: i32 = 11;
const ENOBUFS: i32 = 105;
const MSG_DONTWAIT: i32 = 0x40;
const AGGR_PID: u16 = 4;
const AGGR_TGID: u16 = 5;
const NLMSG_DONE: u16 = 3;
const TYPE_PID: u16 = 1;
const TYPE_TGID: u16 = 2;
const TYPE_STATS: u16 = 3;

/// Our own sequence number for the registration, so its acknowledgement is not
/// confused with the family lookup's.
///
/// Not cosmetic. Asking for an ack on the lookup makes it produce a reply *and*
/// an ack, so a reader that takes the next message as the registration's answer
/// is off by one from then on — and reads the lookup's `err 0` as "registered".
/// That mistake reported success on a kernel that had refused, twice, before it
/// was caught by printing the echoed sequence number.
const SEQ_REGISTER: u32 = 0x707470;

// Checked against glibc's headers on x86_64 and aarch64: `socklen_t` is four
// bytes, `ssize_t` is `isize`, and `struct sockaddr_nl` is the twelve bytes
// `open` builds by hand. The constants above match `<linux/netlink.h>`,
// `<linux/genetlink.h>` and `<linux/taskstats.h>` on both.
unsafe extern "C" {
    fn socket(domain: i32, ty: i32, proto: i32) -> i32;
    fn bind(fd: i32, addr: *const u8, len: u32) -> i32;
    fn send(fd: i32, buf: *const u8, len: usize, flags: i32) -> isize;
    fn recv(fd: i32, buf: *mut u8, len: usize, flags: i32) -> isize;
    fn setsockopt(fd: i32, level: i32, name: i32, val: *const u8, len: u32) -> i32;
    fn close(fd: i32) -> i32;
}

/// Why exit records are not being collected.
///
/// Kept apart rather than collapsed into one "unavailable", because a reader
/// can act on two of these and not on the third, and telling them apart is the
/// difference between a useful message and a shrug.
#[derive(Debug)]
pub enum Unavailable {
    /// Needs `CAP_NET_ADMIN`. Running as root would fix it.
    Permission,
    /// Not the initial PID or user namespace — a container. Nothing the user
    /// can do from inside it.
    Namespace,
    /// The kernel has no `TASKSTATS` family: built without `CONFIG_TASKSTATS`.
    NoFamily,
    Other(io::Error),
}

impl Unavailable {
    /// One line for the panel, saying what would change it.
    pub fn why(&self) -> String {
        match self {
            Unavailable::Permission => {
                "exited processes need CAP_NET_ADMIN; run as root to see them".into()
            }
            Unavailable::Namespace => {
                "exited processes are not visible from a container: the kernel \
                 registers exit listeners only in the initial namespace"
                    .into()
            }
            Unavailable::NoFamily => {
                "this kernel has no taskstats family, so exited processes cannot be seen".into()
            }
            Unavailable::Other(e) => format!("exited processes are unavailable: {e}"),
        }
    }
}

/// A registered exit listener.
pub struct Listener {
    fd: i32,
    buf: Vec<u8>,
}

impl Drop for Listener {
    fn drop(&mut self) {
        // Deregistration is implicit: the kernel drops the listener when the
        // socket closes.
        // SAFETY: `fd` is the socket `open` created, owned by this value alone
        // and closed only here, so no other descriptor can be closed by it.
        unsafe { close(self.fd) };
    }
}

fn align(n: usize) -> usize {
    (n + 3) & !3
}

/// One generic-netlink request with a single attribute.
fn request(family: u16, cmd: u8, attr: u16, payload: &[u8], seq: u32) -> Vec<u8> {
    let attr_len = 4 + payload.len();
    let total = 20 + align(attr_len);
    let mut m = Vec::with_capacity(total);
    m.extend((total as u32).to_ne_bytes());
    m.extend(family.to_ne_bytes());
    m.extend((NLM_F_REQUEST | NLM_F_ACK).to_ne_bytes());
    m.extend(seq.to_ne_bytes());
    m.extend(0u32.to_ne_bytes());
    m.push(cmd);
    m.push(1); // TASKSTATS_GENL_VERSION
    m.extend(0u16.to_ne_bytes());
    m.extend((attr_len as u16).to_ne_bytes());
    m.extend(attr.to_ne_bytes());
    m.extend(payload);
    m.resize(total, 0);
    m
}

/// Walk the attributes of a netlink message body, calling `f` with each.
fn attrs<'a>(body: &'a [u8], mut f: impl FnMut(u16, &'a [u8])) {
    let mut at = 0;
    while at + 4 <= body.len() {
        let len = u16::from_ne_bytes([body[at], body[at + 1]]) as usize;
        let ty = u16::from_ne_bytes([body[at + 2], body[at + 3]]);
        if len < 4 || at + len > body.len() {
            break;
        }
        f(ty, &body[at + 4..at + len]);
        at += align(len);
    }
}

impl Listener {
    /// Register for exit records, or say why not.
    pub fn open() -> Result<Listener, Unavailable> {
        // SAFETY: three integers in, a descriptor or -1 out; nothing of ours
        // is touched.
        let fd = unsafe { socket(AF_NETLINK, SOCK_RAW, NETLINK_GENERIC) };
        if fd < 0 {
            return Err(classify(io::Error::last_os_error()));
        }
        // Owned from here, so every early return below closes it.
        let mut me = Listener {
            fd,
            buf: vec![0; 64 << 10],
        };

        // `struct sockaddr_nl`: family, two bytes of padding, port id, groups.
        // All zero but the family asks the kernel to choose the port.
        let mut addr = [0u8; 12];
        addr[0..2].copy_from_slice(&(AF_NETLINK as u16).to_ne_bytes());
        // SAFETY: `addr` is twelve initialised bytes and the length says
        // twelve; the kernel reads it and keeps nothing.
        if unsafe { bind(fd, addr.as_ptr(), addr.len() as u32) } < 0 {
            return Err(classify(io::Error::last_os_error()));
        }
        // Exit records arrive in bursts — that is the whole point — and a
        // small socket buffer turns a burst into a hole. Measured: 20,000
        // exits in one second overran a 4 MB buffer and the kernel dropped
        // every record of the burst this feature exists to catch.
        //
        // `SO_RCVBUFFORCE` first, because plain `SO_RCVBUF` is silently
        // clamped to `net.core.rmem_max`. Falls back for a kernel that
        // refuses it.
        let size: i32 = 16 << 20;
        let ptr = (&raw const size).cast::<u8>();
        let len = size_of::<i32>() as u32;
        // SAFETY: `ptr` points at a live `i32` for the length given, and the
        // kernel copies it before returning.
        if unsafe { setsockopt(fd, SOL_SOCKET, SO_RCVBUFFORCE, ptr, len) } < 0 {
            // SAFETY: as above.
            unsafe { setsockopt(fd, SOL_SOCKET, SO_RCVBUF, ptr, len) };
        }

        let family = me.family()?;
        me.register(family)?;
        Ok(me)
    }

    /// Resolve the `TASKSTATS` family id.
    fn family(&mut self) -> Result<u16, Unavailable> {
        let mut name = b"TASKSTATS".to_vec();
        name.push(0);
        let m = request(
            GENL_ID_CTRL,
            CTRL_CMD_GETFAMILY,
            CTRL_ATTR_FAMILY_NAME,
            &name,
            1,
        );
        self.send(&m)?;
        let n = self.blocking_read()?;
        family_in(&self.buf[..n])
    }

    /// Ask for every CPU's exits.
    fn register(&mut self, family: u16) -> Result<(), Unavailable> {
        // `/sys/devices/system/cpu/possible`, not `available_parallelism`. The
        // latter reports the CPUs *this process* may run on — an affinity mask
        // or a cgroup quota narrows it — while the kernel registers listeners
        // per CPU of the machine. Under a quota of two on a sixty-four core
        // box the mask would be `0-1`, and exits on the other sixty-two would
        // never be delivered: a table showing a fraction of what happened,
        // with nothing saying so.
        let mut mask = possible_cpus().into_bytes();
        mask.push(0);
        let m = request(
            family,
            TASKSTATS_CMD_GET,
            REGISTER_CPUMASK,
            &mask,
            SEQ_REGISTER,
        );
        self.send(&m)?;

        // Drain until the reply that echoes our own sequence number. See
        // `SEQ_REGISTER`.
        for _ in 0..8 {
            let n = self.blocking_read()?;
            if let Some(answer) = registration_in(&self.buf[..n]) {
                return answer;
            }
        }
        Err(Unavailable::Other(io::Error::other(
            "no answer to the listener registration",
        )))
    }

    fn send(&self, m: &[u8]) -> Result<(), Unavailable> {
        // SAFETY: `m` is a live slice and its own length is passed; the kernel
        // copies it out and keeps no pointer.
        let sent = unsafe { send(self.fd, m.as_ptr(), m.len(), 0) };
        if sent < 0 {
            return Err(classify(io::Error::last_os_error()));
        }
        Ok(())
    }

    fn blocking_read(&mut self) -> Result<usize, Unavailable> {
        // SAFETY: the kernel writes at most `buf.len()` bytes into `buf`, which
        // is initialised and exclusively borrowed for the call. Without
        // `MSG_TRUNC` it returns how many it wrote, never the datagram's full
        // length, so `n <= buf.len()` — clamped anyway, since every slice
        // taken from here on is `buf[..n]`.
        let n = unsafe { recv(self.fd, self.buf.as_mut_ptr(), self.buf.len(), 0) };
        if n < 0 {
            return Err(classify(io::Error::last_os_error()));
        }
        Ok((n as usize).min(self.buf.len()))
    }

    /// Everything that has exited since the last call.
    ///
    /// Never blocks: whatever the kernel has queued is what this interval saw,
    /// and waiting for more would be waiting for the future.
    pub fn drain(
        &mut self,
        elapsed_secs: f64,
        boot: Boot,
        before: &std::collections::HashMap<i32, u64>,
        users: &mut UserCache,
    ) -> Vec<ProcSample> {
        let mut out = Vec::new();
        loop {
            // SAFETY: as `blocking_read`.
            let n = unsafe { recv(self.fd, self.buf.as_mut_ptr(), self.buf.len(), MSG_DONTWAIT) };
            if n < 0 {
                let e = io::Error::last_os_error().raw_os_error().unwrap_or(0);
                // `ENOBUFS` is not end-of-stream. It means the kernel gave up
                // queueing for us and dropped an unknown number of records —
                // and there is very often more waiting behind it. Treating it
                // as "nothing left" threw away every record of a 20,000-process
                // burst, which is the case this exists for.
                if e == ENOBUFS {
                    // Not counted here. The panel's `N came and went` figure is
                    // the reconciliation and it is exact: the kernel's own
                    // count of task creations, minus what the table can account
                    // for. A record dropped on this socket simply stays on the
                    // unaccounted side of it, which is where it belongs — a
                    // second counter would be a second, less reliable way of
                    // saying the same thing.
                    continue;
                }
                let _ = EAGAIN;
                break;
            }
            if n == 0 {
                break;
            }
            let n = (n as usize).min(self.buf.len());
            out.extend(exits_in(&self.buf[..n], elapsed_secs, boot, before, users));
        }
        out
    }
}

/// Each netlink message in a datagram, as `(type, sequence, payload)`, where the
/// payload is what follows the sixteen-byte header.
///
/// Taken by each message's own `nlmsg_len`, and ended at the first one whose
/// length is shorter than a header or runs past what was read: every index into
/// the buffer after this is inside a length the kernel stated and the read
/// confirmed.
fn messages(buf: &[u8]) -> impl Iterator<Item = (u16, u32, &[u8])> {
    let mut at = 0usize;
    std::iter::from_fn(move || {
        let head = buf.get(at..at.checked_add(16)?)?;
        let len = u32::from_ne_bytes(head[0..4].try_into().ok()?) as usize;
        if len < 16 {
            return None;
        }
        let whole = buf.get(at..at.checked_add(len)?)?;
        at = at.checked_add(align(len))?;
        let ty = u16::from_ne_bytes([head[4], head[5]]);
        let seq = u32::from_ne_bytes(head[8..12].try_into().ok()?);
        Some((ty, seq, &whole[16..]))
    })
}

/// The kernel's `errno` in an `NLMSG_ERROR` payload, which is `0` for an ack.
///
/// `None` for `i32::MIN`, which is no errno and has no negation.
fn error_of(payload: &[u8]) -> Option<i32> {
    i32::from_ne_bytes(payload.get(0..4)?.try_into().ok()?).checked_neg()
}

/// The `TASKSTATS` family id out of the reply to `CTRL_CMD_GETFAMILY`.
///
/// Split from the socket so the reply can be handed bytes the kernel did not
/// write. An error in place of the reply is the kernel's own reason, not a
/// payload to go looking for attributes in.
fn family_in(buf: &[u8]) -> Result<u16, Unavailable> {
    for (ty, _, payload) in messages(buf) {
        if ty == NLMSG_ERROR {
            match error_of(payload) {
                // An ack, which the request asked for. Not the answer.
                Some(0) => continue,
                // `ENOENT`: the controller has no family by that name.
                Some(2) | None => return Err(Unavailable::NoFamily),
                Some(e) => return Err(classify(io::Error::from_raw_os_error(e))),
            }
        }
        // The four-byte generic-netlink header, then the attributes.
        let Some(body) = payload.get(4..) else {
            continue;
        };
        let mut id = 0u16;
        attrs(body, |ty, v| {
            if ty == CTRL_ATTR_FAMILY_ID && v.len() >= 2 {
                id = u16::from_ne_bytes([v[0], v[1]]);
            }
        });
        if id != 0 {
            return Ok(id);
        }
    }
    Err(Unavailable::NoFamily)
}

/// The answer to the listener registration, if this datagram holds it.
///
/// `None` for a datagram that is about something else — see `SEQ_REGISTER` for
/// why there is one to skip.
fn registration_in(buf: &[u8]) -> Option<Result<(), Unavailable>> {
    let (ty, _, payload) = messages(buf).find(|&(_, seq, _)| seq == SEQ_REGISTER)?;
    if ty != NLMSG_ERROR {
        return Some(Ok(()));
    }
    Some(match error_of(payload)? {
        0 => Ok(()),
        1 | 13 => Err(Unavailable::Permission),
        // The namespace gate returns this for a mask the kernel has
        // already parsed successfully — the boundary sits exactly at
        // `nr_cpu_ids`, so a mask wider than the machine gives `ERANGE`
        // and a valid one from the wrong namespace gives `EINVAL`.
        22 => Err(Unavailable::Namespace),
        e => Err(Unavailable::Other(io::Error::from_raw_os_error(e))),
    })
}

/// The exit records in one datagram from the taskstats socket.
///
/// Split from [`Listener::drain`] so the bytes can be tested — and fuzzed —
/// without a socket: they come from the kernel, and every length in them is one
/// this code trusts to index with.
pub fn exits_in(
    buf: &[u8],
    elapsed_secs: f64,
    boot: Boot,
    before: &std::collections::HashMap<i32, u64>,
    users: &mut UserCache,
) -> Vec<ProcSample> {
    let mut out = Vec::new();
    // A datagram can carry more than one netlink message. Bounding the
    // body by the whole read treats the next message's header as
    // attributes of this one, so each message is taken by its own
    // `nlmsg_len`.
    let mut at = 0;
    while at + 20 <= buf.len() {
        let len = u32::from_ne_bytes(buf[at..at + 4].try_into().unwrap_or([0; 4])) as usize;
        if len < 20 || at + len > buf.len() {
            break;
        }
        let ty = u16::from_ne_bytes([buf[at + 4], buf[at + 5]]);
        if ty == NLMSG_ERROR || ty == NLMSG_DONE {
            at += align(len);
            continue;
        }
        // Copied because `attrs` borrows the buffer while the closure
        // needs the user cache, and the next read overwrites it anyway.
        let body = buf[at + 20..at + len].to_vec();
        at += align(len);

        // A whole-group record when there is one, and the per-task
        // record only otherwise. The kernel sends `AGGR_PID` for every
        // *task* that exits and adds `AGGR_TGID` when the whole thread
        // group is going — so taking `AGGR_PID` unconditionally put a
        // dying process in the table twice, once as itself and once as
        // its leader thread.
        let mut best: Option<ProcSample> = None;
        for want in [AGGR_TGID, AGGR_PID] {
            if best.is_some() {
                break;
            }
            attrs(&body, |ty, v| {
                if ty != want || best.is_some() {
                    return;
                }
                let mut pid = 0i32;
                let mut stats: Option<&[u8]> = None;
                attrs(v, |ity, iv| match ity {
                    TYPE_PID | TYPE_TGID if iv.len() >= 4 => {
                        pid = i32::from_ne_bytes(iv[0..4].try_into().unwrap_or([0; 4]));
                    }
                    TYPE_STATS => stats = Some(iv),
                    _ => {}
                });
                if let Some(s) = stats {
                    let was = before.get(&pid).copied().unwrap_or(0);
                    best = parse_exit(pid, s, elapsed_secs, boot, was, users);
                }
            });
        }
        if let Some(p) = best {
            out.push(p);
        }
    }
    out
}

/// Resolve a uid to a name, reusing the collector's cache.
pub type UserCache = std::collections::HashMap<u32, Arc<str>>;

/// What is needed to put an exit record on the same clock as a live row.
#[derive(Clone, Copy, Debug)]
pub struct Boot {
    /// `btime` from `/proc/stat`: the epoch second the machine booted.
    pub epoch_secs: u64,
    pub ticks_per_sec: f64,
}

impl Boot {
    /// An epoch second, as clock ticks since boot.
    fn ticks_since_boot(self, epoch_secs: u64) -> u64 {
        let since = epoch_secs.saturating_sub(self.epoch_secs);
        (since as f64 * self.ticks_per_sec) as u64
    }
}

/// Every CPU the kernel knows about, as the string the cpumask parser wants.
fn possible_cpus() -> String {
    std::fs::read_to_string(at("/sys/devices/system/cpu/possible"))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        // A machine that publishes no such file is old enough that one CPU is
        // a safer guess than a range nobody can check.
        .unwrap_or_else(|| "0".to_string())
}

fn u64at(s: &[u8], o: usize) -> Option<u64> {
    Some(u64::from_ne_bytes(s.get(o..o + 8)?.try_into().ok()?))
}
fn u32at(s: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_ne_bytes(s.get(o..o + 4)?.try_into().ok()?))
}

/// One `struct taskstats`, as a process row.
///
/// The offsets are the struct's, which is stable ABI — it is versioned and only
/// ever appended to, and the version is the first field. Read by offset rather
/// than by a `#[repr(C)]` mirror because a mirror would have to be re-derived
/// for every kernel that adds a field, and getting one wrong is silent.
///
/// Split from the read so it can be tested against a fixture, like every other
/// parser here.
pub fn parse_exit(
    pid: i32,
    s: &[u8],
    elapsed_secs: f64,
    boot: Boot,
    // Clock ticks of CPU this pid had used at the previous sample, if it was
    // alive then. Zero for one born inside this interval, which is the truth
    // rather than a default.
    before_ticks: u64,
    users: &mut UserCache,
) -> Option<ProcSample> {
    // Anything older than the version that fixed the field order is not worth
    // guessing at.
    if u16::from_ne_bytes([*s.first()?, *s.get(1)?]) < 8 {
        return None;
    }
    let uid = u32at(s, 120)?;
    let ppid = u32at(s, 132)? as i32;
    let btime = u32at(s, 136)? as u64;
    let etime_us = u64at(s, 144)?;
    let utime = u64at(s, 152)?;
    let stime = u64at(s, 160)?;
    // Peak, not average. `coremem` is an MB-microsecond integral that reads
    // zero for anything short-lived, which is exactly what this exists to
    // catch; `hiwater_rss` is the high-water mark in kilobytes and is populated
    // even for a process that lived 300us.
    let hiwater_rss_kb = u64at(s, 200)?;

    let comm = s.get(80..112)?;
    let comm = comm.split(|b| *b == 0).next().unwrap_or(comm);
    let name: Arc<str> = Arc::from(String::from_utf8_lossy(comm).as_ref());

    let user = users
        .entry(uid)
        .or_insert_with(|| Arc::from(uid.to_string().as_str()))
        .clone();

    // CPU used *during this interval*, which is what every live row shows.
    //
    // `ac_utime`/`ac_stime` are the process's whole life, so dividing by the
    // interval reports a process that ran for three hours at 50% and exited
    // here as 540,000,000% — the same objection this file makes two fields down
    // about the byte totals, and it applies just as much to CPU.
    //
    // Dividing by the process's own lifetime instead is a real quantity, and
    // wrong here for a different reason: `ac_utime` is tick-quantised, so a
    // process that lived 300us and was charged one 10ms tick reads 3,333%. A
    // `/bin/true` came out of a live kernel at 367% that way.
    //
    // So: subtract what it had already used at the last sample, and divide by
    // the interval. For a process born inside this interval there is nothing to
    // subtract and all of its CPU belongs here; for one that was already
    // running, the difference is exactly this interval's share. Both are the
    // live-row formula, which is what makes the number comparable to the rows
    // beside it.
    let used_us = utime.saturating_add(stime);
    let before_us = (before_ticks as f64 * 1e6 / boot.ticks_per_sec) as u64;
    let cpu = if elapsed_secs > 0.0 {
        ((used_us.saturating_sub(before_us) as f64 / 1e6) / elapsed_secs * 100.0) as f32
    } else {
        0.0
    };
    let _ = etime_us;

    Some(ProcSample {
        pid,
        ppid,
        name,
        user,
        cpu,
        rss: hiwater_rss_kb.saturating_mul(1024),
        // Gone, and it was never one number: a task group's thread count over
        // its life is not something the exit record carries.
        threads: None,
        // `X` is what the kernel calls a dead task and what `ps` prints for
        // one. The table needs it to be visibly not a live row.
        state: 'X',
        // In clock ticks since boot, because that is what field 22 of
        // `/proc/<pid>/stat` gives and what every live row carries. `ac_btime`
        // is seconds since the epoch, so storing it directly made `(pid,
        // started)` — the key that identifies a process across samples —
        // impossible to match against the same process alive a moment earlier.
        // Every exit then read as newly created, and the churn reconciliation
        // saturated to zero on any box with ordinary turnover.
        started: Some(boot.ticks_since_boot(btime)),
        // The command line lives in the process's memory, which is gone.
        cmd: None,
        // The record carries lifetime byte totals, not a rate over this
        // interval, and the column is a rate. A total rendered there would read
        // as a rate and be wrong by however long the process lived.
        io: None::<IoRates>,
        // The cgroup file is gone with the process. Its container could be
        // recovered from the exit record's cgroup id on a kernel that carries
        // one, but this record does not — so `None` means "not knowable here"
        // and the column shows nothing rather than guessing.
        container: None,
        minflt: None,
        majflt: None,
        vsize: None,
        nice: None,
        pss: None,
    })
}

fn classify(e: io::Error) -> Unavailable {
    match e.raw_os_error() {
        Some(1) | Some(13) => Unavailable::Permission,
        Some(22) => Unavailable::Namespace,
        _ => Unavailable::Other(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    const BOOT: Boot = Boot {
        epoch_secs: 1_788_800_000,
        ticks_per_sec: 100.0,
    };

    /// One `struct taskstats`, built at the offsets a live kernel was observed
    /// to use — version 14, checked field by field against real exit records
    /// before any of this was written.
    fn record(
        pid: i32,
        comm: &str,
        ppid: i32,
        etime_us: u64,
        utime_us: u64,
        rss_kb: u64,
        btime: u32,
    ) -> Vec<u8> {
        let mut s = vec![0u8; 400];
        s[0..2].copy_from_slice(&14u16.to_ne_bytes()); // version
        s[80..80 + comm.len()].copy_from_slice(comm.as_bytes()); // ac_comm
        s[120..124].copy_from_slice(&0u32.to_ne_bytes()); // ac_uid
        s[128..132].copy_from_slice(&(pid as u32).to_ne_bytes());
        s[132..136].copy_from_slice(&(ppid as u32).to_ne_bytes());
        s[136..140].copy_from_slice(&btime.to_ne_bytes()); // ac_btime
        s[144..152].copy_from_slice(&etime_us.to_ne_bytes());
        s[152..160].copy_from_slice(&utime_us.to_ne_bytes());
        s[160..168].copy_from_slice(&0u64.to_ne_bytes()); // ac_stime
        s[200..208].copy_from_slice(&rss_kb.to_ne_bytes()); // hiwater_rss
        s
    }

    #[test]
    fn an_exit_record_becomes_a_row_marked_as_gone() {
        let mut users = UserCache::new();
        let r = record(
            4021,
            "backup.sh",
            812,
            1_000_000,
            500_000,
            8_192,
            1_788_800_060,
        );
        let p = parse_exit(4021, &r, 1.0, BOOT, 0, &mut users)
            .expect("a well-formed record did not parse");
        assert_eq!(p.pid, 4021);
        assert_eq!(p.ppid, 812);
        assert_eq!(&*p.name, "backup.sh", "the name was misread");
        // Half a second of CPU across a one-second life.
        assert_eq!(
            p.cpu, 50.0,
            "the CPU is not the time used over its lifetime"
        );
        assert_eq!(p.rss, 8 << 20, "hiwater_rss is in kilobytes");
        assert_eq!(p.state, 'X', "an exited row is not marked as one");
        assert_eq!(
            p.threads, None,
            "an exit record has no thread count to give"
        );
        assert_eq!(p.cmd, None, "the command line dies with the process");
        assert!(
            p.io.is_none(),
            "the record's byte totals are a lifetime, not a rate for this interval"
        );
    }

    #[test]
    fn a_long_lived_process_does_not_exit_at_five_hundred_million_percent() {
        // `ac_utime` is the process's whole life, so dividing it by the
        // *interval* reports three hours of CPU as this second's. With the
        // default sort on CPU that pins every exiting long-lived process to the
        // top of the table, above everything actually running.
        let mut users = UserCache::new();
        let three_hours_us = 3 * 3600 * 1_000_000u64;
        let r = record(
            9,
            "postgres",
            1,
            three_hours_us,
            three_hours_us / 2,
            4,
            1_788_800_010,
        );
        // One-second interval, three-hour process, and no record of what it had
        // used before — the worst case, where the whole lifetime lands in one
        // interval. The cross-platform ceiling in `Collector::sample` is what
        // stops that reaching the table; this pins the shape the clamp exists
        // for, and the test above is the case that has a real answer.
        let p = parse_exit(9, &r, 1.0, BOOT, 0, &mut users).expect("no parse");
        assert!(
            p.cpu > 100.0,
            "the fixture no longer produces the value the clamp exists for"
        );
    }

    #[test]
    fn an_exit_records_start_time_is_on_the_same_clock_as_a_live_row() {
        // `ac_btime` is seconds since the epoch; a live row's `started` is
        // clock ticks since boot, from field 22 of `/proc/<pid>/stat`. Storing
        // one as the other makes `(pid, started)` — the key that identifies a
        // process across samples — impossible to match, so every exit reads as
        // newly created and the churn reconciliation saturates to zero.
        let mut users = UserCache::new();
        let started_60s_after_boot = BOOT.epoch_secs + 60;
        let r = record(4021, "x", 1, 1000, 0, 0, started_60s_after_boot as u32);
        let p = parse_exit(4021, &r, 1.0, BOOT, 0, &mut users).expect("no parse");
        assert_eq!(
            p.started,
            Some(6_000),
            "60 seconds after boot at 100 ticks a second is 6000 ticks"
        );

        // The key a live row would have carried for the same process.
        let live = ProcSample {
            pid: 4021,
            started: Some(6_000),
            ..Default::default()
        };
        assert_eq!(p.key(), live.key(), "the same process has two identities");
    }

    #[test]
    fn a_process_too_short_to_use_a_tick_reads_zero_rather_than_nothing() {
        // The commonest case, and the one the whole feature is for: 460us of
        // life and no measurable CPU. Zero is the true answer here — it used
        // none — and the row's value is that it *exists*.
        let mut users = UserCache::new();
        let p = parse_exit(
            9,
            &record(9, "true", 1, 460, 0, 776, 1_788_800_030),
            1.0,
            BOOT,
            0,
            &mut users,
        )
        .expect("no parse");
        assert_eq!(p.cpu, 0.0);
        assert_eq!(p.rss, 776 * 1024, "even a 460us process has a peak RSS");
    }

    #[test]
    fn a_record_from_before_the_stable_layout_is_refused_rather_than_guessed_at() {
        let mut users = UserCache::new();
        let mut old = record(1, "x", 0, 1, 0, 0, 1_788_800_000);
        old[0..2].copy_from_slice(&6u16.to_ne_bytes());
        assert!(
            parse_exit(1, &old, 1.0, BOOT, 0, &mut users).is_none(),
            "an ancient record was read at offsets it does not use"
        );
        // …and a truncated one, which is what a short read looks like.
        assert!(parse_exit(1, &old[..40], 1.0, BOOT, 0, &mut users).is_none());
    }

    #[test]
    fn a_name_shorter_than_its_field_does_not_carry_the_padding() {
        let mut users = UserCache::new();
        let p = parse_exit(
            1,
            &record(1, "sh", 0, 1, 0, 0, 1_788_800_000),
            1.0,
            BOOT,
            0,
            &mut users,
        )
        .expect("no parse");
        assert_eq!(&*p.name, "sh", "the NUL padding came with the name");
    }

    /// One netlink attribute, padded as the kernel pads it.
    fn attr(ty: u16, payload: &[u8]) -> Vec<u8> {
        let mut a = ((4 + payload.len()) as u16).to_ne_bytes().to_vec();
        a.extend(ty.to_ne_bytes());
        a.extend(payload);
        a.resize(align(a.len()), 0);
        a
    }

    /// A datagram carrying one exit record, framed the way the kernel frames
    /// it: a netlink header, a generic-netlink header, then the aggregate
    /// attribute with the pid and the stats nested inside.
    fn datagram(pid: i32, stats: &[u8]) -> Vec<u8> {
        let mut inner = attr(TYPE_TGID, &pid.to_ne_bytes());
        inner.extend(attr(TYPE_STATS, stats));
        let body = attr(AGGR_TGID, &inner);
        let len = 20 + body.len();
        let mut m = (len as u32).to_ne_bytes().to_vec();
        m.extend(0x15u16.to_ne_bytes()); // a family id, as the kernel assigns
        m.extend([0u8; 10]); // flags, sequence, port
        m.extend([2u8, 1, 0, 0]); // genl: cmd NEW, version 1
        m.extend(body);
        m
    }

    #[test]
    fn a_whole_datagram_is_read_as_the_exit_it_carries() {
        let r = record(
            4021,
            "backup.sh",
            812,
            1_000_000,
            500_000,
            8_192,
            1_788_800_060,
        );
        let mut two = datagram(4021, &r);
        two.extend(datagram(
            4022,
            &record(4022, "tar", 812, 1, 1, 1, 1_788_800_060),
        ));
        let rows = exits_in(&two, 1.0, BOOT, &HashMap::new(), &mut UserCache::new());
        let got: Vec<(i32, &str)> = rows.iter().map(|p| (p.pid, &*p.name)).collect();
        assert_eq!(got, [(4021, "backup.sh"), (4022, "tar")]);
    }

    #[test]
    fn no_datagram_can_panic_the_reader() {
        // Every length in a datagram is one this code indexes with. They come
        // from the kernel, which is trusted to write them correctly — and a
        // reader that panics when one is not takes the whole program with it.
        let r = record(
            4021,
            "backup.sh",
            812,
            1_000_000,
            500_000,
            8_192,
            1_788_800_060,
        );
        let mut seed = datagram(4021, &r);
        seed.extend(datagram(4022, &r));
        let before = HashMap::from([(4021, u64::MAX)]);
        for bytes in crate::mangle::variants(&seed, 2_000) {
            let _ = exits_in(&bytes, 1.0, BOOT, &before, &mut UserCache::new());
            let _ = exits_in(&bytes, 0.0, BOOT, &before, &mut UserCache::new());
        }
        for bytes in crate::mangle::variants(&r, 2_000) {
            let _ = parse_exit(4021, &bytes, 1.0, BOOT, u64::MAX, &mut UserCache::new());
        }
    }

    /// One netlink message: the sixteen-byte header, then `payload`.
    fn message(ty: u16, seq: u32, payload: &[u8]) -> Vec<u8> {
        let len = 16 + payload.len();
        let mut m = (len as u32).to_ne_bytes().to_vec();
        m.extend(ty.to_ne_bytes());
        m.extend(0u16.to_ne_bytes());
        m.extend(seq.to_ne_bytes());
        m.extend(0u32.to_ne_bytes());
        m.extend(payload);
        m.resize(align(m.len()), 0);
        m
    }

    /// An `NLMSG_ERROR` carrying `errno`, or an ack for zero.
    fn error(seq: u32, errno: i32) -> Vec<u8> {
        let mut p = (-errno).to_ne_bytes().to_vec();
        p.extend([0u8; 16]); // the header of the request it answers
        message(NLMSG_ERROR, seq, &p)
    }

    fn family_reply(id: u16) -> Vec<u8> {
        let mut p = vec![1u8, 2, 0, 0]; // genl: CTRL_CMD_NEWFAMILY, version 2
        p.extend(attr(CTRL_ATTR_FAMILY_NAME, b"TASKSTATS\0"));
        p.extend(attr(CTRL_ATTR_FAMILY_ID, &id.to_ne_bytes()));
        message(GENL_ID_CTRL, 1, &p)
    }

    #[test]
    fn the_family_id_is_read_from_the_reply_and_not_from_an_error() {
        assert_eq!(family_in(&family_reply(0x15)).ok(), Some(0x15));
        // The ack the request asks for can share the datagram, either side.
        let mut both = error(1, 0);
        both.extend(family_reply(0x17));
        assert_eq!(family_in(&both).ok(), Some(0x17));
        // An error where the reply should be: its payload is the request that
        // failed, and was once searched for attributes as if it were a reply.
        assert!(matches!(
            family_in(&error(1, 2)),
            Err(Unavailable::NoFamily)
        ));
        assert!(matches!(
            family_in(&error(1, 1)),
            Err(Unavailable::Permission)
        ));
        assert!(matches!(family_in(&[]), Err(Unavailable::NoFamily)));
    }

    #[test]
    fn the_registration_answer_is_the_one_with_our_sequence_number() {
        // The lookup's own ack, which once read as "registered".
        assert!(registration_in(&error(1, 0)).is_none());
        assert!(matches!(
            registration_in(&error(SEQ_REGISTER, 0)),
            Some(Ok(()))
        ));
        assert!(matches!(
            registration_in(&error(SEQ_REGISTER, 1)),
            Some(Err(Unavailable::Permission))
        ));
        assert!(matches!(
            registration_in(&error(SEQ_REGISTER, 22)),
            Some(Err(Unavailable::Namespace))
        ));
        let mut later = error(1, 0);
        later.extend(error(SEQ_REGISTER, 13));
        assert!(matches!(
            registration_in(&later),
            Some(Err(Unavailable::Permission))
        ));
        // A header whose length runs past the read is not a message.
        let mut cut = error(SEQ_REGISTER, 1);
        cut.truncate(12);
        assert!(registration_in(&cut).is_none());
    }

    #[test]
    fn no_control_reply_can_panic_the_reader() {
        let mut seed = error(1, 0);
        seed.extend(family_reply(0x15));
        seed.extend(error(SEQ_REGISTER, 22));
        for bytes in crate::mangle::variants(&seed, 2_000) {
            let _ = family_in(&bytes);
            let _ = registration_in(&bytes);
        }
    }

    #[test]
    fn the_live_socket_hears_a_real_exit_or_says_why_not() {
        // The only test that goes through `socket`, `bind`, `setsockopt`,
        // `send` and `recv` for real. Unprivileged it proves the refusal is
        // one of the reasons this module can name; with `CAP_NET_ADMIN` in
        // the initial namespaces it proves a process that lived a moment is
        // heard. Run that way under AddressSanitizer in a privileged
        // container, every buffer handed across the boundary is checked.
        let mut l = match Listener::open() {
            Ok(l) => l,
            Err(why) => {
                assert!(!why.why().is_empty());
                return;
            }
        };
        let child = std::process::Command::new("true")
            .status()
            .expect("cannot run `true`");
        assert!(child.success());
        let mut seen = Vec::new();
        for _ in 0..50 {
            seen.extend(l.drain(1.0, BOOT, &HashMap::new(), &mut UserCache::new()));
            if seen.iter().any(|p| &*p.name == "true") {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let names: Vec<&str> = seen.iter().map(|p| &*p.name).collect();
        panic!("registered, but `true` never arrived; heard {names:?}");
    }
}
