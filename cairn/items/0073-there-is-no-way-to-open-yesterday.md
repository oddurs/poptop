---
id: 73
title: There is no way to open yesterday
type: feature
status: done
milestone: v2.2
depends_on:
- 72
created: 2026-09-08
updated: 2026-09-09
priority: p0
area: ui
---

## Problem

atop reads a raw file with `-r`, jumps to a time with `-b`, steps sample by
sample with `t` and `T`, rewinds with `r` and fast-forwards with `Z`. It is a
complete second mode: the same interface over recorded time instead of live
time.

poptop restores a buffer at startup and then behaves as though it were live.
There is no way to say "open the file from last Tuesday", no way to jump to a
timestamp, and the scrub keys move one sample at a time through whatever is in
memory.

## Why it is more than a file-open

poptop's scrubbing is already the good half of this: the cursor, the process
table that follows it, the detail view, the constraint at the cursor. All of it
works on a buffer and none of it cares where the buffer came from. What is
missing is addressing — *which* history, and *when* in it.

`-b`, jumping to a timestamp, is the one that changes how the tool is used. An
incident has a time; scrubbing back to it by pressing the left arrow six hundred
times is not a workflow.

## What needs deciding

- **Whether live and replay are the same binary state.** atop's twin mode (`-t`)
  spawns a collector so you can review history while still recording. poptop's
  buffer already fills while paused, which is the same idea and simpler.
- **How a timestamp is entered.** A prompt, like the filter. It should accept
  both absolute and relative — "03:00" and "-2h" — because both are how the
  question is asked.
- **What happens at a gap.** poptop already draws seams for unobserved
  intervals. Jumping into one has to land somewhere honest.

## Acceptance criteria

- [x] A recorded day can be opened by date and scrubbed like the live buffer
- [x] Jump to a timestamp, absolute or relative
- [x] Live recording continues while reviewing history
- [x] Landing in a gap says so rather than showing the nearest sample as if it
      were the one asked for
