---
id: 240
title: Every kind of row starts in a different column
type: bug
status: dropped
milestone: r10
labels:
- ui
created: 2026-09-23
updated: 2026-09-23
priority: p2
---

## What happens

The menu bar starts at column 2, the dividers at column 0, the key bar at
column 1, the table's rows at column 2, the axis row at column 1. Down the left
edge the screen is ragged, and at `density = compact` the margins change again.

## What should happen

One margin, set by the density, applied to everything that is not deliberately
full-bleed — which is the dividers, and only them.

## 2026-09-23

Investigated on captures at 120 and 160: every band already starts on the content margin, which  is the single source of. What looked ragged is deliberate padding inside two elements — the tab labels carry a space so the underline does not touch the margin, and the LIVE chip's space is inside its own reversed block — plus the axis row, whose `past` anchor is positional and correctly marks the graph's left edge. Nothing to fix; the margin work this item assumed was needed was done by whoever wrote Density::margin.

## 2026-09-23

Investigated on captures at 120 and 160 columns: every band already starts on the content margin, and Density::margin is the single source of it. What looked ragged is deliberate padding inside two elements -- the tab labels carry a space so the underline does not touch the margin, and the LIVE chip's space sits inside its own reversed block -- plus the axis row, whose past anchor is positional and correctly marks the graph's left edge. Nothing to fix: the margin work this item assumed was missing had already been done.
