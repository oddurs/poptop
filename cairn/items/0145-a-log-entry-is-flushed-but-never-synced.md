---
id: 145
title: A log entry is flushed but never synced
type: bug
status: backlog
milestone: v1.2
created: 2026-09-20
updated: 2026-09-20
priority: p1
effort: s
area: log
---

## Problem

`log::append` writes one framed entry and calls `flush()`. That hands the bytes to the kernel, which is enough for a crash of poptop and nothing at all for a power cut or a hard reset: the page cache can hold the last entries for thirty seconds. The whole claim of the day log is that it outlives the process — "poptop logs if it is left running" — and the case somebody most wants the log for, a machine that went down, is the case where the tail of it may not be there.

The store has the same question and a different answer: it is written once on a clean exit, so a crash costs at most the session, which the README states.

## Proposal

Decide the durability rule and write it down, then implement and measure it. Candidates: `sync_data` per entry (correct, and at a ten-minute default interval free, but a one-second `log-interval` pays it every second); sync every N entries or every N seconds; sync on rotation and on exit. Measure the cost at both extremes before choosing.

## Acceptance criteria

- [ ] The rule is stated in the recording documentation, with the cost measured
- [ ] A test proves the data is on disk after the rule says it is (`sync_data`, then read back through a fresh handle)
- [ ] The cost at a one-second log interval is within the performance budgets (0097)
- [ ] What a power cut can lose is stated in one sentence a user can act on
