---
id: 20
title: Graph what the machine is waiting on, not what it has used
type: feature
status: backlog
milestone: v0.2
depends_on:
- 19
created: 2026-09-06
updated: 2026-09-06
priority: p0
effort: m
area: ui
---

## Problem

The timeline spends half its rows on memory used over time, which is the least
time-varying signal on screen: over a ten-minute window it is a flat line or a
slow ramp, and it repeats what the header already says. That is half of the
most valuable real estate in the interface.

Meanwhile the signals that spike, and that explain why a machine feels slow,
are not graphed at all.

## Proposal

`WAIT` becomes the second graph, drawn exactly like CPU — peak-aggregated,
scrubbable, with the same threshold rules. This is the row that turns the
hung-NFS case from a mystery into a shape.

Memory is not deleted, it is demoted: it returns as a third row when the
terminal is tall enough, through the same degradation ladder that already
decides whether the gutter, the axis anchors and the series labels appear. On a
short terminal you get the two rows that answer the question; on a tall one you
get all three.

`WAIT` is four characters and the gutter is four wide, so the label fits
without touching the layout constants.

The source is `iowait` from item A. PSI (`/proc/pressure/*`) is the better
signal and belongs here eventually, but it is absent on this host's kernel and
on all three distro images checked, so it is an upgrade that degrades honestly
rather than the thing to build the row on. Repeating the taskstats mistake once
is enough.

## Acceptance criteria

- [ ] CPU and WAIT are the two rows on a short terminal
- [ ] MEM appears as a third row when height allows, and is dropped first when it does not
- [ ] The WAIT row scrubs, zooms and carries threshold rules like CPU
- [ ] A machine that is idle-but-blocked looks visibly different from an idle one
