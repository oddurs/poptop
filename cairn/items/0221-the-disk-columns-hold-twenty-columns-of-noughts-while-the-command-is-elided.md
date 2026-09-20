---
id: 221
title: The disk columns hold twenty columns of noughts while the command is elided
type: feature
status: backlog
milestone: r8
labels:
- ui
created: 2026-09-20
updated: 2026-09-20
priority: p1
---

## Problem

On the capture this sprint came from, `DISK R` and `DISK W` hold about twenty
columns and read `0` on every visible row, while `COMMAND` — the only column
that answers *which process this is* — gets thirty-two and loses its middle:

```
 DISK R    DISK W  ...  COMMAND
      0         0       claude --dangero…skip-permissions
      0         0       claude --resume …97b-a1ebc3ceb02d
```

The machine was doing four kilobytes a second of disk. The columns are correct
and they are describing nothing.

## Proposal

The table already folds a column whose every value is the same (`USER`, when
one user owns everything) and drops one whose data does not move (`HIST`, when
no row's history changes). A column whose every value is nought on every row on
screen is the same fact, and the same trade: give the width to the column that
is starving.

Not a permanent decision — the moment a process reads or writes, the columns
are the point. The `CONSTANT_FOR` threshold the user column folds on is the
same shape of answer.

## Acceptance criteria

- [ ] Disk columns fold while every visible row reads nought, and come back on
      the first row that does not
- [ ] The panel rule says they folded, as it does for the others
- [ ] `COMMAND` gets the width back
