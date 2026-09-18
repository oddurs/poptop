---
id: 89
title: main.rs and app.rs are 3,400 lines with no tests of their own
type: chore
status: backlog
milestone: r2
labels:
- review
created: 2026-09-18
updated: 2026-09-18
priority: p2
effort: l
area: app
---

## Problem

`main.rs` (1,505 lines) parses arguments by hand and has at least ten separate `std::process::exit` paths. `app.rs` (1,885 lines) holds the state machine that every key press goes through. Neither has a `mod tests`. `ui_tests.rs` exercises `app` through rendering, which shows what appears on screen but not why a state transition happened. Argument parsing is covered only by what CI happens to run.

## Proposal

Review both files for divergent handling of the same flag, exits that skip cleanup, and state transitions that no key can undo. Move argument parsing into a function that returns a value (or an error and exit code) and can be tested without spawning a process. Add table tests for it and for the `app` transitions that `ui_tests` covers only indirectly.

## Acceptance criteria

- [ ] Argument parsing is a pure function, tested with a table of argv → outcome, including every error exit
- [ ] Every `process::exit` goes through one path that restores the terminal if it was changed
- [ ] Review notes are recorded on this item: findings fixed, and findings filed as items
