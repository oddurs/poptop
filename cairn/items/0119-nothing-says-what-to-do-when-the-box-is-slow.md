---
id: 119
title: Nothing says what to do when the box is slow
type: docs
status: backlog
milestone: r6
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
