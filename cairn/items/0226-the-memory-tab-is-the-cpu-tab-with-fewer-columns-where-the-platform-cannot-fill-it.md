---
id: 226
title: The memory tab is the CPU tab with fewer columns where the platform cannot fill it
type: feature
status: backlog
milestone: r8
labels:
- ui
created: 2026-09-20
updated: 2026-09-20
priority: p2
---

## Problem

On macOS `mem_columns_available` reports no `PSS`, no `VSZ` and no `MAJF/s`, so
the memory tab drops `THR` and adds `GROW` — which prints `·` whenever memory
is steady, which is most of the time. The result is the CPU tab minus a column:

```
 ▾CPU%            RSS      S      GROW     PID  COMMAND
  142.9 ████+     3.0G ▊    S         ·    5317 OrbStack Helper …
```

A reader who presses `2` to ask about memory is shown less than they had.

## Proposal

A tab that cannot be filled on this platform should say so, the way the disk
columns do — the mechanism exists and is good. Either the tab states what this
kernel will not tell it, or on a platform with only `RSS` and growth to offer
it earns its place a different way: the memory the *machine* is spending, by
process, against the total — which is a question `RSS` alone answers badly and
which poptop has the samples for.

Worth deciding rather than leaving: an empty tab is a worse answer than no tab.
