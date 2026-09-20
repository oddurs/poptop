---
id: 218
title: One glyph means both "not collected" and "no change"
type: bug
status: backlog
milestone: r8
labels:
- ui
created: 2026-09-20
updated: 2026-09-20
priority: p2
---

## What happens

`·` is drawn by `io_cell` for a figure that was not collected, and by
`fmt_growth` for a memory delta of exactly zero. On the memory tab both can be
on the same row, a few columns apart.

## What should happen

This table is careful about exactly this: `—` is "not known", a blank is "not
applicable", a number is a measurement. `·` currently straddles "we did not
ask" and "we asked and the answer is nought", which are the two things the em
dash discipline exists to keep apart.

## Proposal

`0` already means "measured, and it is zero" in the disk columns. Growth of
zero is the same claim and can say the same thing; `·` then means only "not
collected", and means it everywhere.
