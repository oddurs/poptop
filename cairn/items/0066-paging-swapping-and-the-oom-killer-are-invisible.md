---
id: 66
title: Paging, swapping and the OOM killer are invisible
type: feature
status: backlog
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

- [ ] Page in/out and swap in/out as rates
- [ ] OOM kills during an interval are reported, and visible when scrubbed to
- [ ] 0047's memory constraint reads the rate rather than inferring it
- [ ] Absent rather than zero where the platform will not say
