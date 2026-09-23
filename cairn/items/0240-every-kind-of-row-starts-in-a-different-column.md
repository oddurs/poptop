---
id: 240
title: Every kind of row starts in a different column
type: bug
status: backlog
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
