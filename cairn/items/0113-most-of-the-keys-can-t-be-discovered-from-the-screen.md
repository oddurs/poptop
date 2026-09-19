---
id: 113
title: Most of the keys can't be discovered from the screen
type: feature
status: backlog
milestone: r5
labels:
- ui
created: 2026-09-19
updated: 2026-09-19
priority: p2
effort: s
area: ui
---

## Problem

The footer lists 18 key hints, and only the first 12 fit at 120 columns: `q ←/→ b +/- Space ↑/↓ s / x t i v`. `d` (a process's own history), `g` (group), `y` (threads), `C`, `K` and `S` never appear on a normal terminal. There's no in-app help screen, so the only way to learn about the best features is `--help`.

## Proposal

A `?` overlay listing every key and what it does, generated from the same table as the footer and `--help`, so the three can't disagree. The footer ends with `? more` whenever it had to drop hints.

## Acceptance criteria

- [ ] `?` shows every key; Esc or `?` closes it
- [ ] The footer says `? more` when it drops hints
- [ ] A test checks that the overlay, the footer and `--help` list the same keys
