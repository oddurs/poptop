---
id: 248
title: A line is drawn with corners, not with coverage
type: bug
status: backlog
milestone: r11
labels:
- ui
- graph
depends_on:
- 244
created: 2026-09-23
updated: 2026-09-23
priority: p2
---

## What happens

`--graph=line` picks box-drawing characters by looking at where the previous
and next columns sat. A steep slope becomes a stack of verticals with visible
steps; a shallow one becomes a dotted run of horizontals; joins between the two
are whichever corner the branch reached.

## What should happen

A stroke is a mark with width, rasterized into coverage like any other: the
subcells it passes through are covered in proportion, joints are where two
segments' coverage overlaps, and the surface fits the closest glyph it has.

On a set with levels that also buys anti-aliasing for free — a half-covered
subcell picks the lighter level — without inventing a value, because the
coverage is geometry rather than data.
