---
id: 113
title: Most of the keys can't be discovered from the screen
type: feature
status: done
milestone: r5
assignee: Oddur Sigurdsson
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

- [x] `?` shows every key; Esc or `?` closes it
- [x] The footer says `? more` when it drops hints
- [x] A test checks that the overlay, the footer and `--help` list the same keys

## How it was resolved

- **One list of keys.** `ui::HELP` holds every key with how it's shown, how `--help` spells it, and what it does. `every_key_is_in_the_help_and_the_help_is_in_usage` requires every footer hint to be in it, and every entry in it to be in `--help`'s KEYS section. That test found real drift at once: `--help` had never mentioned `v`, `y` or `C`, all three of which the footer advertised. They're documented now, along with `?`, and `?` is in the README's key table.
- **`?` opens the list.** It's a bordered box over the middle of the screen, and any key closes it without also being acted on, so `q` closes the list rather than quitting behind it. Ctrl-C still quits from anywhere. `question_mark_shows_every_key_and_any_key_puts_it_away` checks both.
- **The footer says when it drops hints.** When not everything fits, the footer reserves room for `? more` at the end (`a_footer_that_drops_hints_says_where_the_rest_are`). On a terminal narrower than the pointer itself, it draws nothing rather than half a pointer. The README's sample footer line was updated to match what poptop draws at 78 columns.
