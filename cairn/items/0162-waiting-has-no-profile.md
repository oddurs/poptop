---
id: 162
title: Waiting has no profile
type: feature
status: backlog
milestone: v6.0
depends_on:
- 144
created: 2026-09-13
updated: 2026-09-13
priority: p2
area: collect
---

## Problem

0086 makes a blocked process say what it is blocked *on* — the kernel symbol, the
syscall, the file. That is one process at one instant.

The question underneath is different and nobody in this category answers it:
**where does this machine spend its waiting?** Not which process is stuck now,
but which of them have been stuck, on what, and for what fraction of the last ten
minutes.

## What it does

poptop samples every process every tick. Once 0086 reads `wchan` for the blocked
ones, the buffer accumulates — at no extra cost — a statistical picture of
blocking across the whole retention window:

```text
  waiting, over the last 10m    (4,812 blocked-process samples)

  62%  folio_wait_bit            page cache read
       └ 48%  postgres           /var/lib/pg/base/16384/2601
       └ 14%  node               /app/node_modules/… (14,002 distinct files)
  27%  rpc_wait_bit_killable     NFS server not answering
       └ 27%  make               /mnt/build/…
  11%  io_schedule               block device queue
```

This is an off-CPU profile — the thing `offcputime` from bcc produces — and it is
assembled from samples poptop is already taking, with no eBPF, no kernel version
requirement and nothing to install.

## Why it is honest to call this a profile

It is a **sampled** profile and must be labelled as one. `offcputime` instruments
every scheduler switch and gets exact durations; this counts how often a process
was observed blocked at one-second intervals, which is a statistic, not a
measurement. At a one-second interval it will miss anything shorter than a second
entirely — and short blocking is exactly what a real off-CPU profiler is for.

What it catches instead is the thing that matters more often and that nobody
looks for: **sustained** blocking. A process stuck on a hung mount for four
minutes is invisible to a live monitor that shows one `D` in a table, and it is
the whole picture here.

The sample count is printed for that reason. Eight samples is an anecdote.

## Why nobody else has it

`offcputime` needs eBPF, root and a kernel with BTF, and produces a flamegraph
for one process over a window you chose in advance. It is the better tool and it
is not installed on the box during an incident.

The reason poptop can approximate it for free is structural: it is **already
walking every process every second**, and it is already retaining the results.
Nobody else in this category retains per-process state over time, so nobody else
can aggregate it. The profile is a by-product of the buffer.

## What needs deciding

- **Retention.** A `wchan` string per blocked process per sample is small
  because blocked processes are few — but "few" is exactly wrong on the machine
  this feature is for, where a hundred tasks are stuck behind one mount. Needs
  measuring there, not on a quiet laptop.
- **Whether the file is worth keeping.** The symbol and the syscall are small and
  bounded; the resolved path is neither. Aggregating paths to a directory, or
  keeping the top few and counting the rest, is probably the answer — the example
  above shows what "14,002 distinct files" has to look like when it will not fit.
- **Where it lives.** It is the stall panel zoomed (0092), and it is a `--report`
  section. Not a panel of its own.
- **Sub-interval blocking.** At `--interval=1s` this sees nothing shorter than a
  second, and the panel must say so rather than implying completeness.

## Acceptance criteria

- [ ] Blocking is aggregated across the retained window, not shown per instant
- [ ] Grouped by kernel symbol, with the process and the file beneath it
- [ ] The sample count is stated, and a thin sample refuses to characterise
- [ ] The panel says what it cannot see: anything shorter than the interval
- [ ] Path retention is bounded, and the bound is stated when it bites
- [ ] Cost measured with a hundred tasks blocked on one mount
