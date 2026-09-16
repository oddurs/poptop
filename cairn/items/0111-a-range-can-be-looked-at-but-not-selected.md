---
id: 111
title: A range can be looked at but not selected
type: feature
status: backlog
milestone: v3.5
created: 2026-09-14
updated: 2026-09-14
priority: p2
area: ui
---

## Problem

Scrubbing moves a cursor to an instant, and the process table shows that
instant. Every question about a *period* — what ran during that spike, what the
disk did across those two minutes — needs `--report`, which is non-interactive
and works on recorded days rather than on what is on screen.

The graph already shows the period. There is no way to point at it.

## The fix

Brushing. Mark a start and an end on the timeline; everything else re-aggregates
to exactly that span. The process table becomes the table for the range —
totals, peaks, and what was running for all of it against what appeared once.
The header figures become the range's figures.

This is what turns the timeline from a picture into a query.

## What needs deciding

- **Keys.** Scrubbing owns the arrows. A modifier to extend rather than move is
  the obvious shape and needs checking against terminals that eat modifiers.
- **What an aggregated process row means.** Peak CPU, mean CPU and time-present
  are three columns, not one, and the table has room for about one.
- **Whether the selection survives zoom.** It should, in time rather than in
  slots, or zooming would silently resize the question.

## Acceptance criteria

- [ ] A span can be selected with the keyboard alone
- [ ] The table answers for the span, not for an instant in it
- [ ] The selection is drawn unmistakably against the cursor
- [ ] Zoom does not change what is selected
