---
id: 117
title: Activity Monitor's shape, in a terminal
key: v4.0
type: milestone
status: backlog
created: 2026-09-15
updated: 2026-09-15
priority: p1
area: ui
---

## The thesis

Activity Monitor is the best-designed process monitor there is, and almost none
of what makes it good is graphical. Strip the rounded corners and the icons and
what is left is an information architecture that a terminal can carry whole:

```
┌ Activity Monitor ──────── ⊗ ⓘ ⋯ ─ [CPU] Memory Energy Disk Network ── 🔍 ┐
│ All Processes                                                           │
├─────────────────────────────────────────────────────────────────────────┤
│ Process Name    % CPU ⌄   CPU Time  Threads  Idle Wake Ups  Kind   PID   │
│ Xprotect…        57.6     4:32.42         5              2  Apple  22450 │
│ kernel_ta…       52.2  16:36:00.60      702           8938  Apple      0 │
├─────────────────────────────────────────────────────────────────────────┤
│ System:  22.40%  │      CPU LOAD      │  Threads:    8,158              │
│ User:    66.07%  │   ▁▂▃▅▂▁▂▇█▅▃▂▁▃▅  │  Processes:    792              │
│ Idle:    11.54%  │                    │                                 │
└─────────────────────────────────────────────────────────────────────────┘
```

Five ideas, none of them about pixels:

1. **One tab per resource, and the tab changes everything.** Picking Memory does
   not add a column — it changes the columns, the sort, the graph and the
   summary together. The question you are asking determines the whole screen.
2. **A summary strip with the same shape on every tab.** Three zones, always in
   the same order and the same places: named scalars, a graph, counts. You learn
   it once.
3. **The scope is always stated.** "All Processes" sits under the title
   permanently, so a filtered list can never be mistaken for the whole machine.
4. **Columns belong to the question, not to the user.** There is no column
   picker. The tab knows what its columns are.
5. **Summary below the detail.** Your eye lives in the table; the aggregate is a
   glance down, not a thing you read past on the way in.

## What poptop has instead

`View` is a three-way cycle on `v` that changes columns and nothing else — no
graph follows it, no summary, and the cycle is invisible until you press the
key. The header is an ad-hoc row of figures whose shape changes with the
terminal width. The scope is a clause in a panel title that the degradation
ladder drops first.

The pieces exist. They are not composed.

## What must not be carried over

Activity Monitor has no history. It shows the last sixty seconds of one graph
and nothing else, and every process figure is the instant you happen to be
looking at. poptop's whole point is that you can go back, and the timeline is
not a widget in the summary strip — it stays a panel of its own, above the
table, at full width.

Anything in this milestone that would shrink the timeline into a corner of a
summary bar has misread the brief.

## Acceptance criteria

- [ ] The resource being examined is visible without pressing anything
- [ ] Choosing one changes columns, sort, graph and summary together
- [ ] The summary strip is the same shape on every tab
- [ ] The scope is stated permanently, not in a clause that can be dropped
- [ ] The timeline is not diminished to make room for any of it
