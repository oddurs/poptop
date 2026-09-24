---
id: 253
title: A tile does not know how to be small
type: feature
status: backlog
milestone: r13
labels:
- ui
depends_on:
- 250
created: 2026-09-23
updated: 2026-09-23
priority: p0
---

## Problem

Every element in poptop degrades: the header figures by rank, the timeline
captions by a ladder, the table by a width ladder, the axis by dropping its
anchors. None of that composes. A layout solver handed six tiles and forty rows
has no way to ask a tile what it needs, or what it would give up first.

## Proposal

A tile declares three things: a minimum size it is worth drawing at, a want,
and a ladder of what it sheds as it shrinks — legend, then axis, then title,
then down to a single sparkline row, then to a chip.

The solver fits the tree against the frame: satisfy minimums by priority, spend
what is left on wants, and where a tile cannot reach its minimum, drop it and
say so somewhere the reader will see.

## Acceptance criteria

- [ ] A tile trait with minimum, want and a ladder, implemented by the graph, the table and the header
- [ ] The solver is deterministic and total: any frame size produces a layout or a stated reason a tile is missing
- [ ] Every rung of every ladder is reachable by narrowing, held by a test that walks widths and asserts the ladder never goes backwards, as `every_element_yields_monotonically` does now
