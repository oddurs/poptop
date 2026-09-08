---
id: 60
title: Collection cost is a global switch, not a per-subsystem budget
type: feature
status: backlog
milestone: v2.0
created: 2026-09-08
updated: 2026-09-08
priority: p1
area: collect
---

## Problem

`Needs { io: bool }` is the whole capability model. It gates one thing, and it
was added because per-process IO costs a file read per process.

v2.1 adds subsystems with wildly different costs, measured where possible:

| subsystem | cost | note |
|---|---|---|
| per-process io | ~1 read/process | already gated |
| cgroup v2 walk | one read per cgroup, per metric | unmeasured, likely the worst |
| NUMA | one read per node | cheap |
| per-process network | needs eBPF or inode matching | its own project |
| GPU | needs a daemon or NVML | optional |
| `smaps_rollup` for PSS | one read/process | atop gates this behind `R` |

A single boolean cannot express "read this every tenth sample", "read this only
while its panel is open", or "stop reading this because it costs more than the
interval".

## What poptop already does right, and should generalise

The IO probe withdraws the columns when most of them are unreadable, and says
so. The command-line cache re-reads on a slot staggered by pid. The clock
ceiling rescans its policy set once a minute. Three ad-hoc answers to the same
question.

## What needs deciding

- **The unit of gating.** Per subsystem, or per metric. Per metric is finer and
  multiplies the bookkeeping.
- **Who decides.** The panel that needs it (pull), or a budget that turns things
  off when the sample runs long (push). A budget is more robust and can surprise
  a reader by silently dropping a figure — which this codebase does not do
  without saying so.
- **What the reader is told.** poptop's rule is that an absence is stated. A
  subsystem switched off for cost has to say so somewhere, once.

## Depends on

The metric registry — a cost and a cadence belong in a metric's declaration
rather than in a second table that has to be kept in step.

## Acceptance criteria

- [ ] A subsystem declares its cost and its cadence
- [ ] Sampling stays inside its interval on a box with a thousand cgroups
- [ ] Anything withheld for cost is stated, not silently missing
- [ ] Measured with `--bench` on a machine with each subsystem present
