---
id: 250
title: There is one window because there is one graph
type: feature
status: backlog
milestone: r12
labels:
- ui
- graph
created: 2026-09-23
updated: 2026-09-23
priority: p1
---

## Problem

The window, the zoom and the slot grid are derived per drawing from
`shown_window(app, area)`. That works because there is one panel and the
sparklines borrow its answer. It is derived four times already — the timeline,
the sparklines, the cursor row and the mouse — and each derivation is a chance
for two pictures to disagree about which instant a column is.

A dashboard makes that certain rather than likely: a dozen tiles, each deriving
its own window from its own rectangle.

## Proposal

One `TimeAxis`, owned by the app and passed to whatever draws: the window, the
zoom, the slot boundaries (already anchored to absolute positions), the cursor,
and the answer to "which instant is column N". Panels ask it; nothing derives
it.

## Acceptance criteria

- [ ] One derivation, used by the timeline, the sparklines, the cursor and the mouse
- [ ] Column N is the same instant in every drawing on the frame, held by a test that renders two panels and compares
- [ ] Scrubbing moves every drawing at once, because there is one cursor to move
