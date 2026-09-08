---
id: 69
title: A process has eleven fields where atop shows seventy
type: feature
status: done
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

- [x] Fields already in `/proc/<pid>/stat` are collected without new reads
- [x] `PSIZE` behind the cost model, and 0050's memory caveat resolved by it
- [x] Growth figures are correct across a scrub, or absent there
- [x] macOS reports what sysinfo gives and is absent elsewhere
- [x] Measured with `--bench`

## How it was resolved

PR #84. Not seventy fields — the ones that answer a question poptop could not,
in the memory view 0070 built.

**`MAJF/s`** answers *why is this slow*: a process being paged in from disk
while its CPU looks low and its state looks ordinary. A rate over the interval,
not the lifetime total `/proc` publishes.

**`PSS`** is the measurement 0050's caveat asks for. Grouped RSS is an upper
bound because forked workers share pages copy-on-write; PSS divides a shared
page among its sharers, so six renderers sum to what they cost. Measured at
**4.2us a process** — 0.85ms a sample becomes 1.77ms at 218 — so it is read only
while the memory view is open.

**`VSZ` and `NICE`** are free: fields in a `stat` line already parsed.

**`GROW` is derived, not stored** — a growth figure in every retained sample is
a field carried forever to describe one interval. An em dash across a seam,
using `history::gap_limit` so the timeline and the column cannot disagree about
which samples are adjacent.

**Which are worth the buffer:** a retained process goes 70 -> 103 bytes and the
store 15.8 MB -> 24.9 MB at 600 samples of 400 processes. The window is
unchanged at a 64 MB cap, but it is the largest increase any field here has
cost, and every optional pays its tag byte whether or not the platform answers.

**Review found the item's headline column reading zero forever.** The write-back
that makes the fault counters a rate was never applied, so the baseline was
empty on every sample. The test that should have caught it asserted on the map
the parser is handed — the very thing being discarded — so the assertion now
lives on the collector. Also: the elision arithmetic counted the columns the
view drops but not the four it adds; a rate was styled against a percentage
threshold; a group borrowed one member's growth where the platform reports no
start time; and a `u32` of faults a second overflows across a large group.
