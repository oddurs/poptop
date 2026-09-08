---
id: 85
title: Threads are counted but never shown
type: feature
status: backlog
milestone: v2.0
depends_on:
- 58
created: 2026-09-08
updated: 2026-09-08
priority: p1
area: collect
---

## Problem

`ProcSample.threads` is a count. atop has `y` to expand a process into its
threads and shows `TID` alongside `PID`, with per-thread state, scheduling and
CPU.

The count already misleads on its own. A process at 800% with 40 threads tells
you it is busy; which of those threads is spinning, and whether one is stuck in
`D` while the rest idle, is the question a reader has next and poptop cannot
answer. The `BLOCKED` figure in the header counts *tasks*, so a box with two
blocked threads inside one healthy-looking process reports a number the table
cannot itemise — the same complaint 0049 answered for predicates.

## Why it belongs in the completeness milestone

atop's guarantee is about tasks, not processes: `PRC` counts `#trun`, `#tslpi`,
`#tslpu`, `#tidle`, `#zombie` across threads, and the kernel's `processes`
counter that poptop already reads advances on a `clone` exactly as on a `fork` —
which is why 0011's churn figure had to be worded in tasks. Half of poptop's
task-level figures are already there; the rows are not.

## What needs deciding

- **Cost.** `/proc/<pid>/task/` is a directory read per process plus a `stat`
  read per thread. On a box with 400 processes and 4000 threads that is an order
  of magnitude more reads than today. Gated, and by what.
- **Where they appear.** Expanded under their process, as atop does, or as their
  own rows. Expansion interacts with the tree and with grouping, both of which
  already claim the same vertical space.
- **What a thread is in the store.** A `ProcSample` with a `tid`, or a separate
  collection. The ring buffer holds every sample's full table; multiplying its
  size by ten is a decision, not a detail.

## Acceptance criteria

- [ ] A process expands to its threads, with per-thread state and CPU
- [ ] The header's task-level figures are itemisable from the table
- [ ] Cost measured with `--bench`, and gated
- [ ] macOS says what it cannot do rather than showing one thread per process
