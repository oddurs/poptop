---
id: 126
title: Energy is a tab poptop cannot fill
type: feature
status: backlog
milestone: v4.0
created: 2026-09-15
updated: 2026-09-15
priority: p3
area: collect
depends_on:
- 118
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

## What needs deciding

- **What to call it.** "Energy" promises the macOS thing. "Wakeups" is honest
  and means nothing to most people.
- **Whether a tab that is one column deep earns a tab.** It may belong in the
  CPU tab, as it does in the screenshot — Idle Wake Ups is a CPU column there.

## Acceptance criteria

- [ ] Whatever it shows, it is measured rather than scored
- [ ] A platform that cannot supply it says so rather than showing zero
- [ ] Machine-level draw is not presented as per-process
