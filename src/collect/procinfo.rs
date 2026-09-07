//! Facts about processes that `sysinfo` will not report on macOS.
//!
//! Two of them so far, and they fail the same way: sysinfo answers only for
//! processes the caller owns, so on a machine with several users' daemons
//! running — which is every Mac — a third of the table comes back with a
//! placeholder. A placeholder is the one thing this tool is not allowed to
//! produce, so both are read here instead.
//!
//! # Start times, via `sysctl(KERN_PROC_ALL)`
//!
//! poptop identifies a process by `(pid, started)` so that a recycled pid
//! cannot splice two unrelated programs into one graph. sysinfo supplies the
//! start time on macOS, but only for processes the caller owns: measured on
//! this machine, 171 of 600 came back as `0`. For those 171 the key silently
//! degraded to the pid alone — exactly the case it exists to defend against.
//!
//! `proc_pidinfo(PROC_PIDTBSDINFO)` is no better; it refuses the same 171.
//! `sysctl(KERN_PROC_ALL)` answers for all of them, unprivileged, in one call —
//! it is how `ps` prints `lstart` for other users' processes, and how htop's
//! Darwin backend gets the same figure.
//!
//! ## Why this is a byte offset and not a struct
//!
//! The record is `struct kinfo_proc`, ~648 bytes of nested BSD structures whose
//! Rust transcription would be a hundred lines that no test could distinguish
//! from a correct one. Only two fields are wanted, both near the front, so the
//! two offsets are read directly and then *checked against a known answer*:
//! [`Kinfo::probe`] asks for our own process and requires the pid field to read
//! back our own pid. A shifted layout fails that immediately, at startup, and
//! the caller falls back to sysinfo rather than reporting fiction.
//!
//! It is the same path a macOS that changed the layout out from under us would
//! take, and it is checked once at startup rather than trusted.
//!
//! An earlier version of this note claimed the check also made the module safe
//! on another BSD, which has its own `kinfo_proc`. It does not, and the claim
//! was never testable: `sysinfo` is a macOS-only dependency in `Cargo.toml`
//! while the backend around this one compiles for every non-Linux target, so
//! `cargo check --target x86_64-unknown-freebsd` has never got as far as this
//! file. Linux and macOS are the platforms; the `target_vendor` gate below is
//! there so a future third one fails to *run* a Darwin-only call rather than
//! failing to link over it.
//!
//! # Thread counts, via `proc_pidinfo(PROC_PIDTASKINFO)`
//!
//! sysinfo exposes a thread count on Linux only, so the macOS backend reported
//! a flat `1` for every process. On anything genuinely threaded that is not
//! merely missing, it is *contradictory*: a virtual machine using three cores
//! rendered as `300%` beside `1 thread`, and a single thread cannot exceed one
//! core. The figure said the table could not be trusted.
//!
//! `pti_threadnum` gives the real count. Same ownership boundary as everything
//! else here — 433 of 607 on this machine — but it falls in a place that costs
//! nothing: measured across the whole table, **no process above 5% CPU had an
//! unreadable thread count**, because a process busy enough for the number to
//! matter is almost always one of your own. The rest report `None` and render
//! as an em dash.
//!
//! Costs 263us for 607 processes, warm — about 6% of a macOS sample, which is
//! why this is not gated behind a visible column the way per-process IO is.
//! `threads` is a core field and stays one.

use crate::sample::FsStat;
use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::Arc;

// libSystem, already linked by std. Declared here rather than taking a
// dependency on `libc` for one function.
// `libproc`, and Apple's alone — unlike `sysctl`, which every BSD has. Split
// into its own block so a non-Apple target does not fail to *link* over a
// symbol this module is careful to degrade from at runtime.
#[cfg(target_vendor = "apple")]
unsafe extern "C" {
    fn proc_pidinfo(pid: i32, flavor: i32, arg: u64, buffer: *mut c_void, buffersize: i32) -> i32;
    fn getfsstat(buf: *mut c_void, bufsize: i32, flags: i32) -> i32;
}

unsafe extern "C" {
    fn sysctl(
        name: *mut i32,
        namelen: u32,
        oldp: *mut c_void,
        oldlenp: *mut usize,
        newp: *mut c_void,
        newlen: usize,
    ) -> i32;
}

