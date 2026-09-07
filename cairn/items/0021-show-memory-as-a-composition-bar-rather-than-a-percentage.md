---
id: 21
title: Show memory as a composition bar rather than a percentage
type: feature
status: done
milestone: v0.2
created: 2026-09-06
updated: 2026-09-06
priority: p1
effort: s
area: ui
---

## Problem

Memory is reported as `MEM 37.5% (6.0G / 16.0G, 8.0G avail)` — a percentage and
three figures spending a third of the header line to say one thing badly.

The percentage is also the least interesting part. "37.5% used" does not
distinguish a box with plenty of headroom from one whose page cache is the only
thing standing between it and the OOM killer, because the number that matters
is the split between what is in use, what is cache the kernel will hand back
under pressure, and what is genuinely free.

## Proposal

One bar, drawn in the header, showing the composition:

    MEM 37.5% ███▍░░░ 6.0G/16G

The bar is the reason the graph row can be given up in item B: composition in
one column is more diagnostic than magnitude across sixty, because the shape of
memory is what tells you how much trouble you are in and the level is not.

Uses the existing `micro_bar` glyph machinery and the established status
colours, so it costs no new rendering vocabulary. Must remain legible at the
mono tier, where the segments separate by glyph rather than hue — the same
constraint every other meaning-bearing element in this project is held to.

## Acceptance criteria

- [ ] Header shows used / cache / free as one bar
- [ ] Legible at the mono tier, without relying on colour to separate segments
- [ ] Degrades on a narrow terminal like the rest of the header
- [ ] Swap keeps a figure where it is meaningful, and says nothing where there is no swap
