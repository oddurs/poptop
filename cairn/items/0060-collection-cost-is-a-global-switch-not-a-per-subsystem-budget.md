---
id: 60
title: Collection cost is a global switch, not a per-subsystem budget
type: feature
status: done
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

- [x] A subsystem declares its cost and its cadence
- [ ] Sampling stays inside its interval on a box with a thousand cgroups
- [x] Anything withheld for cost is stated, not silently missing
- [x] Measured with `--bench` on a machine with each subsystem present

## How it was resolved

PR #79. Gating is per **source** — the things that cost differently are sources
(`/proc/<pid>/io`, `/proc/<pid>/task/`, a cgroup walk), not individual fields —
and each declares its cost per unit, whether that cost grows with the machine,
and how many samples apart it is worth reading.

**Who decides: both.** Pull decides what is *worth* gathering; a budget decides
what the machine can *afford*. The objection to a budget is not that it is wrong
but that it can silently drop a figure, so it never does: everything given up is
named in the panel until the reader asks for that source back by name.

**Cadence has a real user, not a speculative one.** The clock policy rescan was
one of the three ad-hoc answers this item complains about; that rule now lives
with every other source's and the collector reads it.

**Two things review corrected, both worth recording.** Ranking by cost *per
unit* gives up per-process IO on a box whose threads cost four times as much —
2.3ms against 9.9ms at 400 processes — so cost is now weighted by what the last
sample found. And the budget was charged for mandatory work it could not give
up, so a box whose baseline exceeds the budget lost every optional source to a
banner blaming them; it now gives up a source only when that source could
account for the overrun, and otherwise says the interval is too short.

Backends declare which sources they read, so `y` on macOS no longer starts a
collection that will never produce a row.

## Criterion 2 is deliberately unticked

"Sampling stays inside its interval on a box with a thousand cgroups" cannot be
demonstrated by this item: cgroups arrive in v2.1. What this delivers is the
mechanism that will hold it, tested against a synthetic over-budget sample.
Ticking it on a machine with no cgroups to walk would be the kind of claim this
project does not make. It is 0063's to close.