const CTL_KERN: i32 = 1;
const KERN_PROC: i32 = 14;
const KERN_PROC_ALL: i32 = 0;
const KERN_PROC_PID: i32 = 1;

/// `struct kinfo_proc` opens with `struct extern_proc`, which opens with a
/// union of two pointers and a `struct timeval` — so the start time is at zero.
const OFF_STARTTIME: usize = 0;
/// `p_un` 0, `p_vmspace` 16, `p_sigacts` 24, `p_flag` 32, `p_stat` 36, then
/// `p_pid` at 40 once the `char` is padded up to the alignment of `pid_t`.
const OFF_PID: usize = 40;

/// The size one record must fall within for the offsets above to be inside it,
/// and for a plausible `kinfo_proc`. A kernel reporting anything else is one
/// this module does not understand.
const PLAUSIBLE: std::ops::RangeInclusive<usize> = 128..=8192;

pub struct Kinfo {
    /// Bytes per record, measured from the kernel's own answer rather than
    /// assumed: a null-buffer probe returns a padded *estimate* (six times the
    /// true size on this machine), so the size is taken from a real read.
    stride: usize,
    /// Reused between samples. The table is a few hundred kilobytes and this
    /// runs once a second.
    buf: Vec<u8>,
}

impl Kinfo {
    /// Read one known record and check the layout against it.
    ///
    /// `None` means this kernel's `kinfo_proc` is not the one described above.
    /// It is the only check that matters: if the pid field reads back our own
    /// pid, both offsets are where they are believed to be.
    pub fn probe() -> Option<Self> {
        let mut mib = [
            CTL_KERN,
            KERN_PROC,
            KERN_PROC_PID,
            std::process::id() as i32,
        ];
        let mut buf = vec![0u8; *PLAUSIBLE.end()];
        let mut len = buf.len();
        let rc = unsafe {
            sysctl(
                mib.as_mut_ptr(),
                4,
                buf.as_mut_ptr().cast(),
                &mut len,
                std::ptr::null_mut(),
                0,
            )
        };
        (rc == 0)
            .then(|| accept(len, &buf, std::process::id() as i32))
            .flatten()
            .map(|stride| Self {
                stride,
                buf: Vec::new(),
            })
    }
}

/// Whether a record the kernel returned for `mine` is laid out as believed.
///
/// Split out so it can be handed a record this module did not fetch. The check
/// in `probe` is the one thing licensing every fixed offset below it, and a
/// check that cannot be shown to reject anything is not a check.
fn accept(len: usize, record: &[u8], mine: i32) -> Option<usize> {
    if !PLAUSIBLE.contains(&len) || len > record.len() {
        return None;
    }
    let pid = i32::from_ne_bytes(record.get(OFF_PID..OFF_PID + 4)?.try_into().ok()?);
    (pid == mine).then_some(len)
}

impl Kinfo {
    /// pid -> start time in microseconds since the epoch.
    ///
    /// Empty on any failure. The caller treats a missing pid as "no start time
    /// for this process", which is what it is: something the kernel would not
    /// say, not a zero.
    pub fn starts(&mut self) -> HashMap<i32, u64> {
        let mut mib = [CTL_KERN, KERN_PROC, KERN_PROC_ALL, 0];
        let mut len = 0usize;
        let rc = unsafe {
            sysctl(
                mib.as_mut_ptr(),
                4,
                std::ptr::null_mut(),
                &mut len,
                std::ptr::null_mut(),
                0,
            )
        };
        if rc != 0 {
            return HashMap::new();
        }
        // Slack, because processes are forked between the sizing call and the
        // read and the kernel truncates rather than growing. Sixty-four new
        // processes in that window would be remarkable.
        //
        // If it happens anyway the whole table is dropped for that one sample,
        // not shortened: every process gets `started: None`, so every sparkline
        // takes a gap in that slot and fills again on the next. A gap is what
        // this tool draws when it does not know, and a partial table would be a
        // set of processes silently declared identity-less for one instant.
        self.buf.clear();
        self.buf.resize(len + self.stride * 64, 0);
        len = self.buf.len();
        let rc = unsafe {
            sysctl(
                mib.as_mut_ptr(),
                4,
                self.buf.as_mut_ptr().cast(),
                &mut len,
                std::ptr::null_mut(),
                0,
            )
        };
        if rc != 0 || len > self.buf.len() {
            return HashMap::new();
        }
        self.buf[..len]
            .chunks_exact(self.stride)
            .filter_map(|r| {
                let pid = i32::from_ne_bytes(r[OFF_PID..OFF_PID + 4].try_into().ok()?);
                let sec = i64::from_ne_bytes(r[OFF_STARTTIME..OFF_STARTTIME + 8].try_into().ok()?);
                let usec =
                    i32::from_ne_bytes(r[OFF_STARTTIME + 8..OFF_STARTTIME + 12].try_into().ok()?);
                // A non-positive start time is not a time. Dropped rather than
                // passed on as a token, so the identity stays honest.
                (sec > 0).then(|| (pid, sec as u64 * 1_000_000 + usec.max(0) as u64))
            })
            .collect()
    }
}

