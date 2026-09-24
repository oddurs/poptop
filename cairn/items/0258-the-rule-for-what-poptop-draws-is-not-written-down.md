---
id: 258
title: The rule for what poptop draws is not written down
type: docs
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

0068 wrote down what poptop will *read*: what the kernel publishes to an
ordinary reader, with anything needing a daemon or a vendor library as an
optional source that enriches and is never required. That rule has settled
every question since, including the GPU one it originally declined.

There is no equivalent for what poptop *draws*, and a pixel tier is exactly the
change that needs one — it is the first time the picture depends on the
terminal rather than on the data.

## Proposal

The renderer's rule, in `docs/design/`, in the same shape:

**poptop draws what the terminal shows an ordinary reader. A richer tier draws
the same picture, more finely, and is never required.**

With the consequences spelled out: no figure exists only in pixels; a tier that
cannot be probed is off; the character tiers are what the README shows and what
the tests assert; and a recording says which tier drew it.

## Acceptance criteria

- [ ] The rule written down, with the pixel tier as its worked example
- [ ] A test that renders one series through every surface and asserts they agree on scale, floor, gaps and thresholds
- [ ] The platforms reference says which terminals reach the tier
