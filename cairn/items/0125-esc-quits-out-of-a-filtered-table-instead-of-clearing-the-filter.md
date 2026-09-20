---
id: 125
title: Esc quits out of a filtered table instead of clearing the filter
type: bug
status: backlog
milestone: later
labels:
- ui
created: 2026-09-19
updated: 2026-09-19
priority: p2
---

## Problem

`/`, a query, `Enter` — the box closes and the filter stays applied, which
is right. `Esc` then quits poptop.

The back-out ladder in `main.rs` is: the filter box, the jump box, a signal
prompt, the selection, then the program. An applied filter is not on it, so
`app.deselect()` returns false and `should_quit` is set. But an applied
filter is exactly the kind of state a reader expects Esc to leave — the
table says `processes (6)` where it said `(724)`, and the way out of that is
the same key that got you out of the box a moment ago.

Reproduced at 140x24 on macOS: `/cpu > 5 and user = oddurs`, `Enter`, `Esc`
— poptop exits.

## Proposal

Put the applied filter on the ladder, above the selection: Esc clears it and
redraws the whole table. `q` and `Ctrl-C` still quit from anywhere, so
nothing loses the fast way out.

## Acceptance criteria

- [ ] Esc with a filter applied clears it and does not quit
- [ ] Esc with both a filter and a selection clears the selection first
- [ ] A test walks the whole ladder in one sequence
