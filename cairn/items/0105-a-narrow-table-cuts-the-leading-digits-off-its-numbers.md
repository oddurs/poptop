---
id: 105
title: A narrow table cuts the leading digits off its numbers
type: bug
status: backlog
milestone: r5
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
