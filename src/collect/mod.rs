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
    /// Which optional sources to gather.
    wanted: u32,
    /// Which sample this is, for the sources that are not read every time.
    tick: u64,
}

/// One optional thing a sample can gather, with what it costs and how often it
/// is worth reading.
///
/// A single `bool` was the whole capability model, added because per-process IO
/// costs a file read per process. It cannot express "read this every tenth
/// sample", "read this only while its panel is open", or "stop reading this
/// because it costs more than the interval" — and v2.1 adds subsystems whose
/// costs differ by two orders of magnitude.
///
/// Three ad-hoc answers to the same question already exist in this codebase:
/// the IO probe withdraws its columns when most are unreadable, the
/// command-line cache re-reads on a slot staggered by pid, and the clock
/// ceiling rescans its policy set once a minute. This is where the third of
/// those now lives, and where the rest belong.
/// How much of each thing the last sample found, so a cost per unit can be
/// turned into a cost.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Size {
    pub procs: u64,
    pub tasks: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Source {
    /// Per-process disk throughput, from `/proc/<pid>/io`.
    Io,
    /// Per-thread rows, from `/proc/<pid>/task/<tid>/stat`.
    Threads,
    /// The set of cpufreq policies, which is not the ceiling itself — that is
    /// read every sample — but the hardware maximum each policy is measured
    /// against. CPU hotplug is routine on cloud instances and a driver can load
    /// after the tool starts, so never rescanning costs a machine that gained a
    /// policy the figure entirely; rescanning every sample is a directory walk
    /// for an answer that changes about once a day.
    ClockPolicies,
}

impl Source {
    pub const ALL: [Source; 3] = [Source::Io, Source::Threads, Source::ClockPolicies];

    /// What to call it when poptop has to say it stopped reading it.
    pub fn label(self) -> &'static str {
        match self {
            Source::Io => "per-process disk IO",
            Source::Threads => "threads",
            Source::ClockPolicies => "clock policies",
        }
    }

    /// Roughly what one unit of this costs to read, measured rather than
    /// guessed, so a budget can drop the most expensive thing first instead of
    /// the most recently added.
    ///
    /// Measured on Linux at the sample sizes in the item: per-process IO is one
    /// extra file read per process; a thread is a file read of its own, with a
    /// directory read per multi-threaded process amortised into it.
    pub fn nanos_each(self) -> u64 {
        match self {
            Source::Io => 5_700,
            Source::Threads => 3_100,
            // A directory walk of `/sys/devices/system/cpu`, once a minute.
            // Counted per sample rather than per unit because there is one of
            // it.
            Source::ClockPolicies => 200_000,
        }
    }

    /// Whether its cost grows with the size of the machine.
    ///
    /// The budget only ever gives up sources for which this is true, and the
    /// reason is not tidiness: a fixed cost read once a minute cannot be why a
    /// sample ran long, so dropping it would cost a figure and fix nothing.
    ///
    /// It also makes [`Self::nanos_each`] comparable. Per-unit and per-sample
    /// costs are different quantities, and `max_by_key` over both of them
    /// together picked the clock policy walk — a directory listing once a
    /// minute — over per-process IO on four hundred processes.
    pub fn scales(self) -> bool {
        match self {
            Source::Io | Source::Threads => true,
            Source::ClockPolicies => false,
        }
    }

    /// Roughly what this source costs on a machine of this size.
    pub fn total_nanos(self, size: Size) -> u64 {
        let units = match self {
            Source::Io => size.procs,
            Source::Threads => size.tasks,
            Source::ClockPolicies => 1,
        };
        self.nanos_each().saturating_mul(units)
    }

    /// How many samples apart this is worth reading. One means every sample.
    pub fn every(self) -> u64 {
        match self {
            Source::Io | Source::Threads => 1,
            Source::ClockPolicies => 60,
        }
    }

    fn bit(self) -> u32 {
        1 << (self as u32)
    }
}

impl Needs {
    /// Nothing optional, at sample zero.
    pub const NONE: Needs = Needs { wanted: 0, tick: 0 };

    pub fn at(tick: u64) -> Self {
        Needs { wanted: 0, tick }
    }

    pub fn with(mut self, s: Source) -> Self {
        self.wanted |= s.bit();
        self
    }

    pub fn without(mut self, s: Source) -> Self {
        self.wanted &= !s.bit();
        self
    }

    /// Whether this sample should gather it: asked for, and due.
    ///
    /// The two are separate questions and both have to be yes. A source nobody
    /// wants is never due, and a source everybody wants is still only read on
    /// its cadence.
    pub fn wants(self, s: Source) -> bool {
        self.wanted & s.bit() != 0 && self.due(s)
    }

