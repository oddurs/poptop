---
id: 94
title: The TUI has never been run in a terminal by a test
type: chore
status: done
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

- [x] Start, resize three times, `q`: exits 0 and leaves the terminal as it found it
- [x] Ctrl-C and SIGTERM do the same
- [x] A forced panic restores the terminal (shared with the unwrap audit)
- [x] Runs in CI on both targets, or is marked and run by `./check`

## How it was resolved

`tests/tui.rs` starts the built binary on a real pty, as a shell would: a session of its own, the pty as its controlling terminal, `TERM=xterm-256color`, and a hermetic home. It uses no new dependency. `openpty`, `ioctl`, `tcgetattr` and `setsid` are declared by hand, and the ioctl numbers were checked against glibc and the macOS SDK. The terminal's settings are read before and after through the pty's master side. On macOS the terminal side is revoked when its session leader exits, so the master is the only side still readable afterwards. After each exit the tests require the settings as they were given, the alternate screen left after it was last entered, and the cursor shown after it was last hidden.

- **Start, three resizes, `q`:** 60×15, 200×60, 30×8, each through `TIOCSWINSZ` so it arrives as SIGWINCH, and each followed by a redraw. Then exit 0, with the terminal as it was.
- **Ctrl-C** exits 0 with the terminal restored. **SIGTERM** and **SIGHUP** did not: poptop died of the signal, raw mode was left on the shell, the alternate screen was left open, and the store was not saved. Both now set a flag through `signal-hook`, which is already in the tree through crossterm and is now declared directly. The input wait checks the flag every 100ms without redrawing, and the monitor then quits the way `q` does: exit 0, terminal restored, history saved.
- **A forced panic** restores the terminal and prints its message after leaving the alternate screen. It is forced by `POPTOP_PANIC_AFTER_FIRST_FRAME`, which only a debug build reads, so no release binary can be told to die. This also closes 0088's open criterion.
- **Key sequences split across reads.** Over a slow link Down arrives as `ESC`, then `[B`. crossterm reads a lone `ESC` as the Esc key. In poptop Esc backs out of whatever is open and, with nothing open, quits, so a laggy ssh session could drop a selection or quit poptop with an arrow key. `rejoin` now waits up to 100ms after a bare Esc (Vim's `ttimeoutlen`; Neovim's 50ms lost the race on a loaded CI runner) and puts a following `[`/`O` sequence back together: arrows (with Shift, Alt or Ctrl), Home, End, PageUp and PageDown. A sequence it does not know is dropped whole, not half-typed. A real Esc is still Esc, 100ms later. The pty test fails without the fix, and unit tests cover each sequence, a cut-off one, and a real Esc.
- **`--read DAY` on a terminal**, the one path 0091 could not reach: it opens paused on the day and `q` leaves it cleanly.

All seven run in CI on both targets, and under Rosetta, as part of `cargo test`. They take under a second.

