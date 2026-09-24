---
id: 168
title: Two moments cannot be compared
type: feature
status: backlog
milestone: v5.0
depends_on:
- 140
created: 2026-09-13
updated: 2026-09-13
priority: p1
area: ui
---

## Problem

"What is different about now compared with ten minutes ago" is the second
question anybody asks, straight after "what happened". poptop can show either
moment and never both, so the comparison happens in the reader's head from two
screenfuls they cannot see at once.

This is the question that finds leaks, runaway spawning, and the process that
quietly appeared.

## Why poptop and not the others

It needs two complete process tables from two instants. Only poptop has them,
and it has them without anything having been running beforehand.

## What it looks like

Mark a moment, scrub to another, and ask for the difference:

```text
  03:02:11 → 03:14:40    12m29s
  + 14  cc1plus          +71.2% cpu   +2.1G rss
  + 1   ld                +12.0% cpu  +840M rss
  ~ 1   postgres          +0.1% cpu   +3.4G rss   ← grew, did not restart
  - 3   sshd              was 0.2% cpu
    machine   cpu +84.1   mem +31.2   iowait +0.4   load 1.2 → 14.8
```

Three kinds of row, and the distinction is the point: **appeared**, **vanished**,
**the same process, changed**. The third is identified by `(pid, start time)`,
which is how poptop identifies a process everywhere else — a pid that was reused
is two different processes and must show as one `-` and one `+`, never as
growth.

## What needs deciding

- **How the second moment is chosen.** A mark key and the cursor is the obvious
  shape. `--report` already finds interesting moments; a diff between the peak
  and the calm before it is a report line worth having on its own.
- **What "changed" means for a figure that is a rate.** CPU at two instants is
  two rates and the difference is meaningful. RSS is a level. Bytes read is
  cumulative. Each needs its own verb or the table lies in three different ways.
- **Ordering.** By absolute change in the figure being sorted on, so the diff
  answers the question the table is already asking.
- **Whether it is a panel or an output.** Both, eventually — a diff is exactly
  what somebody pastes into an incident channel, so it wants a `--export` form.

## Acceptance criteria

- [ ] Two moments in the buffer can be compared without leaving the tool
- [ ] Appeared, vanished and changed are distinguished, not merged
- [ ] A recycled pid is two processes, never one that grew
- [ ] Rates, levels and totals are each differenced in a way that is true of them
- [ ] The machine figures are diffed alongside the processes
- [ ] Landing either end in a gap says so, as the jump does
