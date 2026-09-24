---
id: 259
title: A pixel graph costs bandwidth nobody budgeted
type: feature
status: backlog
milestone: r14
labels:
- ui
- graph
created: 2026-09-23
updated: 2026-09-23
priority: p2
---

## Problem

A naive implementation sends a fresh image every frame. At 800×240 RGB that is
half a megabyte a second at a one-second interval, which is nothing locally and
is somebody's ssh session over a hotel connection.

poptop's whole position is that it is the tool you can run on a box you have
just connected to.

## Proposal

Upload once, place many: the image is uploaded when its content changes and
placed by id afterwards, compressed, and redrawn only where the picture
actually moved — which, now that a finished column never changes, is the newest
column and whatever the cursor is over.

A budget, measured and stated like the collection budget: bytes a second at the
default interval, and a fallback to the character tier if the terminal cannot
keep up.

## Acceptance criteria

- [ ] Bytes a second measured at the default interval and written into the item
- [ ] Redraw is proportional to what changed, not to the panel
- [ ] Over a slow link the tier degrades to characters rather than lagging the interface
