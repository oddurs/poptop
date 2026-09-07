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
    ///
    /// Callers want [`Collector::sample`]; this is the half a backend writes.
    fn collect(&mut self, needs: Needs) -> std::io::Result<Sample>;

    /// One snapshot, with the rules that hold on every platform applied.
    ///
    /// Provided rather than implemented, so a third backend gets them by
    /// existing. The clamp below was Linux-only for a while and the asymmetry
    /// was invisible: nothing said whether macOS did not need it or had merely
    /// forgotten it.
    fn sample(&mut self, needs: Needs) -> std::io::Result<Sample> {
        let mut s = self.collect(needs)?;
        if let Some(ceiling) = s.cpu_ceiling() {
            for p in &mut s.procs {
                p.cpu = p.cpu.min(ceiling);
            }
        }
        Ok(s)
    }

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
mod kinfo;
#[cfg(not(target_os = "linux"))]
use darwin as backend;
#[cfg(not(target_os = "linux"))]
pub use darwin::SysinfoCollector as Platform;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sample::ProcSample;
    use std::sync::Arc;

    /// A backend that reports whatever it is told to, so the rules the trait
    /// applies can be tested without a platform under them. Runs on both.
    struct Fake(Sample);

    impl Collector for Fake {
        fn collect(&mut self, _: Needs) -> std::io::Result<Sample> {
            Ok(self.0.clone())
        }
    }

    fn sample_of(cores: usize, cpus: &[f32]) -> Sample {
        Sample {
            cpu_per_core: vec![0.0; cores],
            procs: cpus
                .iter()
                .enumerate()
                .map(|(i, &cpu)| ProcSample {
                    pid: i as i32 + 1,
                    ppid: 1,
                    name: Arc::from("x"),
                    user: Arc::from("root"),
                    cpu,
                    rss: 0,
                    threads: 1,
                    state: 'R',
                    started: Some(1),
                    io: None,
                })
                .collect(),
            ..Sample::empty()
        }
    }

    #[test]
    fn every_backend_gets_the_cpu_ceiling_whether_it_asked_or_not() {
        // The point of the item this came from: the clamp was in one backend
        // and not the other, and nothing said which was right.
        let mut c = Fake(sample_of(4, &[50.0, 399.9, 400.1, 500_000.0]));
        let s = c.sample(Needs::default()).unwrap();
        let cpus: Vec<f32> = s.procs.iter().map(|p| p.cpu).collect();
        assert_eq!(cpus, vec![50.0, 399.9, 400.0, 400.0]);
    }

    #[test]
    fn a_machine_with_no_known_core_count_is_left_alone() {
        // Clamping to a ceiling of zero would report every process as idle,
        // which is the fabricated zero this tool refuses everywhere else.
        let mut c = Fake(sample_of(0, &[250.0]));
        let s = c.sample(Needs::default()).unwrap();
        assert_eq!(s.procs[0].cpu, 250.0, "an unknown ceiling became zero");
        assert_eq!(sample_of(0, &[]).cpu_ceiling(), None);
    }

    #[test]
    fn the_ceiling_is_one_core_worth_per_core() {
        assert_eq!(sample_of(1, &[]).cpu_ceiling(), Some(100.0));
        assert_eq!(sample_of(14, &[]).cpu_ceiling(), Some(1400.0));
    }
}
