---
id: 102
title: Once a process is selected, nothing unselects it
type: bug
status: backlog
milestone: later
labels:
- review
created: 2026-09-18
updated: 2026-09-18
priority: p3
effort: s
area: app
---

## What happens

`App::selected` is set by the arrow keys and by nothing else, so no key sets it back to `None`. Once a process has been picked, poptop follows it for the rest of the session. `watched_but_absent` keeps saying it is not there after it exits, and `d` shows its history, not the machine's. The only way back to "nothing selected" is to quit.

Found in the 0089 review of `main.rs` and `app.rs`, as the one state transition no key undoes.

## What should happen

A key that clears the selection. The natural one is Esc, but in the main table Esc currently quits, and changing that is a decision about the key map, not a bug fix. Other choices are a second press of the arrow at the edge of the list, or `u`.

## Reproduction

1. Press Down to select a process.
2. Try to return to no selection.