/// `PROC_PIDTASKINFO`, and the size `struct proc_taskinfo` has to be for the
/// offset below to mean what it says.
const PROC_PIDTASKINFO: i32 = 4;
/// Six `uint64_t` then twelve `int32_t`: 48 + 48. A kernel writing any other
/// number of bytes is not writing the structure this module was written
/// against. Counted out because the count is what a future reader would check
/// `OFF_THREADNUM` against, and an off-by-two here moves the offset off the
/// field.
const TASKINFO_SIZE: i32 = 96;
/// `pti_policy` 48, `pti_faults` 52, `pti_pageins` 56, `pti_cow_faults` 60,
/// `pti_messages_sent` 64, `_received` 68, `pti_syscalls_mach` 72,
/// `_unix` 76, `pti_csw` 80, then `pti_threadnum` at 84.
const OFF_THREADNUM: usize = 84;

/// How many threads a process has, or `None` if the kernel will not say.
///
/// `None` for any process this user does not own, which is roughly a third of a
/// Mac's table — and deliberately not `1`. A flat `1` is not a missing figure,
/// it is a wrong one, and beside a CPU column it is visibly wrong: one thread
/// cannot use three cores. An em dash says "not mine to see", which is true.
///
/// Validated the same way as everything else here: the call reports how many
/// bytes it wrote, and anything but the expected size means the structure is
/// not the one these offsets were written against.
#[cfg(not(target_vendor = "apple"))]
pub fn threads(_pid: i32) -> Option<u32> {
    // Another BSD has `proc_pidinfo` nowhere, or somewhere else. Saying nothing
    // is the same answer this gives for a process it may not read, and the
    // column already renders that.
    None
}

#[cfg(target_vendor = "apple")]
pub fn threads(pid: i32) -> Option<u32> {
    let mut buf = [0u8; TASKINFO_SIZE as usize];
    let n = unsafe {
        proc_pidinfo(
            pid,
            PROC_PIDTASKINFO,
            0,
            buf.as_mut_ptr().cast(),
            TASKINFO_SIZE,
        )
    };
    if n != TASKINFO_SIZE {
        return None;
    }
    let v = i32::from_ne_bytes(buf[OFF_THREADNUM..OFF_THREADNUM + 4].try_into().ok()?);
    // A live task always has at least one thread. Zero or negative means the
    // offset is not pointing at a thread count.
    (v > 0).then_some(v as u32)
}

/// Ask for what is already known rather than going to the filesystem to find
/// out. The whole reason `getfsstat` is usable here: a mount that has stopped
/// answering cannot hang the collector, because nothing waits on it.
#[cfg(target_vendor = "apple")]
const MNT_NOWAIT: i32 = 2;

/// `sizeof(struct statfs)` with 64-bit inodes, which every macOS this could run
/// on uses. Checked at runtime rather than trusted — see [`filesystems`].
#[cfg(target_vendor = "apple")]
const STATFS_SIZE: usize = 2168;
/// `f_bsize` 0, `f_iosize` 4, `f_blocks` 8, `f_bfree` 16, `f_bavail` 24,
/// `f_files` 32, `f_ffree` 40, `f_fsid` 48, `f_owner` 56, `f_type` 60,
/// `f_flags` 64, `f_fssubtype` 68, then the three name arrays.
#[cfg(target_vendor = "apple")]
const OFF: FsOffsets = FsOffsets {
    bsize: 0,
    blocks: 8,
    bavail: 24,
    flags: 64,
    fstype: 72,
    mount: 88,
    from: 1112,
};

