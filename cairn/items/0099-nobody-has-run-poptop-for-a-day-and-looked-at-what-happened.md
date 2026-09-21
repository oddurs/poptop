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
- [x] The day log rolls over at local midnight with no lost or duplicated samples
- [x] A clock step backwards and a sleep/wake cycle leave the timeline correct and a gap marked (0012)
- [x] The soak script and one run's results are committed

## Where this stands

Two runs are in `soak/results`, and the scripts that made them are in `soak/`.

**Linux, 6 hours, every check passed.** Memory flat at 16 MiB after warm-up (0 MiB of a 16 MiB bound), descriptors between 7 and 8, 3.1% of one core. 2114 samples across two day files: none repeated, none out of order, one gap of 186s where the process was stopped for three minutes, and 10.2s between the last sample of one local day and the first of the next. The wall clock was stepped back five minutes an hour in; the day covers that period twice, 59 samples where one pass would be 30, with nothing lost.

**What the clock step taught.** The first analysis called that run a failure, looking for a backwards jump in the exported timestamps. There cannot be one: the reader sorts a day by timestamp, because a file two poptops wrote at once is not in order. The check now asserts what a step back does leave — the stepped-over period recorded twice — and "out of order" is its own separate failure. poptop was right and the check was wrong.

**macOS, 24 hours, one check failed.** Descriptors flat at 8, midnight rollover clean, the three-minute stop the only gap, 16% of one core at a 200ms interval on a desktop with about seven hundred processes. Resident memory grew 164 MiB after warm-up against a 51 MiB bound, while the size of what poptop buffers — visible as the log's entry size — did not grow at all. That is **0215**, with the numbers and what has been ruled out.

So this item stays open on its first criterion until 0215 says whether poptop is holding that memory or the allocator is not returning it.
