---
id: 120
title: A summary strip with the same shape on every tab
type: feature
status: backlog
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

## What needs deciding

- **Whether the machine figures leave the top entirely.** A monitor with nothing
  at the top for the first second of a run reads as not having started.
- **What Memory's middle graph shows.** Activity Monitor has memory pressure,
  which is a derived signal poptop does not compute.

## Acceptance criteria

- [ ] Three zones, same order, same places, on every tab
- [ ] A zone that has nothing to say is blank rather than collapsed
- [ ] Degrades by dropping a zone whole, never by rearranging
- [ ] The timeline is untouched