    /// Whether this sample is one of the ones this source is read on.
    ///
    /// Split out because it is the part worth testing directly: a cadence with
    /// no consumer yet is still a rule, and `every() == 1` must mean every
    /// sample rather than every sample but the first.
    pub fn due(self, s: Source) -> bool {
        self.tick.is_multiple_of(s.every())
    }

    /// Whether it was asked for at all, cadence aside.
    pub fn asked(self, s: Source) -> bool {
        self.wanted & s.bit() != 0
    }

    /// The most expensive source in this set, which is the one a budget should
    /// give up first.
    ///
    /// The source costing the most on *this* machine, which is the one a budget
    /// should give up first.
    ///
    /// Weighted by how many units there actually are, not by cost per unit.
    /// Per-process IO is 5.7us a process and a thread is 3.1us, so per unit IO
    /// looks dearer — but a box with 400 processes has some 3200 threads, which
    /// makes threads about 9.9ms against IO's 2.3ms. Ranking on the unit cost
    /// gives up the cheaper source, stays over budget, and costs the reader the
    /// IO columns for nothing before three more strikes finally reach the
    /// source that was actually responsible.
    ///
    /// Only sources whose cost scales with the machine; see [`Source::scales`].
    pub fn costliest(self, size: Size) -> Option<Source> {
        Source::ALL
            .into_iter()
            .filter(|s| self.asked(*s) && s.scales())
            .max_by_key(|s| s.total_nanos(size))
    }
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

/// Which optional sources this backend actually reads.
///
/// Declared by the backend rather than inferred, so nothing can ask for a
/// source that will never arrive — and so the budget cannot spend three strikes
/// "giving up" something that was costing nothing.
pub use backend::SUPPORTED;

/// Whether a filesystem lives in RAM rather than on a device.
///
/// Excluded from the capacity figures by name rather than by measurement,
/// because they report perfectly real sizes — a full `tmpfs` is a memory
/// problem, which the header already reports, and counting it here would spend
/// a second figure saying the same bytes are gone.
pub fn is_ram_backed(kind: &str) -> bool {
    matches!(kind, "tmpfs" | "ramfs" | "devtmpfs" | "devfs")
}

/// Whether a filesystem lives at the other end of a network.
///
/// Excluded because `statfs` on an unresponsive mount blocks until it answers,
/// and a monitor that freezes when the fileserver does is worse than one that
/// does not mention the fileserver. macOS avoids the question entirely by
/// asking `getfsstat` not to wait; Linux has no such flag, so this is a rule
/// about names and it is worth knowing it is there.
///
/// Linux only, for that reason: there is nothing for the other backend to use
/// it for.
#[cfg(target_os = "linux")]
pub fn is_network_fs(kind: &str) -> bool {
    matches!(
        kind,
        "nfs"
            | "nfs4"
            | "cifs"
            | "smbfs"
            | "smb3"
            | "afs"
            | "ceph"
            | "glusterfs"
            | "9p"
            | "ocfs2"
            | "gfs2"
            | "lustre"
            | "beegfs"
            | "davfs"
            // An automount point that has not mounted yet appears as `autofs`,
            // not as the type behind it — and resolving its path is what
            // triggers the mount. Asking it how full it is *is* the thing that
            // blocks.
            | "autofs"
    ) || kind.starts_with("fuse")
}

/// Collapse filesystems that are the same physical space seen more than once.
///
/// Two shapes of duplicate, and one rule for both. A bind mount puts one
/// filesystem at several paths, reporting identical numbers each time. APFS
/// puts every volume in one container, so `/`, `/System/Volumes/Data`,
/// `/System/Volumes/VM` and four others all report the same size and differ
/// only slightly in what is left, because each volume reserves a little.
///
/// Read-only volumes are folded in for their *name* and dropped for their
/// *space*. A filesystem nothing can be written to cannot fill up — a squashfs
/// snap has no available space by construction and would read as 100% full
/// forever — but since Catalina the volume macOS mounts at `/` is the sealed,
/// read-only system one, and the writable half is `/System/Volumes/Data`.
/// Filtering before merging removed the root of every Mac; filtering after it
/// keeps the name a reader knows and the number that can actually run out.
///
/// So the key is the total, and the survivor takes the **shortest mount point**
/// and the **least available space**: the name a reader recognises, and the
/// number that decides whether anything is wrong. Reporting
/// `/System/Volumes/VM 86.1% full` was accurate and no use to anybody.
///
/// Keyed on the device rather than on the size. Two identically-sized logical
/// volumes are a normal way to provision a machine, and merging them on size
/// would not rename one — it would delete it, and then report the survivor's
/// figure under the wrong mount point.
pub fn merge_filesystems(
    all: Vec<(String, bool, crate::sample::FsStat)>,
) -> Vec<crate::sample::FsStat> {
    struct Group {
        key: String,
        writable: bool,
        fs: crate::sample::FsStat,
    }
    let mut out: Vec<Group> = Vec::new();
    for (dev, writable, f) in all {
        let key = container_of(&dev).to_string();
        match out.iter_mut().find(|g| g.key == key) {
            Some(g) => {
                // The shortest name across every volume, writable or not. On
                // macOS the sealed system volume is the one mounted at `/` and
                // the writable half is `/System/Volumes/Data`, so taking the
                // name only from writable members would report the machine's
                // disk under a path nobody recognises.
                if f.mount.len() < g.fs.mount.len() {
                    g.fs.mount = f.mount;
                }
                // The space, from the writable members only — that is the space
                // that can run out.
                if writable {
                    if g.writable {
                        g.fs.avail = g.fs.avail.min(f.avail);
                    } else {
                        g.fs.avail = f.avail;
                        g.fs.total = f.total;
                    }
                    g.writable = true;
                }
            }
            None => out.push(Group {
                key,
                writable,
                fs: f,
            }),
        }
    }
    // A container nothing can be written to cannot fill up. Dropped after the
    // merge rather than before it, because a container with one read-only
    // volume and one writable one is a machine's disk, not a read-only mount.
    out.into_iter()
        .filter(|g| g.writable)
        .map(|g| g.fs)
        .collect()
}

/// Accept a filesystem list only if it contains a sized root.
///
/// The check that licenses reading capacity out of a structure neither backend
/// declares. Every machine has a filesystem mounted at `/` and it is never
/// empty — so if that one was rejected while some other mount happened to yield
/// a plausible-looking block size, the offsets are wrong and every figure that
/// did come back is garbage.
///
/// Asserting merely that *something* came back is not the same check, and was
/// what this did first: entries with no size are already dropped, so it reduced
/// to "the list is not empty".
pub fn with_a_root(out: Vec<crate::sample::FsStat>) -> Option<Vec<crate::sample::FsStat>> {
    out.iter()
        .any(|f| &*f.mount == "/" && f.total > 0)
        .then_some(out)
}

/// The storage a device name lives on, for deciding whether two filesystems are
/// the same space.
///
/// `/dev/disk3s1s1` and `/dev/disk3s5` are two APFS volumes in one container and
/// share what is left; `/dev/vda1` and `/dev/vda2` are two partitions that do
/// not. So the slice suffix is stripped only from Darwin's `diskN` names, where
/// it denotes a volume inside a container, and left alone everywhere else.
fn container_of(dev: &str) -> &str {
    let Some(rest) = dev.strip_prefix("/dev/disk") else {
        return dev;
    };
    let n = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    &dev[.."/dev/disk".len() + n]
}

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
mod procinfo;
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
                    threads: Some(1),
                    state: 'R',
                    started: Some(1),
                    cmd: None,
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

#[cfg(test)]
mod fs_tests {
    use super::*;
    use crate::sample::FsStat;
    use std::sync::Arc;

