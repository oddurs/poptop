---
id: 61
title: Threads are counted but never shown
type: feature
status: done
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

- [x] A process expands to its threads, with per-thread state and CPU
- [x] The header's task-level figures are itemisable from the table
- [x] Cost measured with `--bench`, and gated
- [x] macOS says what it cannot do rather than showing one thread per process

## How it was resolved

PR #78. `y` — atop's key — expands the *selected* process.

**Cost.** Measured, not estimated: 1005 threads across 7 processes took a 534us
sample to 3.63ms, so **3.1us per thread**. Single-threaded processes are skipped
entirely — a process *is* its only thread — which is most of what keeps it
affordable. Retention is 17 bytes a thread against 65 for a process, pinned by a
test.

**Where they appear.** Under the selected process only: a box has eight times as
many threads as processes, and a table that grew ninefold on a keypress would
answer "which thread is spinning" by making the spinning one harder to find.
Not in the tree or while grouped — both order rows by something other than "this
process, then its threads", and the panel says so rather than letting the key do
nothing. That keeps this a per-row expansion rather than the fifth mode axis
0070 is about.

**What a thread is in the store.** Its own record. A thread shares its process's
memory, user, command line and parent, so a `ProcSample` per thread would
multiply the retained table by the fields that are identical across it.

**The gate.** Ratcheted like the IO columns, but with a release: sixty samples
after the view is turned off, collection stops. The IO ratchet never lets go and
that is right for one extra read per process; at ten times the cost and 68 KB of
every retained sample, holding it for a session because somebody pressed a key
once is the worse bargain. That correction came from review.

**Criterion 2** is `task = D`: `BLOCKED` counts tasks, so two blocked threads
inside one healthy-looking process were a number the table could not account
for. It finds single-threaded processes too, via the process's own state — they
are the commonest contributor to that figure.

**Follow-up.** macOS says `threads: not read on macOS`, which is honest rather
than final: mach's `task_threads` would answer this and sysinfo does not expose
it. Worth its own item if the macOS backend ever stops going through sysinfo.

**Found by review, and worth recording:** the thread rows were drawn off-screen
for any selection below the first screenful, because the offset pins the
selected row to the bottom line and the threads splice below it. Every fixture
here fits on one screen, so the whole feature was untested on a real-sized
table.
