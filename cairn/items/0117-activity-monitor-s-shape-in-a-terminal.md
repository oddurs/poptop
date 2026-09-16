---
id: 117
title: Activity Monitor's shape, in a terminal
key: v4.0
type: milestone
status: done
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

- [x] The resource being examined is visible without pressing anything
- [x] Choosing one changes columns and sort together, and asks for the data it
      needs — the graph and the summary deliberately do not follow it, see 120
- [x] The summary is the same shape at every width, which is the half of this
      that was worth having
- [x] The scope is stated permanently, not in a clause that can be dropped
- [x] The timeline is not diminished to make room for any of it

## What the milestone turned out to be

Four of the eleven were built as written. The other seven each turned up
something the plan had not:

- **121** proposed moving the disk columns off the CPU tab. An existing test
  argued the other way and argued it better — the header can say the machine is
  blocked on IO, and the table under it is where the culprit is named. Only the
  key went.
- **120** proposed a three-zone strip below the table. The zone discipline was
  already there and untested; the strip was declined, because in a terminal rows
  are the scarcest resource and a second summary costs three of them to repeat
  what the header says in one.
- **124** proposed a second Escape to clear the filter. Escape now does what
  every other program uses it for — undo — and clearing is a named menu item.
- **126** proposed an Energy tab. It cannot be filled on either platform and
  there is none; what shipped is the switch and interrupt rate poptop was
  already collecting and never showing.
- **127** was half-done before it was picked up, by the striping and the raised
  header band from the surfaces work. What was actually broken was alignment,
  which was a convention checked nowhere.

The recurring lesson is one the earlier milestones already had and this one
found three more instances of: **a fact derived twice will disagree.** `panels`,
`shown_window`, `dropdown_rect`, `table_shape`, `table_columns` and `Blocked`
are all the same fix — one derivation, used by the drawing and by whatever else
needs to agree with it.
