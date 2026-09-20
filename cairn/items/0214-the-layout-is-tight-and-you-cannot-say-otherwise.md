---
id: 214
title: The layout is tight and you cannot say otherwise
type: feature
status: done
milestone: v4.0
created: 2026-09-16
updated: 2026-09-16
priority: p2
area: ui
---

## Problem

Everything sits against everything else. The figures in the header are packed
two columns apart, the table's rows are flush with column zero and the right
edge at once, and the graph runs straight into the process panel with nothing
between them.

Air had been added twice by then — a third column between header figures, one
either side of the table — and both were decisions taken for the reader rather
than by them. Somebody working on an eighty-column terminal wants neither and
somebody on a wide one wants more.

## What shipped

`--density=compact|comfortable|spacious`, and the same three in the View menu
with the one in force ticked.

| | compact | comfortable | spacious |
|---|---|---|---|
| the frame's content margin | 0 | 1 | 2 |
| gap between two figures in a group | 2 | 3 | 4 |
| blank row between the graph and the table | — | — | 1 |

Every value is a *maximum*. A narrow terminal gives up the indent before it
gives up a column of the command line; a short one gives up the blank row before
a row of the table. Comfort is the first thing surrendered, because a process
elided to `…derer)` is a worse loss than a row touching the edge.

The panel dividers stay full width whatever the setting: they are what says
where a panel begins, and one stopping short reads as a box missing its corners.

## The refinement that mattered

The first version indented only the table, which did not fix the feeling. What
did was measuring where content actually began on each row:

    menu 0 · tabs 3 · header 1 · summary 2 · headers 2 · footer 0

Five margins, not one. The layout was not short of air so much as ragged, and
the eye cannot run down a left edge that is in five places. `Density::margin` is
now the single source, applied by one `content()` helper to every row that is
not a divider — and there is a test that renders at all three densities and
asserts the whole frame starts in one place.

## What was declined

**A wider gap between the table's own columns.** It was built and reverted. A
full table has thirteen gaps, so a second column of air is thirteen off the
command line — and the width has to be known by `command_width` and by the click
hit-testing as well as by the table, a third place for one fact. That is the bug
this interface has had four times already. The columns are told apart by their
alignment; the air went into the indent and the header, where it costs one
column and none.

Reverting it surfaced a real bug on the way: ratatui's `Table` was still using
its default spacing of one while `sort_at` split with two, so a click on a
header landed a column short of the column it was over. The spacing is set
explicitly now, from the same function both read.

## Acceptance criteria

- [x] Each density is visibly roomier than the one below it
- [x] Comfort is given up before content on a small terminal
- [x] Reachable from the menu, the config file and a flag
- [x] The menu says which one is in force
- [x] The panel dividers are unaffected
