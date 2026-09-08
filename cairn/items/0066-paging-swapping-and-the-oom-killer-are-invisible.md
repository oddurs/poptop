---
id: 66
title: Paging, swapping and the OOM killer are invisible
type: feature
status: done
milestone: v2.1
depends_on:
- 58
created: 2026-09-08
updated: 2026-09-08
priority: p2
area: collect
---

## Problem

atop's `PAG` line reports `scan`, `steal`, `stall`, `compact`, `numamig`,
`migrate`, `pgin`, `pgout`, `swin`, `swout`, `zswin`, `zswout` and **`oomkill`**.

poptop shows swap used against swap total. A machine that swapped four gigabytes
in and out during the interval and one sitting on four gigabytes of idle swap
report the same number — and the constraint detector 0047 had to work around
exactly that, using swap *growth* across a window because the level says
nothing.

`oomkill` is the sharpest of them. A process that was killed by the OOM killer
is gone from the next sample and there is nothing anywhere saying why. That is
the single most common "what happened to my process" question a monitor is
asked, and poptop cannot answer it even with the buffer.

## Why the buffer makes this worth more here

Scrubbing back to the moment of an OOM kill and seeing both the kill count and
the process table from the instant before is a thing no live-only monitor can
do. atop can, from a logfile. poptop can, from memory, with no daemon.

## What needs deciding

- **Where the OOM count goes.** It is an event, not a level, and the panel has
  no vocabulary for events yet — `came and went` is the nearest thing. It
  probably belongs beside that.
- **Whether swap rates replace the level.** They answer the question the level
  fails to. Both is probably right, and 0047's headroom rule should then read
  the rate rather than the growth it currently infers.

## Acceptance criteria

- [x] Page in/out and swap in/out as rates
- [x] OOM kills during an interval are reported, and visible when scrubbed to
- [x] 0047's memory constraint reads the rate rather than inferring it
- [x] Absent rather than zero where the platform will not say

## How it was resolved

PR #87. The panel says `2 processes killed for memory` in the interval it
happened, and **scrubbing back shows the count beside the process table from the
instant before** — the thing no live-only monitor can do and which atop can do
only from a logfile its daemon was already writing.

Where it goes: beside `N tasks came and went`, one rank above, because a kill is
a stronger fact than a task merely vanishing.

**The rates fix the rule upstream.** 0047 had to infer memory pressure from swap
*growth across a window* precisely because the level cannot tell a box that
swapped four gigabytes in and out from one sitting on four idle ones. It reads
the rate now — held across the window, not off the last sample, which review
caught: one frame of ordinary reclaim would otherwise have flipped the advice
and preempted a sustained CPU constraint.

**Two units in one file:** `pgpgin`/`pgpgout` are kilobytes and the swap pair is
pages, so one conversion for both reports swap at a four-thousandth of its size.

**Review also caught three honesty failures**: an absent `oom_kill` key read as
zero — this item's fourth criterion, failed in the one place it pointed at; no
gap guard on the count, so a laptop suspend attributed a night's kills to one
second; and a rate truncated before its unit was applied, which read a box
swapping a page an interval as quiet and so silenced the signal the new
constraint rule keys on.
