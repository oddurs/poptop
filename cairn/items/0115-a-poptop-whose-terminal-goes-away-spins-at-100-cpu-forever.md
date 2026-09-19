---
id: 115
title: A poptop whose terminal goes away spins at 100% CPU forever
type: bug
status: done
milestone: r4
assignee: Oddur Sigurdsson
labels:
- validation
created: 2026-09-19
updated: 2026-09-19
priority: p0
effort: s
area: app
---

## What happens

Close the terminal poptop is running in from the other end without pressing a key, the way an SSH disconnect or a killed terminal emulator does. poptop doesn't exit. It stays running, orphaned, at 98–100% of a core, indefinitely. One left over from a test harness had been spinning for 49 minutes before it was noticed.

The hangup handler is fine: SIGHUP sets poptop's stop flag, and the main loop checks it. But the loop never gets to check it. `sample` shows every sample inside `crossterm::event::poll` → `UnixInternalEventSource::try_read` (crossterm 0.29, the mio backend). In that function's inner read loop, a terminal that has gone away returns `Ok(0)` or an error other than `WouldBlock`/`Interrupted`, and neither breaks the loop. It calls `read` again, forever, ignoring the poll's timeout.

## What should happen

poptop exits when its terminal goes away, as the README says: "SIGTERM, SIGHUP and Ctrl-C quit the same way, and give the terminal back".

## Reproduction

1. Start poptop on a pty (Python's `pty.fork`, or `script`).
2. Close the pty's master side without sending `q`.
3. `ps -o %cpu,stat -p <pid>` four seconds later shows `98.8 Rs+`.

## How it was resolved

Two bugs, one on each side of the exit.

**1. The spin.** crossterm's default input source (mio) reads the terminal in a loop that never ends on end-of-file or EIO. poptop now enables crossterm's `use-dev-tty` source, which polls the terminal with poll(2), stops at end-of-file, respects the poll timeout, and returns real errors. So `event::poll` comes back, and poptop sees its stop flag.

**2. The abort after it.** With the spin gone, poptop exited by SIGABRT. Its terminal was gone, the restore failed with EIO, and ratatui reports that failure with `eprintln!`, which panics when stderr is the dead terminal too. The `Terminal`'s drop does the same when showing the cursor. So:
- A terminal error that means the terminal is gone (EIO, ENXIO) is now a request to quit, noted in `TERMINAL_GONE`.
- The restore is attempted with `try_restore`, and if that finds the terminal gone, poptop gives up the `Terminal` without writing anything.
- `flush` writes its warnings without `eprintln!`, so a closed stderr can't crash the exit.

History is still saved, because that goes to a file.

The restore-time check matters on its own. A hangup signal and a dead terminal arrive together. In a release build the signal usually wins, the loop ends normally without ever reading the EIO, and the restore is the first thing to meet the dead terminal.

**Tests.** `sighup_exits_and_gives_the_terminal_back` already existed, but it sends the signal while the terminal is still there, which is why this was missed. Two new pty tests take the terminal away by closing the only handle on its far side, while draining output the way a terminal emulator does:
- `a_terminal_that_goes_away_takes_poptop_with_it`
- `a_hangup_that_beats_the_dead_terminal_still_exits_cleanly` (the signal first)

Both require exit status 0 within five seconds. On the original code the regression tests fail. Without the restore-time check, the signal-first test dies of SIGABRT five times out of five. The release build, reproduced by hand, went from running at 98.8% CPU indefinitely to exiting with status 0 and writing nothing.

Worth reporting upstream: crossterm 0.29's mio source (`event/source/unix/mio.rs`, the inner read loop in `try_read`) spins on `Ok(0)` and on errors other than `WouldBlock`/`Interrupted`.
