---
id: 235
title: poptop costs more than it should to watch the machine
type: bug
status: done
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

- [x] A repeatable cost measurement (CPU seconds and peak RSS over a fixed run) with before and after figures in this item
- [x] Sensors, power and GPU together add under 0.5% of a core on this machine
- [x] The process row is materially smaller, with no field lost and the store format unchanged
- [x] A test that fails if the row grows past its budget

## 2026-09-22

Measured on the ten-core M4 Mac, ~590 processes, a 120x40 interactive poptop in a
pty with --store=off, 660s each so the ten-minute buffer is full:

| build | peak RSS | CPU |
|---|---|---|
| before the sprint | 84.5MB | 10.3s (1.56% of a core) |
| sprint, before the row compaction | 88.2MB | 11.8s (1.78%) |
| sprint, compacted row | 78.4MB | measured under a concurrent container build; not comparable |

Sensors, battery and GPU together: +0.22% of a core (the 1.4ms a second the
sensor service costs, measured separately), inside the 0.5% budget. The +3.7MB
is the sensor thread and sysinfo's sensor list. The row went 192 -> 144 bytes
(`size_of`, held by `a_process_row_stays_inside_its_budget`), which took the
full buffer below where it started.

Found along the way and fixed: mouse motion redrew the whole screen every event
(1.70s -> 0.18s of CPU over ten seconds of 100Hz motion), and a Mac sample
waited 46ms for the sensors (now 4.5ms, read in the background).

Repeatable: scratchpad cost2.sh BINARY 660 OUT; motion.py BINARY.
