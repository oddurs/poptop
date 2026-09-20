---
id: 119
title: Nothing says what to do when the box is slow
type: docs
status: done
milestone: r6
assignee: Oddur Sigurdsson
labels:
- docs
created: 2026-09-19
updated: 2026-09-19
priority: p1
effort: m
area: docs
---

## Problem

poptop is opened during an incident, and the README is an argument about design rather than a set of answers. "Why is this slow", "what happened four minutes ago", "which of these forty workers", "who is eating the disk", "what did it look like yesterday" — each is a sequence of two or three keys, and none of them is written down in that shape.

## Proposal

A task-oriented guide: the question, the keys, what you will see, and what it means. Short, with real output.

## Acceptance criteria

- [ ] `docs/guide/` covers: first run, finding what is slow, scrubbing to a moment, one process in depth, groups and trees, signals
- [ ] Every sequence has been run against a live poptop and its output pasted in

## How it was resolved

`docs/guide/` covers first run, finding what is slow, scrubbing to a moment,
one process in depth, groups and trees, and signals.

Every sequence was run against a live poptop under a pty with a terminal
emulator, at 100, 110 and 140 columns, and the frames pasted in: the panel
title naming the constraint, a filter taking 724 rows to 6, the memory and
disk views, the tree with an unreadable pid 1, `d` with and without a
selection, the `b` box and `nothing recorded at -2h — nearest sample is
1h59m away`.

Two real defects fell out of running them: 0124 and 0125.
