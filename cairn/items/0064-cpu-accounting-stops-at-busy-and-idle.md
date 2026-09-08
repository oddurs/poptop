---
id: 64
title: CPU accounting stops at busy and idle
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

- [ ] steal, guest, irq and softirq reported where the platform says
- [ ] Context switches and interrupts a second, as rates rather than counters
- [ ] steal appears in the header when it is non-trivial and not otherwise
- [ ] macOS reports what it has and says nothing about what it does not
- [ ] Measured: no additional reads, since the file is already being parsed
