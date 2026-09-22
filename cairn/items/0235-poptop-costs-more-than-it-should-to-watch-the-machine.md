---
id: 235
title: poptop costs more than it should to watch the machine
type: bug
status: backlog
milestone: r9
labels:
- collect
- perf
created: 2026-09-22
updated: 2026-09-22
priority: p0
---

## What happens

A monitor is the one program that has to be cheap, because its cost is part
of what it reports. Measured on the ten-core Mac this milestone was written on,
at 760 processes:

- **CPU:** about 1% of a core at a one-second interval, idle in the interface.
  Acceptable, and nothing in this milestone may raise it noticeably.
- **Memory:** a process row is 192 bytes and the default buffer holds 601
  samples, so the rows alone come to about 88MB once ten minutes have passed —
  and a recorder left running at a longer window was seen at 488MB. The
  buffer is the cost, and it grows with the machine rather than with anything
  the reader asked for.

## What should happen

Every source added here is measured before and after, on CPU and on memory,
and the numbers are written down. The per-sample footprint shrinks where it can
without losing a figure: fields that are almost always absent or small should
not cost sixteen bytes each in every row of every sample.

## Acceptance criteria

- [ ] A repeatable cost measurement (CPU seconds and peak RSS over a fixed run) with before and after figures in this item
- [ ] Sensors, power and GPU together add under 0.5% of a core on this machine
- [ ] The process row is materially smaller, with no field lost and the store format unchanged
- [ ] A test that fails if the row grows past its budget
