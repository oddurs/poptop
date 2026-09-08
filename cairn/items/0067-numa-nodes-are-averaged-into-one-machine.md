---
id: 67
title: NUMA nodes are averaged into one machine
type: feature
status: done
milestone: v2.1
depends_on:
- 58
created: 2026-09-08
updated: 2026-09-08
priority: p3
area: collect
---

## Problem

atop has `NUM` (per-node memory: `tot`, `free`, `file`, `dirty`, `activ`,
`inact`, `slab`, `shmem`, `hugepages`, `frag%`) and `NUC` (per-node CPU).

On a two-socket machine poptop reports one memory figure and one CPU figure. A
box with one node exhausted and one idle reads as half full, and the process
pinned to the exhausted node is stalling on allocation while the header says
there is plenty.

## Honest scoping

This matters on large machines and not at all on a laptop, which is most of
where poptop runs. It is `p3` for that reason, and it is in the plan because
"no compromise parity" means the large machine is not written off — not because
it is urgent.

`/sys/devices/system/node/node*/meminfo` and `numastat` are the sources; one
read per node, so the cost is small and bounded by socket count.

## Acceptance criteria

- [ ] Per-node memory and CPU where the machine has more than one node
- [ ] A single-node machine spends no space saying it has one node
- [ ] macOS says nothing rather than reporting one node
