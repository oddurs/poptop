---
id: 200
title: A command palette for the things with no key
type: feature
status: backlog
milestone: v3.6
created: 2026-09-15
updated: 2026-09-15
priority: p2
area: ui
depends_on:
- 198
---

## Problem

The menu is browsable and the keys are fast, and neither is searchable. "How do
I see disk columns" is answered by reading five dropdowns.

There are also commands that deserve no key and no menu slot — every glyph set,
every sort column, every threshold — and a palette is where those live without
crowding anything.

## The fix

`Ctrl-P` or `:`, a single line, fuzzy match over `Action`'s labels. `command.rs`
already holds the list; it needs a name per action, which the menu items already
supply.

## What needs deciding

- **Whether it shows the key.** It should: a palette that teaches the key it
  replaces makes itself unnecessary, which is the right ambition for it.
- **Ranking.** Recently used first is the usual answer and needs state that
  survives a restart, which poptop has nowhere to put yet except the config
  directory.

## Acceptance criteria

- [ ] Any command can be found by typing part of its name
- [ ] The key that also runs it is shown beside it
- [ ] Escape leaves everything as it was
