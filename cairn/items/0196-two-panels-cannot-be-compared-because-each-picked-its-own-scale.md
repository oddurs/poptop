---
id: 196
title: Two panels cannot be compared because each picked its own scale
type: feature
status: backlog
milestone: v3.5
created: 2026-09-14
updated: 2026-09-14
priority: p1
area: ui
depends_on:
- 190
---

## Problem

Every graph scales to its own peak. Stacked one above another they look like
they can be read against each other, and they cannot: a CPU row at half height
and a disk row at half height have nothing in common.

The whole reason to stack graphs is comparison, and the default scaling quietly
denies it.

## The fix

Make the scale a stated model rather than an implicit one:

- `auto` — per panel, fitted where fitting buys resolution (106). The default.
- `zero` — every panel from zero, always. The honest-by-default option.
- `shared` — one scale across every panel that carries the same unit. Percentages
  share; bytes a second share; a percentage and a byte rate never do.
- `fixed=N` — pinned, for watching one number against a known limit.

And whichever is in force is said once, in the panel title, not inferred from
the gutter.

## What needs deciding

- **What "same unit" means.** CPU% and memory% are both percentages and are not
  the same quantity. Sharing across them is defensible and may be wrong.
- **Whether `shared` is per-frame or per-buffer.** Per-frame flaps; per-buffer
  wastes the panel after a spike ages out.

## Acceptance criteria

- [ ] Two stacked panels can be put on one scale deliberately
- [ ] Which scale is in force is stated, not inferred
- [ ] A percentage and a byte rate are never silently shared
- [ ] `zero` is reachable in one flag, for anyone who wants no cleverness
