---
id: 210
title: Energy is a tab poptop cannot fill
type: feature
status: done
milestone: v4.0
created: 2026-09-15
updated: 2026-09-15
priority: p3
area: collect
depends_on:
- 202
---

## Problem

Activity Monitor's Energy tab is the one with no obvious translation. It shows
an energy impact score, average impact, app nap state, and whether something
prevented sleep — and every one of those is a macOS abstraction with no Linux
counterpart and no public API.

A tab strip with an Energy tab that says "not available" on Linux is worse than
one without it. A tab strip without it on both is a plan that gave up before
looking.

## What is actually available

- **Linux**: RAPL via `/sys/class/powercap/intel-rapl` gives package and DRAM
  energy in microjoules, cumulative, per socket. Readable without root on most
  distributions. Per-process attribution does not exist and cannot be derived
  honestly — the counter is per package.
- **macOS**: `powermetrics` gives per-process energy impact and needs root. The
  `task_power_info` call gives per-task interrupt wakeups without root, which is
  most of what Activity Monitor's score is built from.
- **Both**: idle wakeups are the honest cross-platform proxy, and they are the
  column Activity Monitor puts beside its score. poptop already collects thread
  counts and could collect wakeups on both platforms.

## The shape this suggests

Not "energy", which poptop cannot measure per process on either platform, but
**wakeups** — which it can, which is what actually costs battery, and which is a
real column rather than a score whose formula is not public.

The machine-level draw from RAPL belongs in the summary strip, where a figure
with no per-process breakdown is not pretending to have one.

## What was decided

**There is no Energy tab, and there should not be one.** It would be one column
deep and the column cannot be filled: `CONFIG_SCHEDSTATS` is off by default on
most distributions, so `/proc/<pid>/sched` has no `nr_wakeups` to read, and
`task_power_info` needs root on macOS. A tab that says "not available" on both
platforms is worse than no tab.

**What shipped is the machine's switch and interrupt rate**, which poptop was
already collecting on Linux and showing nowhere but `--export`:

    CSW 5202/s  IRQ 3573/s  LOAD 14.64 13.02 10.48  │  MEM  12.9% …

Measured, not scored. A machine thrashing between threads looks identical to a
busy one without it, which is the diagnostic Activity Monitor's energy score is
reaching for by a route whose formula is not public.

**It is machine-level and does not pretend otherwise.** It sits with the other
machine figures, and nothing suggests a per-process breakdown exists.

**macOS shows nothing rather than zero.** There is no `/proc/stat`, and a zero
there would be a fabricated figure about the one thing the row exists to notice.
The interrupt half alone is an em dash for the same reason.

**It is ranked just above `LOAD`**, which means a busy header drops it — on a
two-hundred-column terminal with NFS, stall and filesystem figures present it is
not drawn at all. That is the ladder working: it is a niche figure, and the
people who want it know to widen the window or read `--export`.

## What RAPL would add, and why it did not ship

`/sys/class/powercap/intel-rapl/*/energy_uj` gives package and DRAM energy in
microjoules, cumulative, per socket, readable without root on most
distributions. Differentiated over the interval it is watts, and it belongs
beside these figures.

It did not ship because neither machine available to develop on has it — a Mac
has no RAPL and Docker on a Mac is a VM without the sysfs tree. Writing a
collector that cannot be run once before it is claimed to work is the thing this
repo's verification rule exists to stop.
