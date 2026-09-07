---
id: 41
title: The process table can be squeezed to zero processes
type: bug
status: backlog
milestone: v1.0
created: 2026-09-07
updated: 2026-09-07
priority: p1
area: ui
---

## What happens

At 120x14 with a filter matching eleven processes:

```text
── processes (11) — sort: CPU · io: 259/783 need root · history ≤400% ───────
PID     USER       CPU%         RSS           S  THR  HISTORY   DISK R   COMMAND
q quit · ←/→ scrub · +/- zoom · Space live · ↑/↓ select · s sort · t tree
```

Eleven matches and **not one of them is drawn**. The user filtered to find
something and the panel that would show it has been squeezed to nothing by a
graph they were not looking at.

In tree mode at 120x16 the table gets two rows, and because tree order is
structural rather than by CPU they are `diskarbitrationd` and
`LegacyProfilesSubscriber` — both at 0.0%, both unreadable, both meaningless.

## Why

`PROCS_FLOOR_H = 2` promises the table "always keeps at least this much,
however cramped the terminal". It is measured in *panel rows*, and a table panel
spends two of them before any data: one on the section divider, one on the
column header. A floor of two panel rows is a floor of zero processes.

At 14 rows total: header 2, footer 1, timeline clamps to its own floor of 9, and
the table gets 2 — both of them chrome.

## The rule this breaks

**A panel's floor has to be measured in the units of the thing it shows.** The
timeline's nine rows are two series of three plus an axis — real units. The
table's floor should be a number of processes, and the panel it needs is that
plus its own chrome.

There is a second question underneath, worth deciding rather than inheriting:
whether a nine-row graph is the right trade on a fourteen-row terminal at all.
`TIMELINE_MIN_H` was set so that growing a window never shrinks the graph, which
is an argument about resolution — and it is being applied at sizes where the
answer is not "less resolution" but "no processes".

## Acceptance criteria

- [ ] The floor is a number of *processes*, and the panel reserves its chrome
      on top of that
- [ ] A filter matching N processes shows some of them at every terminal size
      the tool runs at
- [ ] Decided and written down: whether the timeline yields below some height,
      and to what
- [ ] A test sweeps terminal heights and asserts a process row is drawn at each
