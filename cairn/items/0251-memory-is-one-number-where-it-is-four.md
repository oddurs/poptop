---
id: 251
title: Memory is one number where it is four
type: feature
status: backlog
milestone: r12
labels:
- ui
- graph
created: 2026-09-23
updated: 2026-09-23
priority: p2
---

## Problem

The memory graph draws used percent. The sample holds used, available, cache,
slab, dirty, shared and swap, and the shape of a machine that is filling with
page cache is different from one filling with anonymous pages — the first is
fine and the second is about to swap. One line cannot tell them apart.

The same is true of CPU: busy is user, system, irq, softirq, steal and iowait,
and which one is climbing is the diagnosis.

## Proposal

A **stacked area** mark: a series of bands summing to a whole, each in its own
ink, with the composition named in the gutter or the caption. Only for figures
that genuinely partition — the places where the parts sum to the total and the
platform publishes every part. Where a part is absent the band is absent, not
zero, and the caption says the stack is incomplete.

## Acceptance criteria

- [ ] A stacked mark, used by memory composition and CPU classes where the platform publishes them
- [ ] A missing part leaves a gap and says so, rather than being folded into another band
- [ ] The ink is the palette's existing series colours, checked against the colour-vision rules
