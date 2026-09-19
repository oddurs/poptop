---
id: 88
title: Three hundred unwraps, and nobody has counted how many a user can reach
type: chore
status: done
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

- [x] Every non-test `unwrap` is replaced, or turned into an `expect` whose message says why it cannot fail
- [x] A test (or the pty harness from the r3 sprint) proves a panic restores the terminal
- [x] `clippy::unwrap_used` is denied outside tests, with justified exceptions

## How it was resolved

**Counted by clippy, not by grep.** With `unwrap_used` and `expect_used` switched on and tests exempted, both targets give the same answer: of the ~317, **eight** are outside tests. The rest are in test modules and `ui_tests.rs`.

- Four bare `unwrap`s, all unreachable, and all rewritten so they are no longer unwraps. Two were in `log.rs` (`step` and `starts_block`), turning a four-byte slice into an array. `starts_block` had already bounds-checked the slice, and `step` relied on its caller to check. Both now go through `length_at`, which reads the length with `first_chunk` and returns `None` where fewer than four bytes are left, so the caller's check could be dropped. The other two were in `procinfo::parse_statfs`, at constant offsets inside a fixed-size record, and are now `first_chunk` reads that fall back to zero.
- Four `expect`s that already say why they cannot fail and were left alone: `history.rs` twice (the buffer was checked non-empty three lines up), `check.rs` (`wrap` starts its vector with one line), `app.rs` (the key was inserted on the line above).

`unwrap_used` is denied in `Cargo.toml`, and `clippy.toml` allows it in tests. `expect` is not denied: the rule is that a value which cannot be absent says why, and `expect` is where it says it. `panic!`, `unreachable!`, `todo!` and `unimplemented!` appear only in tests.

Out of scope, and noted: indexing and slicing, about 350 sites outside tests by `clippy::indexing_slicing`. r1 fuzzed the parsers where the index comes from outside bytes. The rest index by the program's own lengths, and auditing them one by one would be an item of its own.

**The terminal after a panic.** `ratatui::init` installs a hook that leaves raw mode and the alternate screen, then prints the panic. poptop has no threads outside tests, so a panic always unwinds through the one `Terminal`. No `process::exit` runs while the terminal is raw: all of them run before `ratatui::init` or after `ratatui::restore`. One gap: the hook does not show the cursor. It came back only when `Terminal` dropped during unwinding, which would not happen under an abort. poptop's hook now shows it first and then calls ratatui's. Checked by hand by injecting a panic after the first frame and running the binary in a pty (`script`). The output reads: enter the alternate screen, hide the cursor, show the cursor, leave the alternate screen, then the message.

The test that proves it came with the pty harness in 0094: `a_panic_gives_the_terminal_back` forces a panic after the first frame in a debug build, then checks the terminal's settings, the alternate screen, the cursor, and that the message was printed after the screen was restored.

