---
id: 82
title: The page never states the problem it solves
type: feature
status: done
milestone: web
depends_on:
- 56
created: 2026-09-08
updated: 2026-09-08
priority: p0
effort: m
area: web
---

## Problem

The landing page opens with an answer. There is no section that says what goes
wrong, so the reader has to supply the problem themselves, and the three claims
under "what it is for" read as a feature list because nothing has been set up
for them to be the answer to.

The problem is specific and every reader of this page has lived it: the box
spiked, the alert fired, you connected, and by the time the monitor painted,
everything was calm. The evidence was gone before you arrived. Every system
monitor shows the present, and the present is never when the problem happened.

## Proposal

A section that shows it rather than says it: two frames of the same machine,
side by side.

**Left** — what you see when you arrive. Flat plots, CPU in the single digits,
a process table with nothing above 4%. Drawn in `--ink-faint` and
`--ink-dim`: quiet, correct, and useless.

**Right** — the same buffer forty seconds earlier. The spike, and postgres named
in the table at 74%. Full colour.

Between them a hairline carrying `40s`, the way the timeline gutter carries a
scale.

Both are stills from the same buffer the hero uses, at two cursors, so the
claim is not a mock-up — it is the same data the reader can scrub to for
themselves in the frame above.

Copy is one line above and one below. The pictures are the argument.

Cut the current three-claim "what it is for" row in the same change: this
section and the three that follow it say all of it, and say it with pictures.

## Acceptance criteria

- [ ] Two stills of the same buffer at two cursors, side by side
- [ ] The later one is visibly quiet; the earlier one is visibly the incident
- [ ] The gap between them is labelled with the real interval
- [ ] They stack on a narrow screen without either becoming unreadable
- [ ] The three-claim row is gone
- [ ] Neither still is focusable or responds to keys