    fn fs(dev: &str, mount: &str, total: u64, avail: u64) -> (String, bool, FsStat) {
        rofs(dev, mount, total, avail, true)
    }

    fn rofs(
        dev: &str,
        mount: &str,
        total: u64,
        avail: u64,
        writable: bool,
    ) -> (String, bool, FsStat) {
        (
            dev.to_string(),
            writable,
            FsStat {
                mount: Arc::from(mount),
                total,
                avail,
            },
        )
    }

    #[test]
    fn a_read_only_filesystem_cannot_fill_up() {
        // A squashfs snap has no available space by construction and would read
        // as 100% full forever. An Ubuntu machine carries twenty-odd, and each
        // is its own device, so the real root could never be reported past
        // them.
        let out = merge_filesystems(vec![
            rofs("/dev/loop3", "/snap/core22/1234", 1000, 0, false),
            fs("/dev/vda1", "/", 1000, 400),
        ]);
        assert_eq!(out.len(), 1);
        assert_eq!(&*out[0].mount, "/");
    }

    #[test]
    fn a_sealed_system_volume_lends_its_name_to_the_writable_half() {
        // Since Catalina the volume macOS mounts at `/` is the read-only system
        // one and the writable half is `/System/Volumes/Data`. Filtering
        // read-only volumes before merging removed the root of every Mac;
        // filtering after keeps the name a reader knows and the space that can
        // actually run out.
        let out = merge_filesystems(vec![
            rofs("/dev/disk3s1s1", "/", 1000, 900, false),
            fs("/dev/disk3s5", "/System/Volumes/Data", 1000, 120),
        ]);
        assert_eq!(out.len(), 1, "the machine's own disk disappeared");
        assert_eq!(&*out[0].mount, "/", "the writable half's obscure name won");
        assert_eq!(out[0].avail, 120, "the sealed volume's free space was used");
    }

