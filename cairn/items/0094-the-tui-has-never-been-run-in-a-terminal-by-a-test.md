---
id: 94
title: The TUI has never been run in a terminal by a test
type: chore
status: backlog
milestone: r3
labels:
- testing
created: 2026-09-18
updated: 2026-09-18
priority: p2
effort: m
area: ui
---

## Problem

`ui_tests.rs` (11,256 lines, 342 tests) renders frames into a buffer. That is thorough for layout, but it never tests the terminal: entering and leaving the alternate screen, raw mode, resize (SIGWINCH), key sequences arriving split across reads, and what the terminal looks like after `q`, Ctrl-C, SIGTERM or a panic.

## Proposal

Add a small pty harness (for example `portable-pty` or `expectrl`) that starts poptop, sends keys, resizes, and exits, then checks the terminal state afterwards. Keep it to a few end-to-end cases; the buffer tests remain the main way to test layout.

## Acceptance criteria

- [ ] Start, resize three times, `q`: exits 0 and leaves the terminal as it found it
- [ ] Ctrl-C and SIGTERM do the same
- [ ] A forced panic restores the terminal (shared with the unwrap audit)
- [ ] Runs in CI on both targets, or is marked and run by `./check`
