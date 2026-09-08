---
id: 46
title: The cursor readout repeats the header and drops the scale
type: bug
status: done
milestone: v1.0
created: 2026-09-07
updated: 2026-09-07
priority: p3
area: ui
---

## Problem

Scrubbing back one second, at 120x14:

```text
 0| PAUSED  -1s CPU  16.5%  │  MEM  82.7% ██████████░░  SWP  70.0%  │  …
…
10|                                                    CPU 16.5%  MEM 82.7% ▐
```

The row under the graph reports `CPU 16.5%  MEM 82.7%`. The header, two
centimetres up, reports `CPU 16.5%` and `MEM 82.7%`. They are the same numbers,
because while scrubbing the header *is* showing the sample under the cursor.

## What that row should be doing

It is the only place that can say **where in time the cursor is**, and it says
that only through the `▐` marker's horizontal position. Meanwhile the two things
a scrubbing reader wants from it — how far back, and what a cell is worth — are
either in the header (`-1s`) or dropped entirely: the `1s/slot` caption is
replaced by this readout, so while scrubbing the scale is not stated anywhere.

That is a hole I opened when the axis and caption were merged into one row: live
shows the caption, scrubbing shows the cursor, and scrubbing lost the scale.

## What should happen

The row is positional. It should carry the marker, the anchors that give it
meaning, and the scale — and leave the values to the header, which is already
showing them and has room.

Something closer to:

```text
past                    1s/slot            ▐  -1s            now
```

Worth checking while in there: whether `-1s` belongs on this row rather than in
the state marker, since it is a fact about the cursor rather than about the
figures. Probably not — the marker's loudness is what stops a stale table being
read as live — but the duplication is worth being deliberate about.

## Acceptance criteria

- [x] The cursor row does not repeat figures the header is already showing
- [x] The slot size is stated while scrubbing, as it is while live
- [x] The anchors survive, so the marker's position means something
