---
id: 257
title: poptop cannot tell what the terminal can draw
type: feature
status: backlog
milestone: r14
labels:
- ui
- graph
created: 2026-09-23
updated: 2026-09-23
priority: p0
---

## Problem

The only capability poptop probes is the background colour, and it does that
well: a query, a bounded wait, and a documented fallback when the terminal
stays quiet. Everything else is assumed from `TERM`.

A pixel tier needs three answers the environment cannot give: whether the
terminal implements a graphics protocol, which one, and how many pixels a cell
is — the last one because a graph that does not know its own pixel size cannot
align with the text grid around it.

## Proposal

One probe, run once at startup beside the background query, bounded the same
way:

- Kitty graphics: a query image and its reply, which the protocol defines.
- Sixel: the primary device attributes reply, where `;4` says sixel.
- Cell size: `CSI 16 t`, and `TIOCGWINSZ`'s pixel fields as the fallback.
- tmux: detected, and the passthrough wrapper used only where the pane allows it.

A terminal that does not answer is a terminal without the tier. Silence is the
common case, not an error, and costs a bounded wait once.

## Acceptance criteria

- [ ] One probe, bounded, with every answer defaulting to "no"
- [ ] What it found is in `--once`'s notes and in the recorded day's assumptions, so a day file says which tier drew it
- [ ] A terminal that answers but is configured off stays off, and says nothing further about it
