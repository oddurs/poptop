---
id: 231
title: Nothing says how hot the machine is
type: feature
status: backlog
milestone: r9
labels:
- collect
depends_on:
- 229
created: 2026-09-22
updated: 2026-09-22
priority: p1
---

## Problem

A machine that is thermally throttling reads as a machine that is busy. The
clock ceiling catches a driver lowering its policy maximum; it cannot catch a
die at 100°C that has not been capped yet, a laptop's fans at full speed, or an
SSD that slowed down because it got hot. Every one of those is readable by an
ordinary user: `/sys/class/hwmon` on Linux, the HID sensor services on macOS
(forty of them on the machine this was measured on).

## Proposal

A `Sensors` source: temperatures and fan speeds, grouped into what they are
about — CPU, GPU, storage, other — with the hottest of each group the figure
shown, because forty labels like `PMU tdie6` are not a reading anyone can use.
Read on a slower cadence than the interval, since die temperature moves over
seconds and one read cost 40ms, and gated like every other source so the budget
can give it up. A row on the timeline and a figure in the header when there is
anything to show; nothing at all on a machine that publishes nothing.

## Acceptance criteria

- [ ] Linux: hwmon temperatures and fans, with labels and critical thresholds where published
- [ ] macOS: component temperatures, grouped
- [ ] The hottest CPU temperature in the header and on the timeline
- [ ] Recorded, exported, and absent (not zero) where unreadable
- [ ] Read on its own cadence and charged to the budget
