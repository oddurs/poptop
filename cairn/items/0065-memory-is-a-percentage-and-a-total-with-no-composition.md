---
id: 65
title: Memory is a percentage and a total, with no composition
type: feature
status: done
milestone: v2.1
depends_on:
- 58
created: 2026-09-08
updated: 2026-09-08
priority: p2
area: collect
---

## Problem

`MemStat` is `total`, `used`, `available`, an optional `free`, and swap. atop's
`MEM` line carries `cache`, `dirty`, `buff`, `slab`, `slrec`, `shmem`, `shrss`,
`shswp`, `pgtab`, `hptot`, `hpuse`, `zfarc`, `anthp`, `tcps`, `udps`.

The ones that answer a question poptop currently cannot:

- **dirty** — pages waiting to be written. A box with gigabytes dirty is about
  to stall on IO and every present figure looks fine until it does.
- **slab** and **slrec** — kernel memory, and how much of it is reclaimable. A
  leak here presents as "used" memory belonging to no process, which is
  precisely the case where the process table cannot explain the header.
- **pgtab** and **hugepages** — on a database box these are large and invisible.
- **shmem** — shared memory counted once, which is what makes per-process RSS
  sum to more than the machine has.

`free` is already `Option` and documented as unavailable on macOS because `used`
and `available` overlap there. That doc is the right shape for all of these.

## What needs deciding

- Which of these earn header space and which are detail. The header is already
  a degradation ladder; most of these belong behind a memory view rather than in
  the top two lines.
- Whether the memory composition bar becomes more than two segments. It was
  drawn as a composition specifically so "37% used" could not hide the
  difference between free memory and reclaimable cache — the same argument
  extends to slab and dirty.

## Acceptance criteria

- [x] dirty, slab, reclaimable slab, shmem, page tables and hugepages, where
      the platform publishes them
- [x] Anything macOS cannot partition stays absent rather than being derived
- [x] The composition bar is honest about what it does not account for
- [x] Measured: `/proc/meminfo` is already read, so no additional reads

## How it was resolved

PR #86. All seven figures, from a file already read and parsed, so this costs
nothing.

**One earns header space:** `DIRTY`, above 5% of memory. A box with a fifth of
its memory awaiting writeback is about to stall and every other figure looks
fine until it does; a few megabytes is what an ordinary machine always carries.
Coloured on its own scale, the third figure now following `stall_heat`'s
argument. The rest go to `--once`.

**The composition-bar honesty** is answered by naming what `used` contains
rather than by adding segments: `used` is `total - available` and already holds
every one of these, which is why a kernel memory leak appears there as used
memory belonging to no process — the exact case where the process table cannot
explain the header.

**Absence is preserved**: an absent line is an em dash, which needed a second
helper because the existing one reports a missing figure as zero. Huge pages are
the trap — counted in *pages of `Hugepagesize`*, not kilobytes, so the shared
helper would report a two-megabyte page as two kilobytes.

**Review caught a ladder bug worth more than the figure**: `DIRTY` was ranked
*above* the `MEM` it qualifies, so a narrow header dropped memory and kept its
component. There is now a test asserting no qualifier outlives its parent at any
width, which covers `CLK` and `STL` too.
