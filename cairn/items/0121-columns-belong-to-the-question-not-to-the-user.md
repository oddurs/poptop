---
id: 121
title: Columns belong to the question, not to the user
type: feature
status: backlog
milestone: v4.0
created: 2026-09-15
updated: 2026-09-15
priority: p2
area: ui
depends_on:
- 118
---

## Problem

poptop has `i` for IO columns, `y` for the thread count, `C` for cgroups, `K`
for kernel threads, and `v` for the column set — five keys spent on deciding
what a table shows, each of which can be in either state, and no two of which
are related.

Activity Monitor has none. The tab knows what its columns are. Choosing Memory
*is* choosing the memory columns, and there is nothing else to press.

That is not a simplification, it is a better model: a column set is the answer
to "what am I asking", and the tab is where that question is already asked.

## The fix

Each tab owns its columns, its default sort and its summary zones. The toggles
go, except where a column is genuinely optional for a reason the tab cannot know
— kernel threads are a *membership* question, not a column question, and belong
with the scope (119).

| tab | columns |
|---|---|
| CPU | name, %cpu, cpu time, threads, state, pid, user |
| Memory | name, rss, share, growth, swap, pid, user |
| Energy | name, cpu time, wakeups, since start, pid, user |
| Disk | name, read/s, written/s, read total, written total, pid, user |
| Network | name, in/s, out/s, in total, out total, pid, user |

The width ladder stays: it decides which of a tab's columns fit, which is a
different question from which columns the tab has.

## What needs deciding

- **Whether any toggle survives.** `i` exists because IO is expensive to
  collect, not because it is optional to see — that is a collection question
  wearing a display key, and it may belong in the Disk tab's own behaviour.
- **What happens to a sort by a column the new tab lacks.** Already handled for
  `View`; the rule generalises.

## Acceptance criteria

- [ ] No key toggles a column that a tab already answers for
- [ ] Each tab has a sane default sort, and switching tabs keeps a valid one
- [ ] The width ladder still decides what fits
- [ ] Nothing that was reachable becomes unreachable
