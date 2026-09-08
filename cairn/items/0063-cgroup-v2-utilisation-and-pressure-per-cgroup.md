---
id: 63
title: cgroup v2 utilisation and pressure, per cgroup
type: feature
status: done
milestone: v2.1
depends_on:
- 58
- 60
created: 2026-09-08
updated: 2026-09-08
priority: p1
area: collect
---

## Problem

atop's `G` view is a hierarchical cgroup v2 display with `CPUBUSY`, `CPUMAX`,
`CPUWGT`, `DISKIO`, `MEMORY`, `MEMMAX`, `SWPMAX`, `NPROCS`, `PBELOW` and — the
part that matters most — **PSI per cgroup**.

poptop reports PSI for the machine. On a container host that answers "something
is stalled on IO" and not "the thing stalled on IO is this pod", which is the
question. Per-cgroup pressure is the single most useful metric atop has that
poptop does not, because it is the only one that attributes a stall.

## Why it is the expensive one

`/sys/fs/cgroup` is a tree, and each node has `cpu.stat`, `memory.current`,
`io.stat`, `cpu.pressure`, `memory.pressure`, `io.pressure`. A node with a
thousand cgroups is thousands of reads a sample. This is the subsystem the cost
model in 0060 exists for, and it should not be built before it.

## What needs deciding

- **Depth.** atop defaults to seven levels and lets you set two to nine. A
  default that walks a Kubernetes node's whole tree is a mistake.
- **What to show at each level.** A leaf's own figures, or a subtree's rollup.
  The kernel gives the former; the latter is arithmetic poptop would own, and it
  has to be right about a cgroup with both processes and children.
- **Whether it is a view or a grouping.** A tree view is a third layout claiming
  the table's space, alongside the process tree and grouping, and all three are
  mutually exclusive. That may be the answer, or it may mean the table needs a
  general notion of what it is a table *of*.

## Acceptance criteria

- [x] Per-cgroup CPU, memory, IO and pressure, where the kernel publishes them
- [x] A stalled cgroup is identifiable from the pressure figure, not inferred
- [x] Depth bounded, with the bound stated
- [x] Cost measured on a tree of a thousand cgroups, and gated
- [x] cgroup v1 says it is unsupported rather than showing an empty tree

## How it was resolved

PR #81. `C` shows cgroups instead of processes: CPU, quota, memory, IO and
pressure per node, most pressured first.

**Depth four, and a cap of 512 nodes.** Four reaches a container on a Kubernetes
node, which is the level somebody is looking for; atop's default of seven is
every process's own scope. The walk is breadth-first, so hitting the cap leaves
whole levels rather than one branch followed to the bottom, and a truncated tree
renders `cgroups (first 512 of more)` — a reader hunting a stalled cgroup must
not be handed a list that silently does not contain it.

**The kernel's figures, not poptop's arithmetic.** In cgroup v2 `cpu.stat` and
`memory.current` are already subtree totals, so a parent reads higher than any
one child. Verified live: a spinner in `/demo.slice/busy.scope` reads 100.0%,
its parent 100.0%, the root 100.1%.

**A view, not a grouping** — it is a table of different things, and atop makes
the same call with `G`. It does not add a fourth exclusive mode to the process
table, so the deeper question 0070 asks stays open.

**Measured on a real 1017-cgroup tree: 7.26ms a sample against 150us without
it** — fifty times everything else combined. Collected only while the view is
open, and the first thing the budget takes away.

**Review caught the feature being dead.** `Source::Cgroups` was never added to
the Linux backend's `SUPPORTED` list, and `needs()` strips anything not in it —
so the key set a bit cleared on every tick and `C` showed "not collected here"
forever. The test that should have caught it began with an early return on that
same list, so it passed by not running. A cadence of two samples was the root of
three more bugs and is gone; the gate is the view being open.
