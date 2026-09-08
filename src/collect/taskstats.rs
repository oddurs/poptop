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
const TYPE_PID: u16 = 1;
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
        unsafe {
            let fd = socket(AF_NETLINK, SOCK_RAW, NETLINK_GENERIC);
            if fd < 0 {
                return Err(classify(io::Error::last_os_error()));
            }
            let mut me = Listener {
                fd,
                buf: vec![0; 64 << 10],
            };

            let mut addr = [0u8; 12];
            addr[0..2].copy_from_slice(&(AF_NETLINK as u16).to_ne_bytes());
            if bind(fd, addr.as_ptr(), 12) < 0 {
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
            if setsockopt(fd, SOL_SOCKET, SO_RCVBUFFORCE, ptr, 4) < 0 {
                setsockopt(fd, SOL_SOCKET, SO_RCVBUF, ptr, 4);
            }

            let family = me.family()?;
            me.register(family)?;
            Ok(me)
        }
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
        let mut id = 0u16;
        if n > 20 {
            attrs(&self.buf[20..n], |ty, v| {
                if ty == CTRL_ATTR_FAMILY_ID && v.len() >= 2 {
                    id = u16::from_ne_bytes([v[0], v[1]]);
                }
            });
        }
        if id == 0 {
            return Err(Unavailable::NoFamily);
        }
        Ok(id)
    }

    /// Ask for every CPU's exits.
    fn register(&mut self, family: u16) -> Result<(), Unavailable> {
        let cpus = std::thread::available_parallelism().map_or(1, |n| n.get());
        let mut mask = format!("0-{}", cpus - 1).into_bytes();
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
            if n < 20 {
                continue;
            }
            let seq = u32::from_ne_bytes(self.buf[8..12].try_into().unwrap_or([0; 4]));
            if seq != SEQ_REGISTER {
                continue;
            }
            let ty = u16::from_ne_bytes([self.buf[4], self.buf[5]]);
            if ty != NLMSG_ERROR {
                return Ok(());
            }
            let err = i32::from_ne_bytes(self.buf[16..20].try_into().unwrap_or([0; 4]));
            return match -err {
                0 => Ok(()),
                1 | 13 => Err(Unavailable::Permission),
                // The namespace gate returns this for a mask the kernel has
                // already parsed successfully — the boundary sits exactly at
                // `nr_cpu_ids`, so a mask wider than the machine gives `ERANGE`
                // and a valid one from the wrong namespace gives `EINVAL`.
                22 => Err(Unavailable::Namespace),
                e => Err(Unavailable::Other(io::Error::from_raw_os_error(e))),
            };
        }
        Err(Unavailable::Other(io::Error::other(
            "no answer to the listener registration",
        )))
    }

    fn send(&self, m: &[u8]) -> Result<(), Unavailable> {
        let sent = unsafe { send(self.fd, m.as_ptr(), m.len(), 0) };
        if sent < 0 {
            return Err(classify(io::Error::last_os_error()));
        }
        Ok(())
    }

    fn blocking_read(&mut self) -> Result<usize, Unavailable> {
        let n = unsafe { recv(self.fd, self.buf.as_mut_ptr(), self.buf.len(), 0) };
        if n < 0 {
            return Err(classify(io::Error::last_os_error()));
        }
        Ok(n as usize)
    }

    /// Everything that has exited since the last call.
    ///
    /// Never blocks: whatever the kernel has queued is what this interval saw,
    /// and waiting for more would be waiting for the future.
    pub fn drain(&mut self, elapsed_secs: f64, users: &mut UserCache) -> Vec<ProcSample> {
        let mut out = Vec::new();
        loop {
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
            let n = n as usize;
            if n < 20 || u16::from_ne_bytes([self.buf[4], self.buf[5]]) == NLMSG_ERROR {
                continue;
            }
            // Copied out because `attrs` borrows the buffer while the closure
            // needs the user cache, and the next read overwrites it anyway.
            let body = self.buf[20..n].to_vec();
            attrs(&body, |ty, v| {
                if ty != AGGR_PID {
                    return;
                }
                let mut pid = 0i32;
                let mut stats: Option<&[u8]> = None;
                attrs(v, |ity, iv| match ity {
                    TYPE_PID if iv.len() >= 4 => {
                        pid = i32::from_ne_bytes(iv[0..4].try_into().unwrap_or([0; 4]));
                    }
                    TYPE_STATS => stats = Some(iv),
                    _ => {}
                });
                if let Some(s) = stats
                    && let Some(p) = parse_exit(pid, s, elapsed_secs, users)
                {
                    out.push(p);
                }
            });
        }
        out
    }
}

