---
id: 86
title: Prove the process table rewinds, do not claim it
type: feature
status: done
milestone: web
depends_on:
- 56
created: 2026-09-08
updated: 2026-09-08
priority: p1
effort: m
area: web
---

## Problem

The technical claim that actually separates poptop from zenith is that
*scrolling back moves the table too*. zenith has zoomable scrollback whose
history holds only aggregate series; its process table renders from a live map
that drops pids as they exit, so scrolling moves the charts and not the rows.

On the page this is one of three claims in a row of cards, indistinguishable in
weight from the two either side of it.

## Proposal

A short scrub strip over a process table, and nothing else in the section.

Drag the strip — or use the arrows — and the table underneath changes: postgres
climbs and the rows reorder, `rustc` appears during its unrelated burst and is
gone afterwards. The row that changed position since the last sample carries a
mark, so the movement is legible rather than just busy.

One line above it: *zenith's scrollback moves the charts but not the table. The
table is what names the process.*

This is the second interactive thing on the page, and that is the limit. It
earns it by being the claim a sceptical reader most wants to test.

## Acceptance criteria

- [ ] Scrubbing changes the table rows, not only the plot
- [ ] A process appears and disappears across the range, visibly
- [ ] Rows that moved are marked by something other than position alone
- [ ] Keyboard operable, with the same keys as the hero
- [ ] The claim about zenith is stated accurately and is checkable
