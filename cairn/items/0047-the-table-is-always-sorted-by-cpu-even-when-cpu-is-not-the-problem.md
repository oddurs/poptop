---
id: 47
title: The table is always sorted by CPU, even when CPU is not the problem
type: feature
status: backlog
milestone: v1.0
created: 2026-09-07
updated: 2026-09-07
priority: p1
area: ui
---

## The lesson

atop sorts its process list by *whichever resource is currently constrained*.
Its docs call it "automatic sort on the most utilized resource", and it means
that when the disk is the bottleneck the table is already sorted by disk. The
tool answers the question rather than laying out numbers and leaving the reader
to.

poptop already collects everything needed to do this and does not use it. The
header says `WAIT 26.7%`, `vda 88% 40ms`, `STALL io 6.1%` — and then the table
below is sorted by CPU, which is the one resource that is *not* the problem.

## Why this fits here better than it fits atop

Saturation over utilisation is already the layout thesis: the header is ordered
by how much each figure answers "why is this slow". Sorting the table by the
same judgement is the same idea applied one panel down.

And poptop has something atop does not — a `STALL` figure per resource, which is
a direct measure of *which* resource is stopping work rather than which is busy.
That is a better input to this decision than utilisation is.

## What needs deciding

- **Suggested, not imposed.** A table that reorders itself under the reader is
  worse than one that does not. The likely shape is a hint — the section title
  saying `sort: CPU · disk is the constraint`, or the relevant column heated —
  with one key to accept it. `s` already cycles sorts.
- **What counts as constrained.** `io.full` above the stall threshold, a device
  past its utilisation threshold, memory with swap active. Each of those already
  has a threshold defined somewhere.
- **Hysteresis.** A constraint that flickers between two resources must not
  flicker the suggestion.
- **What it does while scrubbing.** The constraint at the cursor, not now — the
  whole point of the buffer.

## Acceptance criteria

- [ ] The panel names the constrained resource when there is one
- [ ] Accepting it is one key, and it is never applied without asking
- [ ] Nothing is claimed when no resource is constrained
- [ ] While scrubbing, the constraint is the one at the cursor
