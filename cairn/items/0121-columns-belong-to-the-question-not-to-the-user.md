---
id: 121
title: Columns belong to the question, not to the user
type: feature
status: done
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

## What was decided

**`i` was a collection question wearing a display key, and that is where it
went.** Choosing the Disk tab now insists on the disk figures, so the tab pays
for what it shows. The key is gone; nothing replaced it.

**The disk columns stayed on the CPU tab.** The plan above proposed moving them
to Disk alone, and `io_columns_are_there_before_anyone_asks` already argued the
other way and argued it better: the header can say the machine is blocked on IO,
and the table under it is where the culprit is named. Making that answer a tab
away would be a real loss to save a key, so only the key went.

**The membership keys survive.** `y`, `K` and `C` change which *rows* exist
rather than which columns describe them, which is a question no tab can answer.
The table in this ticket is therefore a plan for 120 and 126 rather than a thing
that landed whole.

## Acceptance criteria

- [x] No key toggles a column that a tab already answers for
- [x] Each tab has a sane default sort, and switching tabs keeps a valid one
- [x] The width ladder still decides what fits
- [x] Nothing that was reachable becomes unreachable
