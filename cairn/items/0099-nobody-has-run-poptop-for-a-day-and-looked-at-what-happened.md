---
id: 99
title: Nobody has run poptop for a day and looked at what happened
type: chore
status: backlog
milestone: r4
labels:
- validation
created: 2026-09-18
updated: 2026-09-18
priority: p2
effort: l
area: app
---

## Problem

The tests run for seconds. Real use is days: memory grows through interning (0007) and history, day logs roll over at midnight, and the clock steps under NTP, changes at DST, and jumps after a laptop sleeps. None of these has been observed over a long run.

## Proposal

A soak run: 24 hours at a 200ms interval on Linux and on macOS, logging, with a process churn generator running. Record RSS, open file descriptors, CPU and log size every minute. Include a forced clock step and a sleep/wake cycle. Keep the script so it can be repeated.

## Acceptance criteria

- [ ] RSS and file descriptors are flat after warm-up (the growth bound is stated and met)
- [ ] The day log rolls over at local midnight with no lost or duplicated samples
- [ ] A clock step backwards and a sleep/wake cycle leave the timeline correct and a gap marked (0012)
- [ ] The soak script and one run's results are committed
