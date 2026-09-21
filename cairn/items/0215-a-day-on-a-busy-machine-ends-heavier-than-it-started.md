---
id: 215
title: A day on a busy machine ends heavier than it started
type: bug
status: backlog
created: 2026-09-20
updated: 2026-09-20
priority: p2
area: app
effort: m
---

## Problem

The 24-hour macOS soak (0099) failed one check: resident memory grew 164 MiB after warm-up, against a bound of a tenth of the warm figure. The 6-hour Linux soak, on the same machine in a container, grew nothing at all.

What makes it worth an item rather than a shrug is that the growth is not in what poptop is holding on purpose. The buffer is ten minutes of samples, and the size of a sample is visible in the log: each entry is the same sample encoded. Over the run the entries did not grow.

| elapsed | resident | a log entry |
| --- | --- | --- |
| 1h | 471 MB | 93 KB |
| 4h | 515 MB | 92 KB |
| 8h | 520 MB | 94 KB |
| 12h | 361 MB | 71 KB |
| 16h | 496 MB | 85 KB |
| 20h | 565 MB | 87 KB |
| 23h | 549 MB | 88 KB |

At 23 hours the machine's process table was *smaller* than at one hour and poptop was holding 80 MB more, with a peak of 676 MB. Descriptors were flat at 8 the whole way, and CPU averaged 16% of one core at a 200ms interval on a desktop with about seven hundred processes.

The run: `soak/results/macos-24h`. The soak generates about five short-lived processes a second, and the machine it ran on was also building and testing all day, so process *churn* was high even where the process *count* was not.

## What has been ruled out

- **The collector's per-pid caches.** `names`, `cmds` and `faults` are pruned against the pids walked each sample, on both backends, and there is a comment and a test saying why.
- **The smoothing table.** Rebuilt per draw from the samples in the window.
- **The sample data itself.** Log entry sizes above; and the exited-process lists are inside those entries.
- **Descriptors, and the log.** Flat, and pruned as configured.
- **A pure ratchet.** Memory comes back down — 520 MB at 8h, 361 MB at 12h — so something does return pages; the growth is not simply a high-water mark that is never released.

## What to try next

- Run the soak with `--store=off --log=off` and with logging alone, to see whether the growth needs either.
- Run it on Linux at the same interval and process count as the Mac, rather than in a five-process container: the Linux soak may simply have had nothing to accumulate.
- Instrument allocation: a counting global allocator behind a feature, sampled into the metrics file, to tell "poptop is holding it" from "the allocator has not returned it".
- If it is the allocator holding freed pages, say so in the documentation and bound what poptop asks for instead, rather than chasing a leak that is not there.

## Acceptance criteria

- [ ] The growth is attributed: poptop holding it, or the allocator not returning it
- [ ] If poptop is holding it, what it holds is bounded and a test holds the bound
- [ ] A 24-hour run on a busy machine meets the memory bound, or the bound is restated with the reason
- [ ] `soak/analyse`'s bound says which of the two it is checking
