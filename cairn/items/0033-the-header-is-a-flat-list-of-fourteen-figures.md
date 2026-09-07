---
id: 33
title: The header is a flat list of fourteen figures
type: bug
status: done
milestone: v1.0
created: 2026-09-07
updated: 2026-09-07
priority: p1
area: ui
---

## What it looks like now

```text
 poptop — LIVE · warn 50 · crit 80
CPU  14.0%   / 87.4% full   en0 195.8K/s 95.4K/s   MEM  87.9% ███████████░   SWP  73.7%   UP 4d 13h 51m
 14 cores ▂▂▂▂ ▁▁▁▁ ▁▃▃▂ ▂▃
```

On Linux, with everything present, the order is: CPU, WAIT, `/` full, disk,
STALL, RUN, BLOCKED, network trouble, MEM, network throughput, SWP, UP, PROCS,
LOAD.

Read as groups that is: compute, compute, **storage-space**, storage-io,
compute, compute, compute, **network**, memory, **network**, memory, machine,
machine, compute. Network appears twice with memory between the halves. Disk
space sits between two compute figures. Load — the most compute of all — is
last.

## Why it is like that

`Figure::rank` was introduced to answer *what to drop on a narrow panel*, and it
is also, by accident, the reading order. Those are different questions with
different right answers. `LOAD` should be given up early — it is the least
diagnostic figure here — and if it is shown at all it belongs beside CPU, not
after uptime.

## What should happen

Two axes, not one.

- **Group and position** decide where a figure sits. Compute, memory, storage,
  network, machine — the order the question "why is this slow" walks through.
- **Rank** decides when it goes, unchanged.

Groups need to be visible or they are not groups: a wider gap between them than
within them, or a dim rule.

The state marker should come down into this row too. `LIVE` / `PAUSED -12s` and
the heat-ramp legend currently occupy a row of their own alongside the word
`poptop`, which is the only part of the screen that never says anything. The
marker qualifies the figures, so it belongs with them — and `PAUSED` keeps its
loud treatment, sitting inside the data it is warning about rather than above it.

## Acceptance criteria

- [ ] Figures sit in group order; rank governs only what is dropped
- [ ] Group boundaries are visible without colour
- [ ] `LIVE`/`PAUSED` and the heat legend move into the figures row
- [ ] The header is two rows, not three
- [ ] `PAUSED` is no less prominent than it is now
