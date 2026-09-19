---
id: 83
title: The /proc parsers are tested only against the files their author thought to write
type: chore
status: done
milestone: r1
labels:
- validation
created: 2026-09-18
updated: 2026-09-18
priority: p1
effort: l
area: collect
---

## Problem

`src/collect/linux.rs` (4,491 lines) and `procinfo.rs`, `cgroups.rs`, `nfs.rs` and `taskstats.rs` parse text and binary formats that the kernel owns and has changed over time. They include fields that are absent on older kernels, `comm` values containing spaces and parentheses, and `mountstats` lines that vary by NFS version. The 90 tests in `linux.rs` use hand-written fixtures. Nothing checks that a malformed or unexpected file produces "unknown" rather than a panic or a wrong number.

## Proposal

Give each parser a property test: arbitrary bytes never panic, and a well-formed file with any one field removed or duplicated yields `None` for that field and the correct values for the others. Add fuzz targets for the parsers that take attacker-influenced text, such as `comm` and `cmdline`.

## Acceptance criteria

- [x] Every parser of a `/proc` or `/sys` file has a never-panics property test
- [x] A `comm` of `a) (b` and one of 16 bytes of invalid UTF-8 are both parsed correctly
- [x] Missing optional fields (e.g. PSI on a pre-4.20 kernel) yield `None`, not zero
- [x] Fuzz targets for `stat`, `status` and `mountstats` parsing

## How it was resolved

**Found and fixed.**

- **A process whose `comm` is not UTF-8 was missing from the table.** `/proc/<pid>/stat` went through the strict UTF-8 read, and a failed read meant `continue`. `comm` is the executable's file name, or whatever `prctl(PR_SET_NAME)` set, so any unprivileged process could leave poptop's table by naming itself `\xff`. The same applied to threads. `stat` is now read as bytes and decoded lossily (`stat_text`). `a_running_process_named_in_bytes_that_are_not_utf8_is_in_the_table` runs a real process named `\xffhidden` through the real collector, and fails if the lossy decode is reverted.
- **One odd mount path blanked every filesystem.** `/proc/mounts` and `/proc/self/mountstats` were read with `read_to_string`. The kernel escapes only whitespace and backslash in mount paths, so a USB stick with a Latin-1 label under `/media` made the whole file unreadable, and with it the capacity of every mount and every NFS figure. The same was true of `/proc/<pid>/cgroup` and `/proc/net/dev`. These are now read lossily (`read_lossy`), so only the odd mount is lost: it fails its `statfs`. Carrying mount paths as bytes would keep that mount too; not done here.
- **Old kernels reported memory 100% used.** Before 3.14 there is no `MemAvailable:` line, and absent was read as zero, so `used == total` on every sample. A container shows its host's kernel. Available is now estimated the way `free` did before the kernel provided it: free + buffers + cached + reclaimable slab − shmem.
- **`cpu_list("0-4294967295")` built a 16 GB vector** from one line. Ranges are now bounded at 65,536 CPUs; `NR_CPUS` tops out at 8,192.
- **`CpuTimes::parse` dropped non-numeric fields**, which shifted every later field into the wrong slot (steal read as irq). It now stops at the first bad field.
- **`cpu.max` parsed as floats** accepted `nan` and `inf` as CPU limits. It now takes integers, as the kernel writes them.
- **Unchecked arithmetic panicked on counters at the edge of `u64`:** `utime + stime`, `rss_pages * page_size`, the kB→bytes conversions in meminfo, node meminfo and smaps, the CPU-time sums, and the cgroup and NFS sums across devices and operations. All saturate now.

**Tests.** `src/mangle.rs` generates variants of a real input: each line removed or doubled, each number replaced by each edge value, bytes that aren't UTF-8, cuts at every line, plus random stacked mutations. A fixed seed means a failure reproduces from the test name. `every_text_parser` runs every text parser in `linux.rs` on each variant. NFS, cgroups and taskstats have the same test in their own modules. `exits_in` was pulled out of the socket loop in `taskstats.rs` so a datagram can be tested and fuzzed without a socket.

**Fuzz targets.** `proc_files` (every `/proc` parser), `nfs` and `taskstats`, Linux only. `status` is not parsed anywhere in poptop, so it has no target. PSI missing entirely already gives `None` (`pressure_from`). A `cpu` pressure file without a `full` line (before 5.13) still reads `full` as 0.0, because `Stall.full` is not optional. Changing that type touches the UI and is not done here.
