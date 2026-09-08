---
id: 53
title: A throttled machine looks identical to a busy one
type: feature
status: done
milestone: v1.0
created: 2026-09-07
updated: 2026-09-07
priority: p2
area: collect
---

## Reconsidering an earlier decision

When the storage and network items were scoped, temperature was dismissed as
"mostly a laptop curiosity, inconsistent across hardware". btop, bottom and
glances all carry it, and the reasoning for leaving it out was wrong — because
it framed the figure as *temperature* rather than as *throttling*.

Thermal and power throttling is a direct answer to "why is this slow", and it is
the one cause poptop currently cannot show at all:

- The CPU reports 100% busy and gets less work done than it did an hour ago.
  Every figure poptop draws looks identical in both cases.
- A laptop on battery, or one with a blocked vent, or a server with a failed
  fan, all present as "busy but slow" with nothing on screen to explain it.
- `STALL`, `WAIT` and disk saturation all read normal, because nothing is
  waiting — the work is simply being done more slowly.

That is a blind spot in exactly the question the tool is built around.

## What the figure should be

Not a temperature. Temperature is a proxy and an inconsistent one; the machine
knows whether it is throttling and says so:

- Linux: `/sys/devices/system/cpu/cpu*/cpufreq/scaling_cur_freq` against
  `cpuinfo_max_freq` gives the ratio directly. Thermal throttle counters live in
  `/sys/class/thermal/` and, on Intel, in `therm_throt` under `/sys/devices/system/cpu`.
- macOS: `pmset -g therm` reports CPU speed limit as a percentage; whether that
  is reachable without shelling out needs checking.

A figure reading "running at 62% of nominal clock" says the thing. A figure
reading "84°C" makes the reader do the inference, and on hardware whose nominal
temperature is 85°C it makes them do it wrongly.

## What needs deciding

- Whether the ratio is available cheaply on both platforms, and what happens on
  the one where it is not — an em dash, as everywhere else.
- Whether it belongs in the compute group of the header beside `CPU`, which is
  what it qualifies.
- Whether a machine that is not throttling should show it at all, or whether
  this is another figure that appears only when it has something to say.

## Acceptance criteria

- [x] A throttled machine says so, in units of lost clock rather than degrees
- [x] The figure is absent rather than zero where the platform will not say
- [x] Measured: the cost of reading it per sample
- [x] A machine running at nominal clock does not spend header space saying so
