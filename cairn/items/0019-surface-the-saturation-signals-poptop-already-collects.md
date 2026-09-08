---
id: 19
title: Surface the saturation signals poptop already collects
type: feature
status: done
milestone: v0.2
created: 2026-09-06
updated: 2026-09-06
priority: p0
effort: s
area: collect
---

## Problem

The default layout answers "how much CPU and memory is in use". Every guide to
diagnosing a slow machine says that is the least diagnostic pair of numbers on
screen: utilization tells you what a machine is doing, **saturation** tells you
whether it is in trouble.

The canonical illustration is thirty processes blocked on one hung NFS mount:
load average 30 on a box whose CPUs are completely idle. "High load, low CPU"
is named in every troubleshooting guide as *the* confusing case people hit, and
poptop currently renders it as a calm graph and an unexplained `LOAD 30.00`.

Worse, poptop already collects the answer and throws it away. `CpuTimes::parse`
correctly counts `iowait` as idle — the doc comment says why — and then keeps
only `idle` and `total`, so the figure that explains the discrepancy is
discarded. The same `/proc/stat` read carries `procs_running` and
`procs_blocked`, which are vmstat's `r` and `b` and, in the words of the
literature, "roughly the instantaneous value the load average is smoothing".

## Proposal

Keep `iowait`, `procs_running` and `procs_blocked` on `Sample`, and put them in
the header:

    CPU 12.4%   WAIT 61.2%   RUN 1/14   BLOCKED 23   ...

`RUN` is normalized against cores, because a raw load figure means nothing
without its denominator — 4.0 is catastrophic on one core and idle on 96. The
existing `LOAD` triple is normalized for the same reason or dropped, since it
is a smoothed version of what `RUN` now says exactly.

`BLOCKED` is the D-state count: the most direct available answer to "why is
load high when nothing is running".

None of this costs a syscall. All three come from the `/proc/stat` read poptop
already performs every sample.

macOS has no equivalent for the blocked count, so it reports `None` rather than
zero, as with every other figure the platform will not say.

## Acceptance criteria

- [ ] `iowait`, `procs_running` and `procs_blocked` retained per sample
- [ ] Header shows wait, run/cores and blocked
- [ ] Load is normalized against cores, or dropped in favour of `RUN`
- [ ] macOS degrades to `None`, never a fabricated zero
- [ ] Sampling cost unchanged, measured with `--bench`
