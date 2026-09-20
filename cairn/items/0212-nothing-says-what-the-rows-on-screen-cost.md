---
id: 212
title: Nothing says what the rows on screen cost
type: feature
status: done
milestone: v4.0
created: 2026-09-15
updated: 2026-09-15
priority: p1
area: ui
depends_on:
- 203
---

## Problem

The header says what the machine is doing. The scope line says how many rows
there are. Nothing says what those rows *cost*.

"Four postgres processes using 142% of a core between them and two gigabytes" is
the question a filter just asked, and answering it meant reading four rows and
adding up in your head — or widening the terminal and squinting at a column.

It is also the thing Activity Monitor's bottom bar does *not* do: that bar is
machine-wide, and stays machine-wide however the list is filtered.

## What shipped

A strip between the panel title and the column headers, over the visible rows:

```
── processes (789) — sort: CPU ! io: 183/789 need root ────────────────────
 789 shown · CPU 255.5% · MEM 12.5G (52%)
 ▾CPU%            RSS      S   THR    DISK R    DISK W HIST ≤400%     PID
```

Three rows, each a step closer to the figures: the title says *which* processes
these are, the strip says what they add up to, the headers name the columns.

It follows everything that changes membership — the filter, the grouping, the
kernel-thread toggle, the user fold — because it is folded over
`App::visible_rows`, which is the same list the table draws and the same one the
scope line counts. The two cannot disagree about how many processes are being
described, and there is a test that walks four filters checking they do not.

## What it deliberately does not do

- **Thread rows are not processes.** `y` puts a row under a process for each of
  its threads, and counting them would make expanding one look like the machine
  had grown forty more — and double its CPU, since a thread row carries its
  process's figures.
- **A total nobody can supply is left out, not dashed.** On macOS a process
  poptop cannot open reports no thread count, so `— threads` would be a clause
  saying nothing on almost every frame.
- **Groups are named only when there are groups.** An ungrouped table has as
  many rows as processes and saying so is noise.
- **It gives up its row before the table does.** A summary of rows you cannot
  see is worth less than the rows.

## Acceptance criteria

- [x] What the visible rows cost is stated without adding up by hand
- [x] It changes with the filter, the grouping and every other membership toggle
- [x] It counts the same processes the scope line counts
- [x] A figure the platform cannot supply is absent rather than zero or dashed
- [x] It is the first row given up when the table is short
