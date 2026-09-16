---
id: 120
title: A summary strip with the same shape on every tab
type: feature
status: done
milestone: v4.0
created: 2026-09-15
updated: 2026-09-15
priority: p1
area: ui
depends_on:
- 118
---

## Problem

poptop's header is a row of figures that rearranges itself. Its degradation
ladder is good work — it drops figures by rank as the terminal narrows — and the
result is that the same machine looks different at two widths, and no position
on that row means anything.

Activity Monitor's summary is the opposite and it is the better idea: three
zones, the same three on every tab, in the same order and the same places.
Named scalars on the left, the graph in the middle, counts on the right. The
zones do not move. Only their contents change with the tab.

## The fix

A strip below the table — after the detail, like Activity Monitor, because your
eye lives in the table and the aggregate is a glance down:

```
  System    22.4%  │        CPU LOAD        │  Threads      8,158
  User      66.1%  │   ▁▂▃▅▂▁▂▇█▅▃▂▁▃▅▇█▅▃  │  Processes      792
  Idle      11.5%  │                        │  Running          4
```

Per tab, the same three zones:

| tab | scalars | graph | counts |
|---|---|---|---|
| CPU | system / user / idle | load | threads / processes / running |
| Memory | used / cached / swap | pressure | processes / compressed |
| Disk | read / written | throughput | reads / writes |
| Network | in / out | throughput | packets in / out |

## Why below and not above

Activity Monitor's hierarchy is navigation, detail, summary, and the summary is
last because it is supporting. poptop currently leads with the summary, which
puts the least specific thing first and the thing you came for third.

The timeline is the exception and stays where it is — above the table, full
width. It is not a widget in this strip, and the small graph in the middle zone
is a *different* thing: sixty seconds of one series, at a glance, the way
Activity Monitor's is.

## What was decided, and what was declined

**The zone discipline was already there and was not verified.** `fit` keeps
figures by rank and *positions* them by `Group`, and those are deliberately
different orders — so a figure never moves as the terminal resizes, it only
appears and disappears. That is the whole of "position means something", it has
been true for a while, and nothing tested it.
`a_figure_never_moves_it_only_appears_and_disappears` walks every width from 40
to 200 and asserts the figures present are a *subsequence* of the widest set,
which is exactly "dropped, never rearranged".

**The three-row strip at the bottom was declined.** In a terminal the scarcest
resource is rows, and a second summary costs three of them off the table while
saying what the header already says in one to three. The repo's own objection
applies — "a reminder that is always on screen twice is not a reminder, it is
noise" — and it applies to summaries too.

**Making the header follow the tab was tried and is wrong.** Biasing `fit`
toward the tab's own `Group` promoted the load average, because `Compute` holds
both the two figures that answer "why is this slow" and the one that conflates
them. `Group` says where a figure sits, not how much it explains. The header is
about the machine, and that question has the same answer whichever table you are
reading; the tab governs the columns. `the_zones_stay_in_one_order_across_every_tab`
pins that.

What is left undone is the blank-rather-than-collapsed criterion: a group with
nothing to say still closes up rather than holding its space. That is a real
difference from Activity Monitor and it is a deliberate one — holding empty
space on an 80-column terminal costs more than it buys.

## Acceptance criteria

- [x] Three zones, same order, same places, on every tab
- [ ] A zone that has nothing to say is blank rather than collapsed — declined
- [x] Degrades by dropping figures, never by rearranging
- [x] The timeline is untouched