/// Resolve a uid to a name, reusing the collector's cache.
pub type UserCache = std::collections::HashMap<u32, Arc<str>>;

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

    // Microseconds of CPU over the interval, as a percentage of one core — the
    // same scale as a live row, so the two can sit in one column. A process too
    // short to accumulate a tick reads 0.0, which is true: it used no
    // measurable CPU.
    let cpu = if elapsed_secs > 0.0 {
        ((utime.saturating_add(stime) as f64 / 1e6) / elapsed_secs * 100.0) as f32
    } else {
        0.0
    };

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
        started: Some(btime),
        // The command line lives in the process's memory, which is gone.
        cmd: None,
        // The record carries lifetime byte totals, not a rate over this
        // interval, and the column is a rate. A total rendered there would read
        // as a rate and be wrong by however long the process lived.
        io: None::<IoRates>,
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

    /// One `struct taskstats`, built at the offsets a live kernel was observed
    /// to use — version 14, checked field by field against real exit records
    /// before any of this was written.
    fn record(pid: i32, comm: &str, uid: u32, ppid: i32, utime_us: u64, rss_kb: u64) -> Vec<u8> {
        let mut s = vec![0u8; 400];
        s[0..2].copy_from_slice(&14u16.to_ne_bytes()); // version
        s[80..80 + comm.len()].copy_from_slice(comm.as_bytes()); // ac_comm
        s[120..124].copy_from_slice(&uid.to_ne_bytes());
        s[128..132].copy_from_slice(&(pid as u32).to_ne_bytes());
        s[132..136].copy_from_slice(&(ppid as u32).to_ne_bytes());
        s[136..140].copy_from_slice(&1_788_858_843u32.to_ne_bytes()); // ac_btime
        s[144..152].copy_from_slice(&460u64.to_ne_bytes()); // ac_etime
        s[152..160].copy_from_slice(&utime_us.to_ne_bytes());
        s[160..168].copy_from_slice(&0u64.to_ne_bytes()); // ac_stime
        s[200..208].copy_from_slice(&rss_kb.to_ne_bytes()); // hiwater_rss
        s
    }

    #[test]
    fn an_exit_record_becomes_a_row_marked_as_gone() {
        let mut users = UserCache::new();
        let p = parse_exit(
            4021,
            &record(4021, "backup.sh", 0, 812, 500_000, 8_192),
            1.0,
            &mut users,
        )
        .expect("a well-formed record did not parse");
        assert_eq!(p.pid, 4021);
        assert_eq!(p.ppid, 812);
        assert_eq!(&*p.name, "backup.sh", "the name was misread");
        // Half a second of CPU in a one-second interval.
        assert_eq!(
            p.cpu, 50.0,
            "the CPU is not the time used over the interval"
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
        assert_eq!(p.started, Some(1_788_858_843));
    }

    #[test]
    fn a_process_too_short_to_use_a_tick_reads_zero_rather_than_nothing() {
        // The commonest case, and the one the whole feature is for: 460us of
        // life and no measurable CPU. Zero is the true answer here — it used
        // none — and the row's value is that it *exists*.
        let mut users = UserCache::new();
        let p = parse_exit(9, &record(9, "true", 0, 1, 0, 776), 1.0, &mut users).expect("no parse");
        assert_eq!(p.cpu, 0.0);
        assert_eq!(p.rss, 776 * 1024, "even a 460us process has a peak RSS");
    }

    #[test]
    fn a_record_from_before_the_stable_layout_is_refused_rather_than_guessed_at() {
        let mut users = UserCache::new();
        let mut old = record(1, "x", 0, 0, 0, 0);
        old[0..2].copy_from_slice(&6u16.to_ne_bytes());
        assert!(
            parse_exit(1, &old, 1.0, &mut users).is_none(),
            "an ancient record was read at offsets it does not use"
        );
        // …and a truncated one, which is what a short read looks like.
        assert!(parse_exit(1, &old[..40], 1.0, &mut users).is_none());
    }

    #[test]
    fn a_name_shorter_than_its_field_does_not_carry_the_padding() {
        let mut users = UserCache::new();
        let p = parse_exit(1, &record(1, "sh", 0, 0, 0, 0), 1.0, &mut users).expect("no parse");
        assert_eq!(&*p.name, "sh", "the NUL padding came with the name");
    }
}
