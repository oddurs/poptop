---
id: 64
title: CPU accounting stops at busy and idle
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

poptop reads `/proc/stat` and reports total busy, per-core busy and `iowait`.
The same file already carries, in the same line poptop is already parsing:

- **steal** — time the hypervisor took. On any cloud instance this is the
  difference between "the box is busy" and "the box is not being given a box".
  There is no other way to see it, and it is one field away.
- **guest** and **guest_nice** — time given to VMs, which on a hypervisor is the
  work rather than the overhead.
- **irq** and **softirq** — separated, a network-heavy box's softirq time is the
  answer to why user time looks low while nothing is idle.
- **ctxt** and **intr** — context switches and interrupts a second, which atop
  shows as `csw` and `intr` on the `CPL` line. A box thrashing between threads
  looks identical to a busy one without them.

Every one of these is in a file poptop reads every sample and parses already.
`steal` in particular is free.

## Why it is not just more numbers

The header is ordered by how much each figure answers "why is this slow", and
steal answers it in a way nothing currently on screen can: every other figure
looks healthy while the machine gets less done. That is the same shape as the
throttling item, which was worth its own figure.

## Acceptance criteria

- [x] steal, guest, irq and softirq reported where the platform says
- [x] Context switches and interrupts a second, as rates rather than counters
- [x] steal appears in the header when it is non-trivial and not otherwise
- [x] macOS reports what it has and says nothing about what it does not
- [x] Measured: no additional reads, since the file is already being parsed

## How it was resolved

PR #85. `/proc/stat`'s CPU line has ten fields and poptop was reading four; the
rest were one parse away in a file already read every sample, so `--bench` is
unchanged.

**`STL` earns a header slot**, above 1% only — on bare metal it is zero forever
and a figure on every frame is one nobody reads. Ranked next to `CLK` because
both *qualify* CPU rather than adding to it. Coloured on its own scale, 5% warn
and 20% critical, mapped onto the theme's boundaries the way `stall_heat`
already does: a guest losing 30% of its time is the condition the figure exists
to expose, and the utilisation thresholds drew it calm.

**guest, irq, softirq and the switch rates go to `--once`** — real answers, but
not ones worth permanent header space. `guest` stays outside the busy total
because the kernel counts it inside `user` already.

**Review found four honesty gaps**: a short CPU line made the extended classes
a fabricated zero rather than an absence; the counter baselines were stored
after fallible reads, so a failed sample doubled the next rate — these are the
first figures here that divide by wall clock rather than by jiffies, which is
why `prev_at`'s existing warning had never bitten; `--once` claimed a platform
published neither counter when it had withheld one; and the new fields were
missing from both the fabricated-default guard and the store round-trip.
