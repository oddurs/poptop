---
id: 119
title: A filtered list can be mistaken for the machine
type: bug
status: done
milestone: v4.0
created: 2026-09-15
updated: 2026-09-15
priority: p1
area: ui
---

## Problem

Activity Monitor writes **All Processes** under its title, permanently, in the
place a document would put its subtitle. It is there when nothing is filtered,
which is what makes it trustworthy when something is: the line never disappears,
so its absence can never be mistaken for "no filter".

poptop states its scope in a clause of the process panel's title — `processes
(760) · all root — sort: CPU` — and that title is a degradation ladder whose
clauses are dropped from the least important end when the panel narrows. So on
the terminals where the table is hardest to read, the sentence saying *which
processes these are* is the first thing to go.

A filtered table that does not say it is filtered is a table that lies about the
machine, and it does it silently.

## The fix

A scope line that is structural rather than a clause: always present, always in
the same place, stating what is being listed and what has narrowed it.

```
  All processes · 760
  postgres · 4 of 760 · user oddurs
```

## What was decided

**It shares the tab strip's row, right-aligned.** "Which resource" and "which
processes" are the same question — what am I looking at — so they belong on the
same band, and the scope costs no row of its own. The strip lost its `tab · 1-3`
hint to make room, which is no loss: the strip shows the tabs and the menu names
their keys, so the footer hint was the third place the same thing was explained.

**The filter and the user are scope; grouping and the tree are not.** Both of
those change the shape of the list rather than its membership, and the panel
title still names them.

## Acceptance criteria

- [x] Present when nothing is filtered, so its absence means nothing
- [x] Never dropped by the width ladder
- [x] States the count shown against the count that exists
- [x] A filtered table cannot be read as the whole machine
