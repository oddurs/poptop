//! Platform backends.
//!
//! The UI never learns which one it is talking to. On Linux we parse `/proc`
//! ourselves, which is where the interesting work is; on macOS there is no
//! `/proc`, so we lean on `sysinfo` to keep the tool runnable on a dev laptop.

use crate::sample::Sample;

/// Which optional, expensive data this sample should gather.
///
/// htop gates reads behind flags derived from the visible columns
/// (`PROCESS_FLAG_*`), so switching a column off stops the syscalls behind it.
/// Core figures — cpu, memory, rss, name, user, state, threads — are never
/// gated: the timeline and the default table depend on them, so their history
/// has to be complete.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Needs {
    /// Per-process disk throughput. One extra file read per process per sample.
    pub io: bool,
}

/// The fastest this backend can be sampled and still report the truth.
///
/// A property of the backend, not of the tool. The `/proc` floor is a cost
/// argument — a pass is about 1ms at 400 processes, so 50ms spends 2% of a core
/// — while sysinfo's is a correctness one: below its refresh minimum the CPU
/// figures it returns are not noisy, they are wrong. Sharing one constant
/// between them meant applying the Linux reasoning to a platform it was never
/// about.
pub use backend::{MIN_INTERVAL, MIN_INTERVAL_WHY};

pub trait Collector {
    /// Take one snapshot. Backends hold whatever raw counters they need to
    /// turn cumulative kernel numbers into per-interval rates.
    fn sample(&mut self, needs: Needs) -> std::io::Result<Sample>;

    /// What the backend could not determine about this machine, said once at
    /// startup rather than folded into every figure that depends on it.
    ///
    /// A backend that has to assume something is still usable — the assumption
    /// is almost always right — but an assumption nobody is told about is the
    /// same shape as a wrong number, and that is the one thing this tool is
    /// not allowed to produce.
    fn notes(&self) -> Vec<String> {
        Vec::new()
    }
}

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux as backend;
#[cfg(target_os = "linux")]
pub use linux::ProcFs as Platform;

#[cfg(not(target_os = "linux"))]
mod darwin;
#[cfg(not(target_os = "linux"))]
use darwin as backend;
#[cfg(not(target_os = "linux"))]
pub use darwin::SysinfoCollector as Platform;
