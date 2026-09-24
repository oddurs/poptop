---
id: 176
title: The layout is three fixed stacks, not a dashboard
type: feature
status: backlog
milestone: r13
depends_on:
- 253
created: 2026-09-13
updated: 2026-09-23
priority: p0
area: ui
---

## Problem

poptop's layout is a vertical stack fixed at build time: a header, a timeline
carrying two series, a process table, a help line. It fits an eighty-column
terminal because it was designed for one, and it does nothing with a two-hundred
column one except make every row longer.

Meanwhile the tool now collects disks, network, stalls, cgroups, NUMA nodes and
NFS, and almost none of it has anywhere to live. The header absorbed them as
figures on a degradation ladder, which is why that ladder is now fourteen
entries deep and why a wide terminal shows the same two graphs a narrow one does.

## What it should be

A **panel grid** that reflows with the terminal, where each panel picks its own
level of detail from the space it is given. Not a configurable dashboard with a
layout language — a fixed set of panels, ranked, packed into whatever room there
is, with the process table always winning what is left.

### Level of detail is the whole idea

A panel has three forms, and the renderer picks by height, not by setting:

| rows | form | why |
| --- | --- | --- |
| 1 | name, figure, sparkline | at one row a line chart is a comb |
| 3–8 | line chart | the shape is the information |
| full screen | chart + breakdown + the processes responsible | see 0092 |

That ladder is not a nicety. A spiky series — disk, in particular — drawn as a
box-drawing line at four rows is an unreadable comb of `╭╮││╰╯`; the same series
as a one-row sparkline is perfectly legible. Prototyped both.

### 80×24 — strips, and the table gets everything

```text
 LIVE  CPU 61.2%  │  MEM 81.4%  SWP 12%  │  / 86% full  │  eth0 4.2M/s  │  UP 6d
CPU 61.2%  ▅▆▅▅▆▆▄▅▆▅▄▇▇▁▁▁▁▁▁   MEM 81.4% ▅▅▅▆▆▆▅▆▆▆▆▆▆▆▆▆▆▆▆▆▆▆▆▆▆▆▆▆▆
DISK 41%   ▆▅ ▆ ▅▆ ▆ ▇▆ ▆ ▇ ▆▅   NET 4.2M/s ▃▄▄▃▄▄▂▄▄▃▄▄▃▃▄▄▃▄▄▂▄▄▃▄▄▃▄▄
PROCESSES
   PID USER        CPU%     MEM  COMMAND
  4823 postgres    34.2    3.1G  postgres
  9912 build       22.8    412M  cc1plus
```

Two rows buy four metrics. The old layout spent nine on two.

### 130×34 — two charts, three strips

```text
▸ CPU 61.2%                            MEM 81.4%
                       ╭────╮
        ╭────╮  ╭────╮ │    │          ───────────────────╮   ╭────
        │    ╯──╰    ╯─╰    │                             ╰───╯
────────╰                   ╯────
DISK vda 41% ▇▅ ▆ ▆▆ ▆▅  NET eth0 ▃▄▃▄▄▄▃▄  STALL io 12.4% ▂▂ ▂▂ ▂▂
PROCESSES
```

### 190×44 — three charts, four strips

Same ladder, one more column and one more strip. Nothing is re-laid out by hand;
the grid is a function of width.

## What needs deciding

- **How panels are ranked.** The process table is not a panel — it is what the
  tool is for, and it takes whatever the panels leave. Everything else is a
  ladder like the header figures already have, and the same rule applies: rank
  is drop order, and it is separate from position.
- **Whether the layout may move.** It may not. #93 found that deriving the header
  height from the sample made the whole screen jump a row when scrubbing across a
  boundary, and the same rule holds here: a panel appearing because a machine
  gained an NFS mount is a layout that moves under the reader. Panels are decided
  by the machine's capabilities, once, not by whether a figure is currently
  interesting.
- **The header's future.** Fourteen figures on a ladder exists because there was
  nowhere else to put them. With panels, most of them have a home, and the bar
  should shrink to the things that qualify everything else — the live/paused
  marker, CPU, memory, and the clock.
- **Breakpoints.** 100 and 160 columns in the prototype. They want checking
  against real terminals rather than being chosen because they are round.

## Acceptance criteria

- [ ] Panels are laid out by width, with no per-width code
- [ ] A panel picks its form from its height: strip, chart, or zoomed
- [ ] The process table keeps every row the panels do not need
- [ ] The layout does not move when a figure appears or disappears
- [ ] Every panel is reachable at eighty columns, even if only as a strip
- [ ] The degradation ladder is stated, as the header's is
