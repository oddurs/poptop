---
id: 60
title: Draw peak versus mean instead of asserting it
type: feature
status: done
milestone: web
depends_on:
- 56
created: 2026-09-08
updated: 2026-09-08
priority: p1
effort: s
area: web
---

## Problem

"Zooming aggregates by peak, never mean" is the most convincing detail in the
project — it is the difference between a tool that can answer the question and
one that quietly cannot — and on the landing page it is a sentence in a claim
card.

It is also inherently visual, which makes leaving it as prose a waste.

## Proposal

The same twenty samples, drawn twice, side by side.

**Mean** — one saturated second averaged with three idle ones. The plot renders
around 25% and the spike is simply not there.

**Peak** — the same samples. The spike is full height.

Under each, the figure that slot reports. Above, one line: *averaging a
saturated second with three idle ones renders 25%, and hides the exact event the
tool exists to catch.*

Both drawn by the same code from the same samples, with only the aggregate
option differing — which is why item 56 puts that option in `frame.js` rather
than special-casing it here.

## Acceptance criteria

- [ ] Two plots from one set of samples, differing only in aggregation
- [ ] The spike is absent from one and full height in the other
- [ ] The reported figure under each is computed from the samples
- [ ] Reads on a narrow screen, stacked