    #[test]
    fn one_filesystem_at_several_paths_is_reported_once() {
        // A bind mount. This container has `/dev/vda1` at three paths, all
        // reporting the same numbers.
        let out = merge_filesystems(vec![
            fs("/dev/vda1", "/etc/resolv.conf", 1000, 400),
            fs("/dev/vda1", "/", 1000, 400),
            fs("/dev/vda1", "/etc/hosts", 1000, 400),
        ]);
        assert_eq!(out.len(), 1);
        assert_eq!(&*out[0].mount, "/", "the longest path was kept");
    }

    #[test]
    fn volumes_sharing_a_container_report_the_worst_of_them() {
        // APFS. Seven volumes in one container, each reserving a little, so
        // they differ slightly on what is left. Reporting
        // `/System/Volumes/VM 86.1% full` was accurate and no use to anybody,
        // and reporting the roomiest would have been worse than no use.
        let out = merge_filesystems(vec![
            fs("/dev/disk3s1s1", "/", 1000, 400),
            fs("/dev/disk3s6", "/System/Volumes/VM", 1000, 130),
            fs("/dev/disk3s5", "/System/Volumes/Data", 1000, 390),
        ]);
        assert_eq!(out.len(), 1, "one container was reported three times");
        assert_eq!(&*out[0].mount, "/", "an obscure volume was named");
        assert_eq!(out[0].avail, 130, "the roomiest volume was reported");
    }

    #[test]
    fn two_disks_of_equal_size_are_two_disks() {
        // Identically-sized logical volumes are a normal way to provision a
        // machine. Keyed on size, the second would not be renamed — it would be
        // deleted, and the survivor's figure reported under the wrong mount.
        let out = merge_filesystems(vec![
            fs("/dev/vg0/lv1", "/srv/a", 1000, 400),
            fs("/dev/vg0/lv2", "/srv/b", 1000, 50),
        ]);
        assert_eq!(out.len(), 2, "an identically-sized disk disappeared");
    }

    #[test]
    fn separate_containers_stay_apart() {
        let out = merge_filesystems(vec![
            fs("/dev/disk1s1", "/System/Volumes/xarts", 500, 480),
            fs("/dev/disk3s1s1", "/", 1000, 400),
        ]);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn a_partition_is_not_a_volume_in_a_container() {
        // `/dev/disk3s1` and `/dev/disk3s5` are volumes in one APFS container;
        // `/dev/vda1` and `/dev/vda2` are two partitions that are not.
        assert_eq!(container_of("/dev/disk3s1s1"), "/dev/disk3");
        assert_eq!(container_of("/dev/disk3s5"), "/dev/disk3");
        assert_eq!(container_of("/dev/disk11s2"), "/dev/disk11");
        assert_eq!(container_of("/dev/vda1"), "/dev/vda1");
        assert_eq!(container_of("/dev/nvme0n1p2"), "/dev/nvme0n1p2");
        assert_eq!(container_of("overlay"), "overlay");
    }

    #[test]
    fn a_filesystem_list_with_no_root_is_refused() {
        // Every machine has a sized filesystem at `/`. A list without one means
        // the offsets are not pointing where this code believes.
        let ok = vec![fs("/dev/vda1", "/", 1000, 400).2];
        assert!(with_a_root(ok).is_some());
        let no_root = vec![fs("/dev/vda2", "/home", 1000, 400).2];
        assert_eq!(
            with_a_root(no_root),
            None,
            "a list with no root was accepted"
        );
        assert_eq!(with_a_root(Vec::new()), None);
        // A root with no size is not a root that was read.
        let empty_root = vec![fs("/dev/vda1", "/", 0, 0).2];
        assert_eq!(with_a_root(empty_root), None);
    }

    #[test]
    fn what_lives_in_memory_is_not_a_disk() {
        for kind in ["tmpfs", "ramfs", "devtmpfs", "devfs"] {
            assert!(is_ram_backed(kind), "{kind} was counted as disk");
        }
        assert!(!is_ram_backed("ext4"));
        assert!(!is_ram_backed("apfs"));
    }
}
