---
id: 93
title: 'The Linux backend is tested on one kernel: whatever GitHub runs'
type: chore
status: done
milestone: r3
labels:
- testing
created: 2026-09-18
updated: 2026-09-18
priority: p1
effort: l
area: collect
---

## Problem

The `/proc` backend depends on the kernel version, cgroup v1 or v2, page size (0023 was a 64k-page bug), and whether PSI, taskstats and NFS are present. CI runs `ubuntu-latest`, which is one recent x86_64 kernel with 4k pages and cgroup v2. Every other combination is untested.

## Proposal

Record `/proc` and `/sys` trees (only the files poptop reads) from a handful of real machines: an old LTS kernel without PSI, cgroup v1, arm64 with 64k pages, a container with a restricted `/proc`, and a host with NFS mounts. Check them in as fixtures. Point the collector at a fixture root in tests and assert the parsed sample against values recorded alongside it.

## Acceptance criteria

- [x] The collector can read from a root other than `/` (tests only, or a hidden flag)
- [x] At least five fixture trees covering the combinations above, with a script to record more
- [x] Each fixture has expected values, and a test compares the collector's output to them
- [x] A missing subsystem in a fixture is reported as absent, never as zero

## How it was resolved

**A root other than `/`, in tests only.** Every read of `/proc`, `/sys` and `/etc/passwd` in the Linux collector goes through `collect::at`: 31 call sites, plus `statfs` and the NUMA, cpufreq and cgroup roots. Outside tests `at` returns its argument unchanged and compiles to nothing. In tests it re-roots under a thread-local fixture directory, so a fixture test and the tests reading the real machine can run side by side.

**Recorded, not written by hand.** `tests/fixtures/linux/record` runs `poptop --export=json` under `strace -e trace=%file` and copies every `/proc`, `/sys` and `/etc/passwd` file a call succeeded on. That is exactly what poptop reads on that kernel, with nothing chosen by hand. Directories poptop lists only for their names get a `.keep`, because neither git nor an artifact upload keeps an empty directory. The first recordings lost `/sys/block` and the NUMA nodes that way. The `record-fixtures` workflow runs it on GitHub's runners, triggered by hand or by pushing a branch of that name, and uploads a tarball.

**Expected values worked out independently.** `record` computes `expected` from the recorded files with awk and perl, not with poptop: cores, memory total, available and swap, load, uptime, process count, PSI present, cgroup v2 present, the page size from `AT_PAGESZ` in the recorded auxv, pid 1's name and resident pages, the interfaces that have carried a byte, and NFS present. `collect::linux::fixtures` samples each tree twice with every source but exit records, and holds the second sample to those values. Pid 1's memory is checked as pages × page size, which is where a wrong page size shows. Forcing the page size to 4096 fails the 64k fixture on exactly that line.

**Eight fixtures**, listed with what each covers in `tests/fixtures/linux/README.md`:

- *Recorded:* `ubuntu-x86_64` (6.17), `ubuntu-22.04-x86_64` (6.8), `ubuntu-arm64` (6.17, arm64) and `container-arm64`, a Docker container on a 7.0 kernel built without PSI. The container has a restricted `/proc`, a pid namespace of two processes, and a namespaced cgroup. Each carries its `uname`.
- *Derived*, because nobody here has these machines. Each is a recorded tree changed by `tests/fixtures/linux/derive` in exactly the documented ways, and says so in its `derived` file. `kernel-3.10` has no PSI, no `MemAvailable` (so poptop's `free`-style estimate is checked), no `smaps_rollup`, and cgroup v1. The others are `cgroup-v1`, `arm64-64k-pages` and `nfs-client`. A recorded fixture from such a machine should replace each.

**Absent, never zero.** A fixture without PSI must give `pressure: None`. One without cgroup v2 must give no cgroups. One without NFS mounts must give no NFS. A process without `smaps_rollup` must have `pss: None`. All of them pass.

Nothing was found broken. The value is in the next change to a parser, which now meets eight kernels and not one. The fixtures are about 1 MB across 5,100 small files. Command lines in them are GitHub's runner services and the recording shell's, and the attribution check passes over all of them.

