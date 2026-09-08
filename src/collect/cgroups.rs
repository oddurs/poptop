//! Per-cgroup utilisation and pressure, from cgroup v2.
//!
//! poptop reports PSI for the machine. On a container host that answers
//! "something is stalled on IO" and not "the thing stalled on IO is this pod",
//! which is the question — and per-cgroup pressure is the only metric that
//! *attributes* a stall rather than reporting one.
//!
//! # Why this is the expensive subsystem
//!
//! `/sys/fs/cgroup` is a tree and every node has its own `cpu.stat`,
//! `memory.current`, `io.stat` and three pressure files. A Kubernetes node with
//! a thousand cgroups is six thousand reads a sample. This is what the cost
//! model in the collector exists for: it is declared, gated, bounded by depth,
//! and the budget can take it away.
//!
//! # What the figures mean
//!
//! In cgroup v2 `cpu.stat` and `memory.current` are already **subtree totals** —
//! a parent's usage includes its children's. So the rollup is the kernel's
//! arithmetic, not poptop's, and a parent will read higher than any one child
//! rather than equal to the sum of the rows below it.

use crate::sample::{CgroupStat, Pressure, Stall};
use std::collections::HashMap;
use std::fs;
use std::sync::Arc;

/// Where the unified hierarchy is mounted.
const ROOT: &str = "/sys/fs/cgroup";

/// The bounds the walk runs under. Stated in [`crate::collect`] because the
/// panel has to describe a stored sample by the bounds it was collected under.
pub use super::{CGROUP_DEPTH as DEFAULT_DEPTH, CGROUP_MAX_NODES as MAX_NODES};

/// Whether this machine publishes a unified hierarchy at all.
///
/// cgroup v1 has no `cpu.stat` in this shape, no unified tree and no PSI, so
/// there is nothing to walk. Saying so beats showing an empty table.
pub fn v2_available() -> bool {
    fs::metadata(format!("{ROOT}/cgroup.controllers")).is_ok()
}

/// Cumulative counters from the previous sample, for the ones that are rates.
#[derive(Default)]
pub struct Prev {
    cpu_usec: HashMap<Arc<str>, u64>,
    io_bytes: HashMap<Arc<str>, (u64, u64)>,
}

/// One `some`/`full` pressure file.
///
/// Both lines carry `avg10`, `avg60`, `avg300` and a cumulative `total`. The
/// ten-second average is what poptop shows everywhere else, so it is what this
/// reads — a figure that is already an average over a window the reader can
/// hold in their head.
fn parse_pressure(text: &str) -> Option<Stall> {
    let mut some = None;
    let mut full = 0.0;
    for line in text.lines() {
        let avg10 = line
            .split_whitespace()
            .find_map(|f| f.strip_prefix("avg10="))
            .and_then(|v| v.parse::<f32>().ok());
        match (line.split_whitespace().next(), avg10) {
            (Some("some"), Some(v)) => some = Some(v),
            (Some("full"), Some(v)) => full = v,
            _ => {}
        }
    }
    // `full` alone is not a reading: every pressure file has a `some` line, and
    // one without it is a file this parser does not understand.
    Some(Stall { some: some?, full })
}

fn read_num(path: &str) -> Option<u64> {
    fs::read_to_string(path).ok()?.trim().parse().ok()
}

/// `usage_usec` from a `cpu.stat`.
fn parse_cpu_stat(text: &str) -> Option<u64> {
    text.lines()
        .find_map(|l| l.strip_prefix("usage_usec "))
        .and_then(|v| v.trim().parse().ok())
}

/// Read and write bytes, summed across devices.
fn parse_io_stat(text: &str) -> (u64, u64) {
    let mut read = 0;
    let mut write = 0;
    for line in text.lines() {
        for field in line.split_whitespace() {
            if let Some(v) = field.strip_prefix("rbytes=") {
                read += v.parse::<u64>().unwrap_or(0);
            } else if let Some(v) = field.strip_prefix("wbytes=") {
                write += v.parse::<u64>().unwrap_or(0);
            }
        }
    }
    (read, write)
}

/// `cpu.max` as a percentage of one core, or `None` for `max`.
fn parse_cpu_max(text: &str) -> Option<f32> {
    let mut it = text.split_whitespace();
    let quota: f64 = it.next()?.parse().ok()?;
    let period: f64 = it.next()?.parse().ok()?;
    (period > 0.0).then_some((quota / period * 100.0) as f32)
}

