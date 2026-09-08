---
id: 45
title: The process title mixes four kinds of statement
type: bug
status: done
milestone: v1.0
created: 2026-09-07
updated: 2026-09-07
priority: p2
area: ui
---

## Problem

```text
── processes (783) — sort: CPU · tree · io: 259/783 need root · history ≤400% ──
```

Five statements, four kinds, all wearing the same clothes:

| text | kind |
|---|---|
| `processes (783)` | a fact about the data |
| `sort: CPU`, `tree` | settings the reader chose |
| `io: 259/783 need root` | a warning, and about one column |
| `history ≤400%` | a scale, and about a different column |

A reader cannot tell which of these they can change, which is telling them
something is wrong, and which is a legend for a column three metres to the
right. They are separated by identical `·` marks, so the line reads as one
undifferentiated string of facts.

Compare the timeline's title, which says one thing:
`── timeline — 2s of 9m59s buffered ──`

## Why it grew this way

Each addition was reasonable on its own — the `i` key needed to explain itself,
the sparkline axis needed stating, tree mode needed announcing. Nothing decided
that a section title is a place for *one* kind of statement, so it became the
place for any statement that had nowhere else.

## What should happen

Decide what a section title is for, and put the rest where it belongs.

The candidate split:

- **Title**: what this panel shows and how much of it — `processes (783)`, plus
  the mode when it is not the default, since a mode changes what the panel *is*.
- **Column-scoped facts**: `io: … need root` and `history ≤400%` are legends for
  columns. A legend belongs with the thing it explains — under the column
  header, or in the column header itself.
- **Warnings**: `259/783 need root` is not a legend, it is a reason a column is
  empty. That reads as a warning and should look like one.

The header solved the same problem by grouping and by giving the state marker
its own treatment; the table has not had that pass.

## Acceptance criteria

- [x] A section title carries one kind of statement
- [x] Column-scoped facts sit with their column
- [x] A warning is distinguishable from a legend without reading it
- [x] The title still degrades on a narrow panel and still names the count
