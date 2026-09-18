---
id: 95
title: Signals are tested against the test's own process
type: chore
status: backlog
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

- [ ] End-to-end test: collected child → signal → child observes it
- [ ] A stale `(pid, started)` is refused with the recycled-process message
- [ ] Runs without root on both CI targets
