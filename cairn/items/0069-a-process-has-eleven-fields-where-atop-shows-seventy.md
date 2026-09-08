---
id: 69
title: A process has eleven fields where atop shows seventy
type: feature
status: backlog
milestone: v2.1
depends_on:
- 58
created: 2026-09-08
updated: 2026-09-08
priority: p1
area: collect
---

## Problem

`ProcSample` has pid, ppid, name, user, cpu, rss, threads, state, started, cmd
and io. atop shows around seventy fields across its process views. The ones that
answer a question poptop currently cannot:

**Memory** — `MINFLT`/`MAJFLT` (a process taking major faults is being paged in
from disk, and it is why it is slow), `VSIZE`, `VGROW`/`RGROW` (growth *during
the interval*, which is how a leak is spotted rather than inferred),
`SWAPSZ`, and `PSIZE` — proportional set size, which is the honest answer to
"how much memory is this actually costing" and the fix for the upper-bound
caveat 0050's grouped RSS carries.

**Scheduling** — `POLI`, `NICE`, `PRI`, `RTPR`, `CPUNR`, `NVCSW`/`NIVCSW`
(voluntary against involuntary context switches: a process being preempted
constantly and one waiting on IO look identical without them), `WCHAN` (what a
`D`-state process is blocked in, which is the natural next question after the
`state = D` filter finds it).

**Lifecycle** — `EUID`/`EGID`, `STDATE`/`STTIME`, and for exited processes
`ENDATE`/`ENTIME`, `ST` and `EXC` — how it ended and with what code.

Most of these are fields in `/proc/<pid>/stat`, which poptop reads and parses
every sample already. `PSIZE` needs `smaps_rollup`, which is a second read and
belongs behind the cost model.

## What needs deciding

- **Which are worth the buffer.** Every field multiplies by the retained sample
  count. Growth figures are deltas and could be derived at render time from two
  samples rather than stored — cheaper, and correct only if the samples are
  adjacent, which scrubbing breaks.
- **What `WCHAN` costs.** atop gates it behind `W` because it is a read per
  thread. It is also the highest-value one on this list.

## Acceptance criteria

- [ ] Fields already in `/proc/<pid>/stat` are collected without new reads
- [ ] `PSIZE` behind the cost model, and 0050's memory caveat resolved by it
- [ ] Growth figures are correct across a scrub, or absent there
- [ ] macOS reports what sysinfo gives and is absent elsewhere
- [ ] Measured with `--bench`