/// `MNT_RDONLY`.
#[cfg(target_vendor = "apple")]
const MNT_RDONLY: u32 = 0x1;

#[cfg(target_vendor = "apple")]
struct FsOffsets {
    bsize: usize,
    blocks: usize,
    bavail: usize,
    flags: usize,
    fstype: usize,
    mount: usize,
    from: usize,
}

/// A NUL-terminated name out of a fixed-size array.
#[cfg(target_vendor = "apple")]
fn cstr(b: &[u8]) -> &str {
    let n = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    std::str::from_utf8(&b[..n]).unwrap_or("")
}

/// Mounted filesystems, or `None` if this kernel's `struct statfs` is not the
/// one these offsets were written against.
///
/// One call for every mount — 7us for twelve of them, measured, against the
/// 12.5ms `sysinfo::Disks` costs for the same figures. Validated the way the
/// process table is: a machine always has a filesystem mounted at `/` with a
/// non-zero size, so if that is not what comes back, the layout is not what
/// this believes and nothing here is reported.
#[cfg(target_vendor = "apple")]
pub fn filesystems() -> Option<Vec<FsStat>> {
    let n = unsafe { getfsstat(std::ptr::null_mut(), 0, MNT_NOWAIT) };
    if n <= 0 {
        return None;
    }
    // Slack, because a volume can be mounted between the two calls.
    let mut buf = vec![0u8; STATFS_SIZE * (n as usize + 8)];
    let got = unsafe { getfsstat(buf.as_mut_ptr().cast(), buf.len() as i32, MNT_NOWAIT) };
    if got <= 0 {
        return None;
    }
    let out = parse_statfs(&buf, got as usize);
    // The layout check. Every machine has a sized filesystem at the root.
    crate::collect::with_a_root(out)
}

#[cfg(not(target_vendor = "apple"))]
pub fn filesystems() -> Option<Vec<FsStat>> {
    None
}