/// How stalled a cgroup is, for the ordering the view exists to provide.
///
/// The worst of the three, not two. Leaving memory out sorts a cgroup thrashing
/// on reclaim with quiet CPU and IO to the bottom — and since the table shows
/// what fits and does not scroll, the bottom means off screen, in the one view
/// whose entire purpose is finding what is stalled.
///
/// A node with no pressure reading sorts last. It is not stalled as far as
/// anyone can tell, and the `—` in its column is what says nobody can tell.
pub fn pressure_key(c: &CgroupStat) -> f32 {
    c.pressure
        .map_or(0.0, |p| p.cpu.some.max(p.io.some).max(p.memory.some))
}

/// Walk the tree and read every node's figures.
///
/// `elapsed_secs` turns the cumulative counters into rates. Depth is counted
/// from the root, which is itself depth zero.
pub fn read(prev: &mut Prev, elapsed_secs: f64, depth: u32) -> Vec<CgroupStat> {
    let mut out = Vec::new();
    let mut seen_cpu = HashMap::new();
    let mut seen_io = HashMap::new();
    // Breadth-first, so a bound reached partway through leaves whole levels
    // rather than one arbitrary branch followed to the bottom.
    let mut queue = vec![(String::new(), 0u32)];
    let mut head = 0;
    while head < queue.len() && out.len() < MAX_NODES {
        let (rel, d) = queue[head].clone();
        head += 1;
        let dir = format!("{ROOT}{rel}");

        if d < depth
            && let Ok(entries) = fs::read_dir(&dir)
        {
            for e in entries.flatten() {
                if e.file_type().is_ok_and(|t| t.is_dir())
                    && let Some(name) = e.file_name().to_str()
                {
                    queue.push((format!("{rel}/{name}"), d + 1));
                }
            }
        }

        let name: Arc<str> = Arc::from(if rel.is_empty() { "/" } else { rel.as_str() });
        let cpu_usec = fs::read_to_string(format!("{dir}/cpu.stat"))
            .ok()
            .and_then(|t| parse_cpu_stat(&t));
        // A rate needs two readings. A node seen for the first time reports no
        // CPU rather than its whole lifetime's usage divided by one interval.
        let cpu = cpu_usec.and_then(|now| {
            seen_cpu.insert(name.clone(), now);
            let was = prev.cpu_usec.get(&name)?;
            Some(((now.saturating_sub(*was) as f64 / 1e6) / elapsed_secs * 100.0) as f32)
        });
        if let Some(now) = cpu_usec {
            seen_cpu.insert(name.clone(), now);
        }

        let io = fs::read_to_string(format!("{dir}/io.stat"))
            .ok()
            .map(|t| parse_io_stat(&t));
        let io_rate = io.and_then(|(r, w)| {
            seen_io.insert(name.clone(), (r, w));
            let (wr, ww) = prev.io_bytes.get(&name)?;
            Some((
                ((r.saturating_sub(*wr) as f64) / elapsed_secs) as u64,
                ((w.saturating_sub(*ww) as f64) / elapsed_secs) as u64,
            ))
        });
        if let Some(v) = io {
            seen_io.insert(name.clone(), v);
        }

        let pressure = |what: &str| {
            fs::read_to_string(format!("{dir}/{what}.pressure"))
                .ok()
                .and_then(|t| parse_pressure(&t))
        };
        let pressure = match (pressure("cpu"), pressure("io"), pressure("memory")) {
            (Some(cpu), Some(io), Some(memory)) => Some(Pressure { cpu, io, memory }),
            // Partial is not a reading. `cgroup.pressure` can be switched off
            // per node, and a `Pressure` with two real figures and one zero
            // would say the third subsystem never stalls.
            _ => None,
        };

        out.push(CgroupStat {
            path: name,
            depth: d,
            cpu,
            cpu_max: fs::read_to_string(format!("{dir}/cpu.max"))
                .ok()
                .and_then(|t| parse_cpu_max(&t)),
            mem: read_num(&format!("{dir}/memory.current")),
            mem_max: fs::read_to_string(format!("{dir}/memory.max"))
                .ok()
                .and_then(|t| t.trim().parse().ok()),
            read: io_rate.map(|(r, _)| r),
            write: io_rate.map(|(_, w)| w),
            pressure,
            procs: fs::read_to_string(format!("{dir}/cgroup.procs"))
                .ok()
                .map(|t| t.lines().count() as u32),
        });
    }
    prev.cpu_usec = seen_cpu;
    prev.io_bytes = seen_io;
    // Deepest pressure first: the reason to open this view is to find what is
    // stalled, and a parent's pressure is its children's.
    out.sort_by(|a, b| {
        pressure_key(b)
            .total_cmp(&pressure_key(a))
            .then(a.path.cmp(&b.path))
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pressure_file_is_read_as_some_and_full() {
        let s = parse_pressure(
            "some avg10=1.25 avg60=0.40 avg300=0.10 total=616\n\
             full avg10=0.75 avg60=0.20 avg300=0.05 total=96\n",
        )
        .expect("did not parse");
        assert_eq!(s.some, 1.25);
        assert_eq!(s.full, 0.75);
    }

    #[test]
    fn a_pressure_file_with_no_some_line_is_not_a_reading() {
        // `full` alone would render as "nothing is stalled" for a file this
        // parser does not understand.
        assert!(parse_pressure("full avg10=0.75 total=96\n").is_none());
        assert!(parse_pressure("").is_none());
    }

    #[test]
    fn cpu_stat_is_read_past_the_fields_before_it() {
        assert_eq!(
            parse_cpu_stat("usage_usec 17971\nuser_usec 3171\nsystem_usec 14800\n"),
            Some(17971)
        );
        // `user_usec` starts with the same letters as nothing here, but a
        // prefix match on `usage` would find `usage_usec` inside a longer key
        // if one is ever added; the trailing space is what stops that.
        assert_eq!(parse_cpu_stat("nr_periods 0\n"), None);
    }

    #[test]
    fn io_stat_is_summed_across_devices() {
        let (r, w) = parse_io_stat(
            "254:0 rbytes=135168 wbytes=100 rios=3 wios=0\n\
             254:1 rbytes=1000 wbytes=7 rios=1 wios=1\n",
        );
        assert_eq!((r, w), (136_168, 107));
    }

    fn stalled(cpu: f32, io: f32, memory: f32) -> CgroupStat {
        let s = |some| Stall { some, full: 0.0 };
        CgroupStat {
            pressure: Some(Pressure {
                cpu: s(cpu),
                io: s(io),
                memory: s(memory),
            }),
            ..CgroupStat::default()
        }
    }

    #[test]
    fn memory_pressure_counts_towards_being_the_most_stalled() {
        // A cgroup thrashing on reclaim with quiet CPU and IO is exactly the
        // one somebody opened this view to find, and the table shows what fits
        // and does not scroll — so sorting it below the quiet ones puts it off
        // screen.
        assert!(
            pressure_key(&stalled(0.0, 0.0, 61.5)) > pressure_key(&stalled(1.0, 2.0, 0.0)),
            "memory pressure does not count towards the ordering"
        );
        // …and each of the other two on its own still does.
        assert_eq!(pressure_key(&stalled(9.0, 1.0, 2.0)), 9.0);
        assert_eq!(pressure_key(&stalled(1.0, 9.0, 2.0)), 9.0);
        // A node nobody is measuring sorts last rather than first.
        assert_eq!(pressure_key(&CgroupStat::default()), 0.0);
    }

    #[test]
    fn an_unlimited_cpu_max_is_absent_rather_than_infinite() {
        assert_eq!(parse_cpu_max("50000 100000"), Some(50.0));
        assert_eq!(parse_cpu_max("200000 100000"), Some(200.0));
        assert_eq!(parse_cpu_max("max 100000"), None);
    }
}

/// The container id in a `/proc/<pid>/cgroup` path, if there is one.
///
/// Split from the read because it cannot be tested against a machine: no host
/// runs docker, podman, containerd and plain systemd at once, and the paths
/// differ per runtime *and* per cgroup version. So it is a pure function over
/// the file's text, with a fixture for each.
///
/// Returns the id truncated to twelve characters, which is what `docker ps`
/// shows and what atop falls back to. The pod *name* is not in the path at all
/// — atop reads it from the runtime, with superuser — so an id is what poptop
/// can know without asking anybody's permission.
pub fn container_of(text: &str) -> Option<Arc<str>> {
    // v2 is one `0::/path` line; v1 is several `id:controllers:/path`. Both end
    // in the path, and the container component looks the same in either.
    for line in text.lines() {
        let path = line.rsplit(':').next()?;
        for part in path.split('/').rev() {
            if let Some(id) = id_in(part) {
                return Some(Arc::from(id));
            }
        }
    }
    None
}

/// The id inside one path component, if that component names a container.
fn id_in(part: &str) -> Option<&str> {
    // systemd: `docker-<id>.scope`, `libpod-<id>.scope`,
    // `cri-containerd-<id>.scope`, `crio-<id>.scope`.
    let stem = part.strip_suffix(".scope").unwrap_or(part);
    let after = [
        "docker-",
        "libpod-",
        "cri-containerd-",
        "crio-",
        "containerd-",
    ]
    .into_iter()
    .find_map(|p| stem.strip_prefix(p))
    // cgroupfs: the component *is* the id, as under `/docker/<id>`.
    .unwrap_or(stem);
    // A container id is a long hex string. Requiring that is what keeps
    // `session-3.scope` and `user-1000.slice` from being read as containers —
    // and a pod's own `.slice`, whose uuid has dashes in it, is not one either:
    // the container inside it is, and that is the row's answer.
    let hex = after.len() >= 32 && after.chars().all(|c| c.is_ascii_hexdigit());
    hex.then(|| &after[..12])
}

#[cfg(test)]
mod container_tests {
    use super::*;

    /// One fixture per runtime, because no machine has them all.
    const CASES: &[(&str, &str, Option<&str>)] = &[
        (
            "docker, cgroup v2 under systemd",
            "0::/system.slice/docker-9a1f0e4c2b7d8e6f5a4b3c2d1e0f9a8b7c6d5e4f3a2b1c0d9e8f7a6b5c4d3e2f.scope\n",
            Some("9a1f0e4c2b7d"),
        ),
        (
            "docker, cgroupfs",
            "0::/docker/9a1f0e4c2b7d8e6f5a4b3c2d1e0f9a8b7c6d5e4f3a2b1c0d9e8f7a6b5c4d3e2f\n",
            Some("9a1f0e4c2b7d"),
        ),
        (
            "docker, cgroup v1 with a controller per line",
            "12:pids:/docker/9a1f0e4c2b7d8e6f5a4b3c2d1e0f9a8b7c6d5e4f3a2b1c0d9e8f7a6b5c4d3e2f\n\
             11:memory:/docker/9a1f0e4c2b7d8e6f5a4b3c2d1e0f9a8b7c6d5e4f3a2b1c0d9e8f7a6b5c4d3e2f\n",
            Some("9a1f0e4c2b7d"),
        ),
        (
            "podman",
            "0::/machine.slice/libpod-4b3c2d1e0f9a8b7c6d5e4f3a2b1c0d9e8f7a6b5c4d3e2f1a0b9c8d7e6f5a4b.scope\n",
            Some("4b3c2d1e0f9a"),
        ),
        (
            "containerd under kubernetes",
            "0::/kubepods.slice/kubepods-burstable.slice/kubepods-burstable-pod3f2e1d0c_9b8a_7654_3210_fedcba987654.slice/\
             cri-containerd-c0ffee1234567890abcdef1234567890abcdef1234567890abcdef1234567890.scope\n",
            Some("c0ffee123456"),
        ),
        (
            "cri-o",
            "0::/kubepods.slice/kubepods-besteffort.slice/crio-deadbeefcafe1234567890abcdef1234567890abcdef1234567890abcdef12.scope\n",
            Some("deadbeefcafe"),
        ),
        (
            "a login session, which is not a container",
            "0::/user.slice/user-1000.slice/session-3.scope\n",
            None,
        ),
        (
            "a plain system service",
            "0::/system.slice/sshd.service\n",
            None,
        ),
        ("the root cgroup", "0::/\n", None),
        ("nothing at all", "", None),
        (
            // Observed on a Docker Desktop VM: from inside a container the
            // path is namespaced and relative, and carries no id at all. Which
            // is why this parser is tested against fixtures — no machine has
            // every runtime, and the one available here has none of them.
            "a namespaced relative path",
            "0::/../..\n",
            None,
        ),
    ];

    #[test]
    fn a_container_id_is_found_in_every_runtimes_path() {
        for (what, text, want) in CASES {
            assert_eq!(
                container_of(text).as_deref(),
                *want,
                "{what}: parsed the wrong container out of {text:?}"
            );
        }
    }

    #[test]
    fn a_pods_own_slice_is_not_mistaken_for_a_container() {
        // The uuid in `…-pod3f2e1d0c_9b8a_….slice` is long, and without the
        // hex check it reads as an id — so every process in the pod would be
        // labelled with the pod's slice rather than the container it is in.
        assert_eq!(
            container_of(
                "0::/kubepods.slice/kubepods-burstable-pod3f2e1d0c_9b8a_7654_3210_fedcba987654.slice\n"
            ),
            None,
            "a pod slice was read as a container"
        );
    }

    #[test]
    fn a_short_hex_component_is_not_an_id() {
        // `user-1000.slice` and friends are hex-ish and short. The length is
        // what separates them from a container id.
        assert_eq!(container_of("0::/system.slice/abcdef.scope\n"), None);
    }
}
