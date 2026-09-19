---
id: 91
title: No test runs the binary
type: chore
status: done
milestone: r3
labels:
- testing
created: 2026-09-18
updated: 2026-09-18
priority: p1
effort: m
area: cli
---

## Problem

There is no `tests/` directory. Everything a script depends on is tested only from inside the crate, if at all: exit codes, `--help`, unknown flags, `--once`, `export`, `report`, reading a day, and stderr versus stdout. CI's two live steps check that `--once` does not crash and that a closed pipe does not panic.

## Proposal

Add integration tests in `tests/` that run the built binary with `assert_cmd` or plain `std::process::Command`. Use a temp `HOME` and log directory so they are hermetic. Cover every subcommand and flag in `--help`, and the exit code and stream of every error.

## Acceptance criteria

- [x] Every flag and subcommand in `--help` is invoked by at least one test
- [x] Every documented exit code is produced by a test and asserted
- [x] Errors go to stderr and data to stdout, asserted for each command that emits data
- [x] The tests pass on both CI targets without network access or root

## How it was resolved

`tests/cli.rs` runs the built binary 15 ways, using `std::process::Command` with no new dependency. Each run gets its own `HOME`, config and state directories, and an environment cleared down to `PATH`, so nothing on the machine running the tests can change what it does. No root or network is needed. It runs on both CI targets and on x86_64 macOS under Rosetta.

- **Every flag in `--help` is run.** A meta-test reads `--help` from the binary and fails if a flag it names does not appear in the test file. A new flag without a test fails the build. Every setting is run once at a value it takes and once at one it does not, on each side of the command word.
- **Every exit status is produced and asserted**, and each is now documented in `--help` (`EXIT STATUS:`) and the README. `0` covers every command. `1` covers a day that is not there, and a `--check-theme` of `classic`. `2` covers each kind of refused command line, each bad setting, a bad theme, no state directory, and the monitor without a terminal.
- **Streams.** `Run::ok` requires data on stdout and nothing on stderr. `Run::refused` requires nothing on stdout, and a first stderr line that starts `poptop: ` followed by the expected reason. A config file with bad lines warns on stderr and still exits 0.
- **A recorded day end to end:** `--once --log=on` twice from cron's point of view, then `--days` lists it, `--report` summarises it in all three spellings, and `--export json DAY` gives one object per sample.
- **A reader that goes away:** `--schema`, `--once` and `--help` into a closed pipe exit 0 without a panic.

**Found and fixed.** Started without a terminal, as by a cron line that forgot `--once`, `poptop` panicked inside `ratatui::init`, exited 101, and wrote escape codes to the redirected stdout. It now checks that stdin and stdout are terminals before reading anything, and exits 2 naming `--once` and `--export=json`. `ratatui::try_init` replaces `init`, so a terminal that refuses is an exit 1 with the reason, not a panic.

`--read DAY` on a real terminal is the one path not run here; it needs the pty harness in 0094.

