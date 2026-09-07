---
id: 54
title: Kernel threads crowd out the processes anyone is looking for
type: feature
status: done
milestone: v1.0
created: 2026-09-07
updated: 2026-09-07
priority: p3
area: ui
---

## Problem

On Linux the process table is full of kernel threads — `kworker/*`,
`ksoftirqd/*`, `irq/*`, `migration/*`. On a many-core box they outnumber real
processes, and none of them is what anyone opened a monitor to find.

htop has `K` to hide them and `H` for user threads. bottom has `z`. poptop shows
them all and has no toggle, despite already knowing which they are —
`ProcSample::is_kernel_thread` exists and is used to keep them out of the
per-process IO accounting, for exactly the reason that they distort a figure.

The same argument applies to the table. They are excluded from one figure
because including them misleads, and included in the panel where they crowd out
the processes the reader is looking for.

## What should happen

Hidden by default, with a key to show them, and the count said somewhere so
their absence is not silent — the same shape as every other omission here.

Worth checking whether the default should differ by platform: on macOS
`is_kernel_thread` is nearly always false, so the toggle would do almost
nothing and the default hardly matters. On Linux it is the difference between a
usable table and a wall of `kworker`.

## Acceptance criteria

- [x] Kernel threads are hidden by default on Linux
- [x] A key shows them, and the footer lists it
- [x] The number hidden is stated rather than silently dropped
- [x] Sorting, filtering and the tree behave with them hidden
