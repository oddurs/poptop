---
id: 88
title: Three hundred unwraps, and nobody has counted how many a user can reach
type: chore
status: backlog
milestone: r2
labels:
- review
created: 2026-09-18
updated: 2026-09-18
priority: p2
effort: m
area: app
---

## Problem

`unwrap()` and `expect()` appear about 317 times outside `ui_tests.rs`. Many are in test modules and many are provably fine. The rest are panics a user could reach. A panic in a TUI is worse than an error: unless a panic hook restores it, the terminal is left in raw mode on the alternate screen.

## Proposal

Classify every non-test site into three groups: proven infallible (leave it and add the reason to the `expect` message), reachable (return an error), or unclear (investigate). Confirm there is a panic hook that leaves raw mode and the alternate screen, restores the cursor, and then prints the panic.

## Acceptance criteria

- [ ] Every non-test `unwrap` is replaced, or turned into an `expect` whose message says why it cannot fail
- [ ] A test (or the pty harness from the r3 sprint) proves a panic restores the terminal
- [ ] `clippy::unwrap_used` is denied outside tests, with justified exceptions
