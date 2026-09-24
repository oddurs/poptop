---
id: 222
title: The history column draws the same picture in every row
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

0110 again, by a different route. Every one of the seven rows on the capture
draws a byte-identical sparkline:

```
HIST ≤400%
⠀⠀⠀⠀⢀⣀⣀⣀⣀⣀
⠀⠀⠀⠀⢀⣀⣀⣀⣀⣀
⠀⠀⠀⠀⢀⣀⣀⣀⣀⣀
```

The guard added for 0110 asks whether *any* process in the buffer was ever
drawn at two different heights. One process that spikes answers yes, and the
column is then kept for every other row — where, against a ceiling of 400%, a
process at 13% and one at 3% are both drawn on the floor.

## Proposal

The question the guard asks is about the buffer; the question that matters is
about the rows on screen at the scale they are drawn at. Ask it of those: if no
*visible* row is drawn at two heights, the column is ten columns of one picture
and the command should have them.

The shared ceiling is what makes this bite, and it is right — scaling each row
to its own peak is what 0110 rejected. So the fix is in the guard, not the
scale.

## Acceptance criteria

- [ ] The column folds when every visible row draws the same shape
- [ ] It survives a machine where one row moves and the rest are flat, if that
      row is on screen
- [ ] Scrolling a busy process into view brings it back, and the rule says why
      it went
