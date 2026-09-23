---
id: 177
title: Nothing can be zoomed into
type: feature
status: backlog
milestone: v5.0
depends_on:
- 149
created: 2026-09-13
updated: 2026-09-13
priority: p0
area: ui
---

## Problem

Every panel shows its summary and stops. `v` cycles the process table's columns,
`C` swaps the whole table for cgroups, `d` replaces the machine's graph with one
process's — three different keys, three different mechanisms, each hard-coded to
one thing, and none of them generalises.

A dashboard wants one gesture: **put the thing under the cursor on the whole
screen, in its fullest form**. Then put it back.

## What it looks like

`Tab` moves focus between panels, `z` zooms the focused one. Zoomed, a panel is
not the same chart made larger — it is the panel's third level of detail:

```text
 LIVE  CPU 61.2%  │  MEM 81.4%  SWP 12%  │  / 86% full  │  UP 6d  PROCS 412
▸ CPU 61.2%   peak 98.4% at 03:04:12   ·   z back
                                              ╭────────╮
                                              │        │
                     ╭──╮     ╭╮        ╭╮    │        │
                    ╭╰  ╯╮   ╭╰╯──╮   ╭─╰╯╮   │        │
                 ╭╮ │    ╯╮ ╭╰    ╯╮  │   ╯─╮ │        │
                ╭╰╯─╰     ╯─╰      ╯──╰     ╯─╯        ╰──────────────
────────────────╯
per core
  ▅▄▇▇▁  ▅▅▇▇▁  ▅▄▇▇▁  ▅▄▇▇▁  ▅▄▇▇▁  ▅▄▇▇▁  ▅▅▇▇▁  ▅▄▇▇▁
top by cpu · 87% of the machine figure accounted for
  4823 postgres    34.2    3.1G  postgres
  9912 build       22.8    412M  cc1plus
```

Three things arrive together and none of them fits in a summary panel: the graph
at full width, the **breakdown** (per core, per device, per node, per mount), and
**the processes responsible**, with the unattributed remainder stated — which is
0082's explanation, rendered where somebody asked for it.

## Why this is the right shape for poptop specifically

Zoom is a navigation idea in most dashboards. Here it composes with the one poptop
already has: the cursor is a moment in time, and a zoomed panel is a *moment* seen
in depth. Scrub while zoomed and the breakdown follows the cursor — which is the
thing no live-only monitor can do, on the panel where it matters most.

It also subsumes three existing keys. `C` is the cgroup panel zoomed. `d` is the
process panel zoomed onto one row. `v`'s memory view is the memory panel zoomed.
Replacing three bespoke mechanisms with one is most of the value here, and it is
also most of the risk.

## What needs deciding

- **What focus looks like, and whether it is worth a key.** A dashboard with
  `Tab` and `z` has two more keys than a stack with none. The alternative is
  zooming the panel the cursor is already in, which is fewer keys and less
  control. Prototype both before choosing.
- **Whether zoom is a mode or a state.** It has to be a state that survives
  scrubbing, filtering and sorting, or it is a dead end rather than a lens.
- **What happens to the process table while zoomed.** It is what the tool is
  for, and a zoom that hides it entirely is a mode you leave immediately. The
  prototype keeps it, shortened, showing the processes responsible for *that
  panel's* figure — which is more useful than the general table, not less.
- **Migrating `C`, `d`, `v`.** They stay as shortcuts to a zoomed panel, or they
  go. Either is defensible; silently keeping both mechanisms is not.

## Acceptance criteria

- [ ] One gesture zooms any panel, and the same gesture returns
- [ ] A zoomed panel shows its breakdown and the processes responsible for it
- [ ] Zoom survives scrubbing: the breakdown follows the time cursor
- [ ] The attribution states its unattributed remainder, as 0082 requires
- [ ] `C`, `d` and `v` are either expressed in terms of zoom or removed
- [ ] The tool is still usable with no knowledge that zoom exists
