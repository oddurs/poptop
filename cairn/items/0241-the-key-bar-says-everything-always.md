---
id: 241
title: The key bar says everything, always
type: feature
status: done
milestone: r10
labels:
- ui
created: 2026-09-23
updated: 2026-09-23
priority: p2
---

## Problem

```
q quit · F10 menu · ←/→ scrub · b jump · +/- zoom · Space live · ↑/↓ select · s sort · / filter · x signal · ? more
```

Eleven pairs, every frame, whatever the reader is doing. It is the second
busiest line on the screen and it is the same line whether they are scrubbing
through history, choosing a process, or looking at a filtered table.

## Proposal

The keys that act on what is in front of you, and a way to the rest. Scrubbing
keys while scrubbing; the selection's keys once a row is selected; `?` always.
The ladder already drops keys by width — this drops them by relevance first.

## Acceptance criteria

- [x] The bar names fewer keys, and the ones it names are the ones that act now
- [x] Nothing becomes unreachable: `?` and the menu still name everything
- [x] The bar does not change width as the state changes, so the screen does not jitter
