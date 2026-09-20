---
id: 146
title: At the byte budget logging stops, and the session it was recording is the one it drops
type: bug
status: backlog
milestone: v1.2
created: 2026-09-20
updated: 2026-09-20
priority: p2
effort: m
area: log
---

## Problem

`log::append` weighs the whole directory against `log-bytes` and returns `false` when it is full, and today's file is never pruned because it is the history of the running session. Together those mean a long session at a short `log-interval` reaches the budget and then logs nothing more, forever, with one note in the footer. The reader who set a one-second interval to catch something is the reader who gets the least of it, and the samples they lose are the recent ones — the ones nearest whatever they are waiting for.

Measured in the code's own comment: 87 KB a sample, so a one-second interval is seven gigabytes a day against a 512 MB default budget. The budget is reached in under two hours.

## Proposal

Make room instead of stopping. Options, in the order they seem worth trying: prune older days first (already done) and then older *parts* of today, which needs today's file to be more than one file — size-based parts within a day, `poptop-20260920.001`; or drop to a coarser interval and say so, which keeps a whole day at lower resolution rather than two hours at full.

Whatever it is, the rule a reader can state should be: poptop keeps the most recent `log-bytes` of history, not the oldest.

## Acceptance criteria

- [ ] At the budget, the newest samples are kept and the oldest are dropped
- [ ] The rule is one sentence in the recording documentation
- [ ] `--days` shows what a day actually holds when it has been trimmed
- [ ] A test fills the budget and checks which samples survived
