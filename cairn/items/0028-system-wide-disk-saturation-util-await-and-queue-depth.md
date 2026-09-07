---
id: 28
title: 'System-wide disk saturation: %util, await and queue depth'
type: bug
status: done
milestone: v1.0
created: 2026-09-07
updated: 2026-09-07
priority: p1
area: collect
---

## Problem

The header can tell you the machine is stalled on IO — `WAIT 26.7%`, `BLOCKED 30`
— and then leaves you there. The next question is always *which device, and how
badly*, and poptop has no answer. The per-process `DISK R/s` columns say who is
issuing IO, which is a different question: a disk at 100% utilisation with 40ms
service times is slow for everyone on it, including processes doing almost
nothing.

This is the largest remaining gap against the layout thesis, because it is the
one place the default view raises a question it cannot answer.

## What is available

`/proc/diskstats`, one file, twenty fields per device, no new dependency.
Measured in a container while writing 300MB:

```text
over ~1s on vda:
  IOPS             45 read + 433 write
  throughput       5.6 MB/s read, 300.3 MB/s write
  %util            7.8%
  await            7.82 ms per IO
  avg queue depth  3.80
```

All of it is derived from deltas the same way `iostat` does it:

- `%util` = Δ(field 13, ms doing IO) / elapsed
- `await` = Δ(ms reading + ms writing) / Δ(reads + writes completed)
- queue depth = Δ(field 14, weighted ms) / elapsed
- throughput = Δsectors × 512 / elapsed

**Saturation, not utilisation.** `%util` and `await` are the figures that answer
"is the disk the bottleneck"; throughput alone does not — a disk can be at 100%
utilisation moving 2 MB/s of random reads.

## macOS

`sysinfo::Disks::refresh` costs **12.5ms steady state**, measured — three times
poptop's entire 4ms sample. It cannot run per-sample. Options, in order of
preference:

1. Read IOKit directly for the counters actually wanted.
2. Refresh on a slower cadence than the sample loop and mark the figures stale.
3. Ship Linux-only at first and render `—` on macOS, which is what the tool
   already does everywhere else it cannot know.

Option 3 is an acceptable first cut; the `/proc` backend is the good one and
this is on-thesis enough to be worth having on one platform.

## Acceptance criteria

- [ ] Per-device `%util`, `await`, queue depth, IOPS and throughput on Linux
- [ ] Retained per-sample, so a latency spike can be scrubbed back to
- [ ] Loop and ram devices excluded; partitions not double-counted with disks
- [ ] macOS renders `—` rather than a fabricated zero, and says why once
- [ ] Measured: collection cost against the 1.41ms Linux baseline
