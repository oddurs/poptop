---
id: 122
title: The sort is not shown where the sorting is
type: feature
status: done
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

## What was decided

**Reversal was declined and the caret is not decoration.** Every sort is
descending, because "what is using the most" is the question. The caret says
*which* column the ordering is over, which is the thing that was previously
only stated several rows away in a clause the width ladder could drop.

**The caret goes in the padding a right-aligned column already has**, prefixed
rather than appended. Appending it pushes the label two columns left and the
header stops sharing a right edge with the figures under it, which
`a_column_of_figures_shares_a_right_edge` exists to prevent.

**The bar beside a figure does not take a second caret**, and the disk pair
takes one on the first of them: the ordering is read *plus* write, so a caret on
each would claim two keys and a caret on the second would read as a claim about
that column alone.

## Acceptance criteria

- [x] The sorted column is marked in its own header
- [x] Clicking a header sorts by it
- [x] `s` still works, and agrees with what the header shows
- [x] The marker survives the width ladder, or the column does not
