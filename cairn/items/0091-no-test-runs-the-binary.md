---
id: 91
title: No test runs the binary
type: chore
status: backlog
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

- [ ] Every flag and subcommand in `--help` is invoked by at least one test
- [ ] Every documented exit code is produced by a test and asserted
- [ ] Errors go to stderr and data to stdout, asserted for each command that emits data
- [ ] The tests pass on both CI targets without network access or root
