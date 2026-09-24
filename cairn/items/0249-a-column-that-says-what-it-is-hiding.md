---
id: 249
key: r12
title: A column that says what it is hiding
type: milestone
status: backlog
labels:
- ui
- graph
created: 2026-09-23
updated: 2026-09-23
priority: p0
---

The second milestone: what a graph draws, and what it is allowed to leave out.

r11 makes any mark cheap to draw. This decides which marks earn their place,
and fixes the three things that make the present graphs say less than the
buffer knows:

- A column is one number, so thirty seconds at 100% and one spike to 100% are
  the same picture (0191).
- The floor is always zero, so a series living between 72% and 85% spends most
  of the panel on ink that never moves (0190).
- Every panel scales to its own peak, so stacking them invites a comparison
  they do not support (0196).

The marks that answer those: an **envelope** — the min-max band with the mean
through it — a **heat strip** for the many-of-a-kind series a single graph
cannot hold (0194), and a **stacked area** for the figures that are a
composition rather than a number.

And the two rules that will make the dashboard in r13 read as one picture
rather than a wall of graphs, both of which are cheap to establish now while
there is one panel to get them right in: **one clock** for every drawing, and
**scale groups** so comparable units share a ceiling.

The constraint the whole milestone works under is the one the aggregation
already keeps: a mark may summarise, and may never invent. A band drawn from
min and max is two measured values; a mean line through it is arithmetic on
measurements; neither is allowed to become the only thing on screen, which is
what averaging did before peak replaced it.
