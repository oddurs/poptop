---
id: 105
title: A narrow table cuts the leading digits off its numbers
type: bug
status: done
milestone: r5
assignee: Oddur Sigurdsson
labels:
- ui
created: 2026-09-19
updated: 2026-09-19
priority: p0
effort: s
area: ui
---

## What happens

Below about 66 columns, right-aligned cells in the process table are cut from the left. The number that remains is a different number:

- At 64 columns, 100.9% CPU shows as `00.9` and an RSS of 1.2M as `.2M`.
- At 60 columns, 17.2% shows as `7.2`, 14.6M as `6M`, and the headers read `PU%`, `SS` and `HIST ≤2` (for `≤25%`).

The rest of poptop shows `—` rather than a wrong figure. This is the one place it does the opposite, and only on the narrow terminals where people glance at it.

## What should happen

A number that doesn't fit is never truncated. Either the column is dropped, following the degradation ladder that already exists, or the number is replaced with a marker saying it didn't fit. Headers follow the same rule.

## Reproduction

1. Run poptop in a 64×24 terminal.
2. Compare the CPU column with `ps -o %cpu` for the top process.

## How it was resolved

Every column was a fixed `Length`. Below the width they add up to, ratatui squeezes fixed lengths rather than dropping any, and a right-aligned number that gets squeezed loses its leading digits. The table now has a single description of its columns, `ui::Columns`, used both to decide what fits and to lay the table out, so the two can't disagree. When the columns don't fit, `Columns::fit` drops whole columns, least useful first, until the command has its minimum:

1. the history sparkline
2. the bars (they repeat the numbers beside them)
3. the container column
4. USER
5. the thread count
6. a view's own columns
7. the state
8. RSS
9. the pid

CPU% and the command are always kept.

Live on this Mac: at 64 columns the history column goes, at 60 the bars, at 44 USER, and at 30 the table is CPU%, PID and the name. Every number is whole at every width. The command's width is now measured from the same description, not estimated separately.

Tests: `a_narrow_table_drops_columns_rather_than_digits` renders every width from 20 to 160 and checks that the header names only real columns and that each CPU and RSS figure appears in full. On the old code it fails at the first width tried. `the_columns_that_are_drawn_always_fit` checks the ladder's own arithmetic from 17 to 200 columns, and that nothing is dropped on a wide terminal.
