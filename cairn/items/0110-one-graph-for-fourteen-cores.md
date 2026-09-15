---
id: 110
title: One graph for fourteen cores
type: feature
status: todo
milestone: v3.5
created: 2026-09-14
updated: 2026-09-14
priority: p2
area: ui
depends_on:
- 112
---

## Problem

The header draws per-core bars — a snapshot, one column a core. The timeline
draws the machine's total. Neither shows a core's *history*, so "one core has
been pinned for ten minutes while the rest idle" — the signature of a
single-threaded bottleneck, and one of the few things a system monitor is
actually for — is invisible in both.

The same gap exists for disks, for filesystems, and for containers.

## The fix

Small multiples: a grid of one-row graphs, one per core, all on one scale, in a
fixed order. The fixed order is the point — a grid that re-sorts by activity
cannot be read across time, because the cell you were watching moves.

One shared scale is the other half. Fourteen graphs each fitted to their own
range look identical whatever the machine is doing, which is the opposite of
what a grid is for.

## What needs deciding

- **Which axis is fixed.** A 96-core machine does not fit a column. Wrapping by
  core index keeps neighbours adjacent; wrapping by socket keeps NUMA readable.
- **When it replaces the total.** Probably a mode rather than a panel, since a
  grid wants the whole screen.

## Acceptance criteria

- [ ] One pinned core is visible against thirteen idle ones
- [ ] Every cell is on the same scale, stated once
- [ ] Cells do not move between frames
- [ ] Degrades to the total when there is no room