/// Split out so the offsets, the filters and the de-duplication can be tested
/// against a record this module built rather than one the kernel happened to
/// return.
#[cfg(target_vendor = "apple")]
fn parse_statfs(buf: &[u8], count: usize) -> Vec<FsStat> {
    let mut out: Vec<(String, bool, FsStat)> = Vec::new();
    for i in 0..count {
        let Some(r) = buf.get(i * STATFS_SIZE..(i + 1) * STATFS_SIZE) else {
            break;
        };
        let u32_at = |o: usize| u32::from_ne_bytes(r[o..o + 4].try_into().unwrap()) as u64;
        let u64_at = |o: usize| u64::from_ne_bytes(r[o..o + 8].try_into().unwrap());
        let bsize = u32_at(OFF.bsize);
        let total = u64_at(OFF.blocks).saturating_mul(bsize);
        let avail = u64_at(OFF.bavail).saturating_mul(bsize);
        let flags = u32_at(OFF.flags) as u32;
        let kind = cstr(&r[OFF.fstype..OFF.fstype + 16]);
        let mount = cstr(&r[OFF.mount..OFF.mount + 1024]);
        let from = cstr(&r[OFF.from..OFF.from + 1024]).to_string();
        if total == 0 || crate::collect::is_ram_backed(kind) {
            continue;
        }
        // Kept, and marked. A filesystem nothing can be written to cannot fill
        // up — but since Catalina the volume macOS mounts at `/` is the sealed,
        // read-only system one, so dropping it here would take the only name a
        // reader recognises with it. `merge_filesystems` folds it in for its
        // name and takes the space from its writable sibling.
        out.push((
            from,
            flags & MNT_RDONLY == 0,
            FsStat {
                mount: Arc::from(mount),
                total,
                avail,
            },
        ));
    }
    crate::collect::merge_filesystems(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    unsafe extern "C" {
        fn geteuid() -> u32;
    }

    #[test]
    fn the_layout_check_agrees_with_this_kernel() {
        assert!(
            Kinfo::probe().is_some(),
            "kinfo_proc is not laid out as this module believes"
        );
    }

    #[test]
    fn every_live_process_gets_a_distinct_start_time() {
        // The whole point of the module. If two processes share a token the
        // key is no better than the pid on its own for that pair.
        let mut k = Kinfo::probe().unwrap();
        let starts = k.starts();
        assert!(starts.len() > 10, "only {} processes", starts.len());
        // Discriminating power, not exactness. Two processes really can fork in
        // the same microsecond on a loaded machine, and that costs nothing:
        // what the rest of the tool needs is that `(pid, started)` is unique,
        // and the pid half already guarantees that — which is also why
        // asserting the pairs are distinct would assert nothing at all.
        //
        // The failure worth catching is an offset landing on some field that
        // reads the same for everyone, which would leave a handful of distinct
        // values across hundreds of processes rather than a couple of
        // collisions.
        let distinct: std::collections::HashSet<_> = starts.values().collect();
        assert!(
            distinct.len() * 100 >= starts.len() * 95,
            "only {} distinct start times across {} processes — this is not a start time",
            distinct.len(),
            starts.len()
        );
        assert!(starts.values().all(|&t| t > 0), "a zero token got through");
    }

    #[test]
    fn our_own_start_time_is_in_the_past_and_this_century() {
        // Catches a plausible-looking misread: an offset landing on some other
        // field would still be a number, but not one near now.
        let mut k = Kinfo::probe().unwrap();
        let mine = k.starts()[&(std::process::id() as i32)];
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64;
        assert!(mine <= now, "we started in the future");
        assert!(
            now - mine < 60 * 60 * 1_000_000,
            "this test process claims to be over an hour old"
        );
    }

    /// A record of `len` bytes whose pid field holds `pid`.
    fn record(len: usize, pid: i32) -> Vec<u8> {
        let mut r = vec![0u8; len.max(OFF_PID + 4)];
        r[OFF_PID..OFF_PID + 4].copy_from_slice(&pid.to_ne_bytes());
        r
    }

    /// One `struct statfs` record with the given fields.
    #[cfg(target_vendor = "apple")]
    fn statfs_record(bsize: u32, blocks: u64, bavail: u64, kind: &str, mount: &str) -> Vec<u8> {
        fs_record(bsize, blocks, bavail, kind, mount, mount, 0)
    }

    #[cfg(target_vendor = "apple")]
    fn fs_record(
        bsize: u32,
        blocks: u64,
        bavail: u64,
        kind: &str,
        mount: &str,
        from: &str,
        flags: u32,
    ) -> Vec<u8> {
        let mut r = vec![0u8; STATFS_SIZE];
        r[OFF.flags..OFF.flags + 4].copy_from_slice(&flags.to_ne_bytes());
        r[OFF.from..OFF.from + from.len()].copy_from_slice(from.as_bytes());
        r[OFF.bsize..OFF.bsize + 4].copy_from_slice(&bsize.to_ne_bytes());
        r[OFF.blocks..OFF.blocks + 8].copy_from_slice(&blocks.to_ne_bytes());
        r[OFF.bavail..OFF.bavail + 8].copy_from_slice(&bavail.to_ne_bytes());
        r[OFF.fstype..OFF.fstype + kind.len()].copy_from_slice(kind.as_bytes());
        r[OFF.mount..OFF.mount + mount.len()].copy_from_slice(mount.as_bytes());
        r
    }

    #[test]
    fn the_same_filesystem_seen_twice_is_reported_once() {
        // APFS puts every volume in one container, so seven of them report the
        // same size and differ only in what is left. The shorter mount point is
        // the one a reader recognises.
        let mut buf = Vec::new();
        // One container, three volumes: `/dev/disk3s1s1`, `/dev/disk3s5` and
        // `/dev/disk3s4` all reduce to `/dev/disk3`.
        for (mount, dev) in [
            ("/System/Volumes/Data", "/dev/disk3s5"),
            ("/", "/dev/disk3s1s1"),
            ("/System/Volumes/Update/mnt1", "/dev/disk3s4"),
        ] {
            buf.extend(fs_record(4096, 1000, 400, "apfs", mount, dev, 0));
        }
        let fs = parse_statfs(&buf, 3);
        assert_eq!(fs.len(), 1, "one filesystem was reported three times");
        assert_eq!(&*fs[0].mount, "/", "the longest mount point was kept");
    }

    #[test]
    fn a_read_only_volume_lends_its_name_and_not_its_space() {
        // Since Catalina the volume macOS mounts at `/` is the sealed,
        // read-only system one and the writable half is
        // `/System/Volumes/Data`. It has to survive far enough to give up its
        // name, and its free space must not be the figure reported.
        let mut buf = Vec::new();
        buf.extend(fs_record(
            4096,
            1000,
            900,
            "apfs",
            "/",
            "/dev/disk3s1s1",
            MNT_RDONLY,
        ));
        buf.extend(fs_record(
            4096,
            1000,
            120,
            "apfs",
            "/System/Volumes/Data",
            "/dev/disk3s5",
            0,
        ));
        let fs = parse_statfs(&buf, 2);
        assert_eq!(fs.len(), 1);
        assert_eq!(&*fs[0].mount, "/");
        assert_eq!(
            fs[0].avail,
            4096 * 120,
            "the sealed volume's space was used"
        );
    }

    #[test]
    fn a_wholly_read_only_container_is_not_reported() {
        // A mounted disk image. Nothing can be written to it, so it cannot fill
        // up — and it reports no available space, which would read as 100% full
        // forever.
        let mut buf = Vec::new();
        buf.extend(fs_record(
            4096,
            1000,
            0,
            "apfs",
            "/Volumes/Installer",
            "/dev/disk9s1",
            MNT_RDONLY,
        ));
        buf.extend(fs_record(4096, 2000, 500, "apfs", "/", "/dev/disk3s5", 0));
        let fs = parse_statfs(&buf, 2);
        assert_eq!(fs.len(), 1);
        assert_eq!(&*fs[0].mount, "/");
    }

    #[test]
    fn pseudo_and_ram_filesystems_are_left_out() {
        let mut buf = Vec::new();
        // `devfs` reports a real size and is memory; `autofs` reports none.
        buf.extend(statfs_record(
            4096,
            0,
            0,
            "autofs",
            "/System/Volumes/Data/home",
        ));
        buf.extend(statfs_record(4096, 52, 0, "devfs", "/dev"));
        buf.extend(statfs_record(4096, 1000, 400, "apfs", "/"));
        let fs = parse_statfs(&buf, 3);
        assert_eq!(fs.len(), 1);
        assert_eq!(&*fs[0].mount, "/");
        assert_eq!(fs[0].total, 4096 * 1000);
        assert_eq!(fs[0].avail, 4096 * 400);
    }

    #[test]
    fn the_live_filesystems_include_a_sized_root() {
        // The layout check that licenses every offset above: a machine always
        // has a filesystem mounted at `/` and it is never empty.
        let fs = filesystems().expect("no filesystems, so the statfs layout is wrong");
        let root = fs.iter().find(|f| &*f.mount == "/").expect("no root");
        assert!(root.total > 0);
        assert!(root.avail <= root.total);
        assert!(fs.iter().all(|f| f.total > 0));
    }

    #[test]
    fn our_own_thread_count_agrees_with_ps() {
        // Checked against an independent reader, the way the start times are
        // checked against `ps -o lstart`. A delta test is not enough: every
        // neighbouring field returns *a* number, and `pti_csw` four bytes
        // earlier also climbs when threads are started, so "it went up by
        // roughly the right amount" accepts it. Measured on this machine while
        // holding 64 threads, `ps -M` said 65 and the fields read:
        //
        // ```text
        //   pti_policy 1   pti_csw 797   pti_threadnum 65   pti_numrunning 1
        //   pti_priority 20   pti_syscalls_unix 2591
        // ```
        //
        // Only one of those is a thread count, and only a test that knows the
        // answer can say which.
        let me = std::process::id() as i32;

        // Sixty-four of them, so the expected figure is nowhere near the small
        // numbers several other fields hold. Blocked on a barrier rather than
        // spinning, so they are threads without also being *running* threads —
        // `pti_numrunning` is the very next field along.
        let gate = std::sync::Arc::new(std::sync::Barrier::new(65));
        let up = std::sync::Arc::new(std::sync::Barrier::new(65));
        let mut handles = Vec::new();
        for _ in 0..64 {
            let (gate, up) = (gate.clone(), up.clone());
            handles.push(std::thread::spawn(move || {
                up.wait();
                gate.wait();
            }));
        }
        up.wait();

        // Bracketed, because `ps` is a fork and an exec and takes milliseconds,
        // and libtest is running three hundred other cases in this same process
        // meanwhile — each free to start and join threads of its own. A single
        // reading either side of that gap drifts by more than any tolerance
        // tight enough to be worth having: an earlier version compared one
        // reading with a tolerance of 8 and failed one run in three, at 68
        // against 79.
        // Up to three attempts, and this is a retry of the *measurement*, not
        // of the verdict. Reading a live system against another live reader is
        // sampling, and a sample can be spoiled by something outside the test —
        // I saw one failure in roughly seventy runs that I could not reproduce
        // in seventy more, on a machine that had a second `cargo test` on it at
        // the time. A wrong offset is not sampling error: it is out by forty or
        // more, deterministically, and fails all three attempts. So the loop
        // buys robustness against load without buying tolerance of a bug.
        let mut attempts = Vec::new();
        for _ in 0..3 {
            let before = threads(me);
            let ps = std::process::Command::new("ps")
                .args(["-M", "-p", &me.to_string()])
                .output();
            let after = threads(me);
            attempts.push((before, ps, after));
            if let Some((Some(b), Ok(p), Some(a))) = attempts.last() {
                let n = String::from_utf8_lossy(&p.stdout)
                    .lines()
                    .count()
                    .saturating_sub(1) as u32;
                if (b.min(a).saturating_sub(8)..=b.max(a) + 8).contains(&n) {
                    break;
                }
            }
        }
        let (before, ps, after) = attempts.pop().expect("no attempt was made");

        // Released before anything can panic. A failed assertion above this
        // line would leave 64 threads parked on the barrier for the rest of the
        // suite, inflating exactly the figure under test.
        gate.wait();
        for h in handles {
            h.join().unwrap();
        }

        let ps = ps.expect("ps -M");
        // One header line, then one line per thread.
        let expected = String::from_utf8_lossy(&ps.stdout)
            .lines()
            .count()
            .saturating_sub(1) as u32;
        let (before, after) = (
            before.expect("no thread count for our own process"),
            after.expect("no thread count for our own process"),
        );

        assert!(
            expected >= 60,
            "ps saw {expected} threads, so the 64 never started and this proves nothing"
        );
        // `ps` looked somewhere inside the bracket, so its answer has to land
        // inside it too, give or take the threads that came and went while the
        // three readings were taken. Still nothing like loose enough to admit a
        // neighbouring field: those are out by forty or more.
        let lo = before.min(after).saturating_sub(8);
        let hi = before.max(after) + 8;
        assert!(
            (lo..=hi).contains(&expected),
            "ps -M counted {expected} threads; we read {before} before it and {after} after"
        );
    }

    #[test]
    fn a_process_that_is_not_ours_reports_nothing_rather_than_one() {
        // A pid that cannot exist, whoever is asking.
        assert_eq!(threads(-1), None);

        // pid 1 is launchd, owned by root. A `1` here would be a fabricated
        // figure sitting next to a CPU percentage that can contradict it — but
        // only an unprivileged caller is refused, and this project supports
        // running as root precisely so that more of the table becomes readable.
        // Asserting unconditionally would fail under `sudo cargo test`.
        if unsafe { geteuid() } != 0 {
            assert_eq!(threads(1), None, "a thread count was invented for launchd");
        }
    }

    #[test]
    fn a_record_that_reads_back_our_own_pid_is_accepted() {
        assert_eq!(accept(648, &record(648, 4242), 4242), Some(648));
    }

    #[test]
    fn a_record_laid_out_differently_is_refused() {
        // The whole safety argument. If the pid field is not where this module
        // believes, neither is the start time, and every token it produced
        // would be a number read out of some unrelated field.
        assert_eq!(
            accept(648, &record(648, 9999), 4242),
            None,
            "a foreign layout was accepted"
        );
    }

    #[test]
    fn a_record_of_an_implausible_size_is_refused() {
        // Belt to the pid check's braces: a size outside the range means this
        // is not the structure at all, whatever happens to sit at offset 40.
        assert_eq!(
            accept(0, &record(648, 4242), 4242),
            None,
            "an empty record was accepted"
        );
        assert_eq!(
            accept(1 << 20, &record(648, 4242), 4242),
            None,
            "an enormous record was accepted"
        );
        // Short buffer, plausible length: the kernel said more than it wrote.
        assert_eq!(
            accept(648, &record(OFF_PID + 4, 4242), 4242),
            None,
            "a truncated read was accepted"
        );
    }
}
