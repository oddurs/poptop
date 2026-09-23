---
id: 238
title: The column header speaks three vocabularies
type: feature
status: done
milestone: r10
labels:
- ui
created: 2026-09-23
updated: 2026-09-23
priority: p1
---

## Problem

```
▾CPU%            RSS      S   THR    DISK R    DISK W HIST ≤800%     PID USER       COMMAND
```

Four idioms in one row: a sort caret glued to a name, `top`'s uppercase
abbreviations, a one-letter column whose name is a letter, and a *scale* —
`≤800%` — living inside a header. The scale changes width when the machine's
busiest process crosses a power of two, so the header shifts sideways while
the reader is looking at it.

## Proposal

One vocabulary. Short names in one case, the sort caret in the same place for
every column rather than glued to the name, and the sparkline's ceiling out of
the header and into the gutter the graphs already use for scales.

## Acceptance criteria

- [x] The header is one vocabulary, and the widths do not change with the data
- [x] The sparkline's scale is still stated, somewhere it does not move
- [x] The sorted column is still obvious at a glance
