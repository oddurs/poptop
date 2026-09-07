---
id: 32
title: The timeline can only draw percentages
type: feature
status: backlog
milestone: later
created: 2026-09-07
updated: 2026-09-07
priority: p3
area: ui
---

## Problem

`draw_timeline` assumes every series is a percentage of a fixed denominator. It
prints the ceiling at the top of each row, rules the warn and critical
thresholds across the graph, and formats the cursor readout as `{name} {v:.1}%`.

That is true of CPU, WAIT, MEM, DISK and STALL. It is not true of anything
measured in bytes per second, which has no denominator at all — a link's
capacity is not portably knowable, and normalising to the window's own peak
makes the busiest sample 100 by construction. An idle laptop moving 8 B/s of
loopback drew a full-scale graph through the critical rule and read `NET 100.0%`
under the cursor.

So the network throughput row was dropped rather than shipped saying that. The
header figure carries the number.

## What should happen

A series should be able to carry its own scale:

- a ceiling in its own units, rounded to something legible
- an axis label in those units rather than a percentage
- threshold rules suppressed, since warn and critical are percentages
- a cursor readout formatted by the series rather than by the panel

Per-series ceilings already exist — `ceiling_for` runs per candidate — so the
missing pieces are the label, the rules and the readout.

## Acceptance criteria

- [ ] A byte-valued series renders with a byte axis and no threshold rules
- [ ] The cursor readout uses the series' own units
- [ ] The percentage series are unchanged, asserted by their existing tests
- [ ] The network throughput row comes back
