---
id: 219
title: A folded row puts a count in the PID column
type: bug
status: backlog
milestone: r8
labels:
- ui
created: 2026-09-20
updated: 2026-09-20
priority: p3
---

## What happens

Grouping by name draws `×2` under the `PID` header, beside rows that hold real
pids:

```
 PID
5317
1046
 789
  ×2
 400
```

## What should happen

A column holds one kind of thing. Scanning `PID` for a number and finding a
multiplier is the same class of surprise as a unit changing halfway down.

## Proposal

The count is the fact that *replaces* the pid, so the header can say so when
the table is folded — `PID` becoming `×` or `COUNT` for as long as grouping is
on. The rows then all hold the same kind of thing again, and the header is what
changed.
