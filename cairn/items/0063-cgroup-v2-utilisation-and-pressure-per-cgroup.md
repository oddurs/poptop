---
id: 63
title: cgroup v2 utilisation and pressure, per cgroup
type: feature
status: backlog
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

- [ ] Per-cgroup CPU, memory, IO and pressure, where the kernel publishes them
- [ ] A stalled cgroup is identifiable from the pressure figure, not inferred
- [ ] Depth bounded, with the bound stated
- [ ] Cost measured on a tree of a thousand cgroups, and gated
- [ ] cgroup v1 says it is unsupported rather than showing an empty tree
