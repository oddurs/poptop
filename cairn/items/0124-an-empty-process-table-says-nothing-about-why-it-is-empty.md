---
id: 124
title: An empty process table says nothing about why it is empty
type: bug
status: backlog
milestone: later
labels:
- ui
created: 2026-09-19
updated: 2026-09-19
priority: p2
---

## Problem

`g` cycled to containers on a machine with none draws a panel headed
`processes (0)`, a column header row, and thirteen blank lines. The same
happens for a filter that matches nothing, and for `C` where there are no
cgroups. The timeline learned to label its own empty region in 0109 — "no
history before 20:24:41 — it fills from the right" — and the table never did.

Captured while writing the r6 docs, at 100x26 on macOS:

```
── processes (0) — sort: CPU · grouped by container ! io: panel too narrow ──
   CPU%            RSS      S   THR HIST ≤100%     PID USER       COMMAND
```

...and nothing below it. A reader cannot tell whether poptop is broken, the
machine has no containers, or the filter is wrong.

## Proposal

One centred line in the empty region, naming the reason it is empty: no
containers on this machine, no cgroups, nothing matched the filter. The
reason is known at the point the rows are built — it is the grouping key or
the filter that emptied it.

## Acceptance criteria

- [ ] Each way the table can empty draws its own line
- [ ] The line does not move the columns or the panel title
- [ ] A test per reason
