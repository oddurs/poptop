---
id: 89
title: main.rs and app.rs are 3,400 lines with no tests of their own
type: chore
status: done
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

- [x] Argument parsing is a pure function, tested with a table of argv → outcome, including every error exit
- [x] Every `process::exit` goes through one path that restores the terminal if it was changed
- [x] Review notes are recorded on this item: findings fixed, and findings filed as items

## How it was resolved

**Argument parsing is a pure function.** Settings flags were already parsed in `config::resolve` and tested there. What was left was the command word, handled in a 260-line `match` inside `main`. `command(&[String]) -> Result<Command, Usage>` now decides it without doing any of it. Two tables cover it: 21 command lines that run, and 19 that are refused, each with the start of its message. The runtime failures (no state directory, a day that cannot be read) need a filesystem and stay out of the table. They exit 2 and 1 as before, through the one exit path below.

**One way out.** Every exit goes through `exit(code)`: every usage error, every runtime failure, and `--check-theme`'s verdict. `exit` gives the terminal back first if poptop has taken it. No exit happens while it does today; now none can. `fail(warnings, why, code)` prints what was already found, then the reason, then exits. Before, some paths flushed the warnings first, and `--check-theme`'s did not flush them at all.

**Findings fixed**

- **Arguments after a command were dropped silently.** `poptop --read 2026-09-08 --once` opened the day and ignored `--once`. `--days junk`, `--schema x` and `--export json DAY extra` all ran. Each is refused now: `--read does not take '--once' — one command at a time`.
- **`--help` and `--version` opened the collector first.** `Platform::new()` ran before the arguments were looked at, so `--help` needed a readable `/proc`, and on macOS it paid for sysinfo's full refresh. Now only the commands that sample open it: the TUI, `--once`, live `--export` and `--bench`.
- **Two spellings, half supported.** `--report=DATE` and `--export=json` worked, but `--read=DATE` and `--check-theme=NAME` were unrecognised options. Every command word now takes both forms.
- **A chord typed into the filter or the jump box was inserted as its letter.** Ctrl-U added `u` and Alt-B added `b`. Characters with Ctrl or Alt are now ignored there; Shift still types.
- **`S` could show the IO columns with nothing collecting into them.** When the constraint is the disk, `S` set `show_io` directly. It skipped what `i` does: re-arming the collection ratchet, and taking IO back from the budget if it had been withheld. On a machine where the startup probe had turned IO off, the columns came back as dashes. Both keys now go through `App::reveal_io`.
- **Esc quits from the main table, and the help never said so.** It is documented in `--help` and the README key table, along with what Esc does in the filter, the jump box and a signal prompt.

**Filed:** 0102. Once a process is selected, nothing unselects it: the one state transition no key undoes. The fix is a key map decision, because Esc means quit.

**Transitions tested directly**, not through a rendered frame. Each text box takes text and not chords (checked to fail without the fix). Every mode can be left without quitting: the filter by Esc or Enter, a fresh `/` clears it, the jump box cancels, `t`, `d`, `K`, `i`, `y` and `C` each undo themselves, and grouping cycles back to off and never coexists with the tree. A signal prompt answered with anything but `y` sends nothing and does not quit. Showing the IO columns re-arms collection whichever key does it.

**Reviewed without a finding:** `ask_to_signal` and `confirm_signal` (the refusals are ordered correctly, and `replaying` is checked before `is_live`), `select_delta` (an empty filter result keeps the selection), and the sample loop's clock (a missed deadline re-bases instead of bursting).

