//! Process start times from `sysctl(KERN_PROC_ALL)`.
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
//! That check is also what makes this safe to compile for every non-Linux
//! target rather than macOS alone. Another BSD has its own `kinfo_proc`, the
//! pid would not read back, and the probe returns `None` — the same path as a
//! macOS that changed the layout out from under us.

use std::collections::HashMap;
use std::ffi::c_void;

// libSystem, already linked by std. Declared here rather than taking a
// dependency on `libc` for one function.
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
        // processes in that window would be remarkable; if it happens the table
        // is short for one sample and complete on the next.
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

#[cfg(test)]
mod tests {
    use super::*;

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
        let distinct: std::collections::HashSet<_> = starts.values().collect();
        assert_eq!(distinct.len(), starts.len(), "two processes share a token");
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
