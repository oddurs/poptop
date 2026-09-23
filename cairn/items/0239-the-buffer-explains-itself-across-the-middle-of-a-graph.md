---
id: 239
title: The buffer explains itself across the middle of a graph
type: bug
status: backlog
milestone: r10
labels:
- ui
created: 2026-09-23
updated: 2026-09-23
priority: p2
---

## What happens

While the buffer is filling, the sentence `no history before 00:51:29 — it
fills from the right` is centred in the graph block, which puts it through the
middle of whichever row happens to be there — usually `MEM`. It reads as a
corrupted row of data.

## What should happen

The empty region is left empty, and the sentence is said once where it cannot
be mistaken for data: on the timeline's axis row, which is already about the
span being shown.
