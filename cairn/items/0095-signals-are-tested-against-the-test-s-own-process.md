---
id: 95
title: Signals are tested against the test's own process
type: chore
status: done
milestone: r3
labels:
- testing
created: 2026-09-18
updated: 2026-09-18
priority: p2
effort: s
area: signal
---

## Problem

`signal.rs`'s tests check `kill(me, 0)` and `kill(i32::MAX, 0)`. The refusal rules (recycled identity, unknown start time, scrubbing, the process being poptop itself) are tested as logic. None is tested against a real process that poptop found through its own collector.

## Proposal

Spawn a child, collect it through the real collector, and send it a signal through the same path the `x` key uses; assert that it received the signal. Then make the child exit, and check that the stale identity is refused by name. Where possible, reuse its pid with a new child as well.

## Acceptance criteria

- [x] End-to-end test: collected child → signal → child observes it
- [x] A stale `(pid, started)` is refused with the recycled-process message
- [x] Runs without root on both CI targets

## How it was resolved

Three tests in `main.rs` go through the whole path. A real `sleep` child, the platform's real collector, and `App` driven by key presses only: `/`, `pid = N`, Enter, Down, `x` or `X`, `y`. They pass without root on macOS and on Linux. 0087 made `send` re-check against the kernel, so these also cover the step between the sample and the signal.

- **Collected child, then signal, and the child observes it.** The prompt names `sleep` and the child's pid. After `y` the child exits with signal 15, and the note reads `sent TERM to sleep (pid N)`.
- **A stale identity is refused with the recycled-process message.** The child is selected and `X` pressed, and the pending signal is given the start time and name of an earlier process on that pid. That is exactly what a reader holding an old row has. After `y` the note reads `pid N is sleep now, not postgres — nothing was sent`, and the child is still running.
- **A process that exits between the sample and `y`** is refused, as gone (or as another process, if its pid was handed on at once), not signalled.

**Pid reuse with a new child** was tried and dropped. The test started up to 2,000 processes looking for the freed pid, and neither platform handed it out again: Linux's `pid_max` is 4,194,304 and macOS's is 99,999. Forcing a particular pid needs `ns_last_pid`, which needs root or a new PID namespace. A test that almost never checks anything was worse than none. The refusal that case would reach is the one the stale-identity test asserts, and `signal::send`'s own tests cover the kernel re-check for a process whose start time differs.

