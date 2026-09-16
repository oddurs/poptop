---
id: 107
title: Zooming out hides volatility behind the peak
type: feature
status: backlog
milestone: v3.5
created: 2026-09-14
updated: 2026-09-14
priority: p1
area: ui
depends_on:
- 106
---

## Problem

Aggregation is by peak, never mean, and that is right: averaging a 100% spike
with three idle samples renders 25% and hides the event the tool exists to
catch.

But a cell showing 100 tells you a peak of 100 happened somewhere in it. It does
not tell you whether the machine sat at 100 for the whole cell or touched it
once. At zoom 30 that is thirty seconds of difference between "saturated" and
"a spike", and those are not the same problem.

## The fix

Draw the range, not only the peak. Each cell has a min and a max over the
samples it covers; draw the max as it is drawn now and the min–max span behind
it in a recessive weight. A cell where min and max are close is a solid load; a
cell where they are far apart is spiky. Candlestick logic, for the same reason
candlesticks exist.

At zoom 1 min equals max and the band vanishes, so this costs nothing at the
default and appears exactly when aggregation starts hiding something.

## Why nobody else does this

They aggregate at write time. A tool that stored the mean cannot recover the
range, and a tool that stored the peak cannot either. poptop still has the
samples, so the band is free.

## What needs deciding

- **Weight.** The band must not compete with the peak line. Chrome is taken by
  the threshold rules; a dimmed series hue is the obvious candidate and needs
  checking at the 16-colour and mono tiers.
- **Whether the band is the min or a quantile.** A single anomalous low sample
  drags the band to the floor. The honest answer is the min; the readable one
  may be a low quantile, and choosing the readable one needs saying out loud.

## Acceptance criteria

- [ ] A spiky cell is distinguishable from a saturated one at a glance
- [ ] The band is absent at zoom 1, not merely invisible
- [ ] The peak stays the most prominent mark in the cell
- [ ] Survives the mono tier
