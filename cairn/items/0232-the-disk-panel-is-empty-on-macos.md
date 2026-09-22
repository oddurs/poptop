---
id: 232
title: The disk panel is empty on macOS
type: feature
status: done
milestone: r9
labels:
- collect
created: 2026-09-22
updated: 2026-09-22
priority: p1
---

## Problem

The disk figures come from `/proc/diskstats`, so on macOS the panel and the
tab say nothing. The OS keeps per-volume byte counters that any user can read,
and sysinfo — already the macOS backend — exposes them.

## Proposal

Read them, turned into rates against the time since the last read as the
network counters are. APFS reports one physical store under several mount
points, so volumes are merged by the device they are on or the same traffic is
counted twice. Utilisation, queue depth and latency are not published there and
stay absent rather than zero.

## Acceptance criteria

- [x] macOS disk read and write rates in the header, tab and timeline
- [x] One entry per device, not per mount
- [x] Figures macOS does not publish render as unknown
