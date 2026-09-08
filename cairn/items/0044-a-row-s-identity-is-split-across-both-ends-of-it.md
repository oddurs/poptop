---
id: 44
title: A row's identity is split across both ends of it
type: feature
status: done
milestone: v1.0
created: 2026-09-07
updated: 2026-09-07
priority: p2
area: ui
---

## Problem

```text
PID     USER       CPU%         RSS           S  THR  HISTORY   DISK R    DISK W     COMMAND
81977   oddurs     103.4  ████+ 6.5G     █▏   R  33   0         0         ⠀⠀⠀⠀⠀⢀⣀⣄⣷⣀ com.apple.Virtual…
```

The three columns that say *which process this is* — `PID`, `USER`, `COMMAND` —
sit at opposite ends of the row with eight columns of measurement between them.
Reading a row means starting at the left, jumping seventy columns to the right
to find out what it is, and coming back.

The measurements are contiguous and well ordered. The identity is not: it is
split, and the half that actually identifies is the half at the far end.

## Why it is like this

htop puts `COMMAND` last, and this followed it. But htop's command column
carries the whole identity — its `PID` and `USER` are secondary — and it has no
sparkline column between the numbers and the name. Here `HISTORY` puts ten
columns of braille immediately before the name, so the two identity halves are
as far apart as the table can make them.

## Options worth weighing

1. **Move `COMMAND` left**, next to `PID`/`USER`, and let the measurements run
   to the right edge. Reads well; breaks the convention every other monitor
   follows, and a variable-width column in the middle makes the numeric columns
   ragged.
2. **Move `PID`/`USER` right**, so all identity is adjacent at the end. Keeps
   the convention, keeps numbers left where they are scanned, and puts the
   variable-width column at the edge where it belongs.
3. **Leave the order and close the gap** — move `HISTORY` away from the boundary
   so `COMMAND` at least abuts the numbers it belongs to.

Option 2 looks best on paper and is the biggest visual change; option 3 is
nearly free. This wants deciding with a rendered frame of each rather than in
the abstract.

## Acceptance criteria

- [x] The columns identifying a process are adjacent
- [x] Numeric columns stay contiguous and scannable
- [x] The choice is written down with the alternative that was rejected
