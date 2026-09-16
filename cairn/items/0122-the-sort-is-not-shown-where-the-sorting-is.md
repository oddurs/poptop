---
id: 122
title: The sort is not shown where the sorting is
type: feature
status: backlog
milestone: v4.0
created: 2026-09-15
updated: 2026-09-15
priority: p2
area: ui
depends_on:
- 115
---

## Problem

Activity Monitor puts a caret in the header of the column it is sorted by, and
clicking a header sorts by it. The state is shown exactly where the action
happens, which is why nobody has ever needed to be told how it works.

poptop states its sort in the panel title — `sort: CPU` — several rows away from
the column it is talking about, in a clause the width ladder can drop. And the
only way to change it is `s`, which cycles.

## The fix

The marker on the column:

```
  CPU% ⌄          RSS      S   THR HIST ≤100%     PID COMMAND
```

Clicking a header sorts by it; clicking the sorted one reverses. `s` keeps
cycling, because a monitor over ssh on a terminal with no mouse is the case
poptop is for.

## What needs deciding

- **Descending is not a thing poptop has.** Every sort is descending, because
  "what is using the most" is the question. Reversal has to either be added or
  deliberately declined, and declining it means the caret is decoration.
- **Where the marker goes on a right-aligned numeric column.** Against the
  header text, not the column edge, or it reads as part of the number below it.

## Acceptance criteria

- [ ] The sorted column is marked in its own header
- [ ] Clicking a header sorts by it
- [ ] `s` still works, and agrees with what the header shows
- [ ] The marker survives the width ladder, or the column does not
