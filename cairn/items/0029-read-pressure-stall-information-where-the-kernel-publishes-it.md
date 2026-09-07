---
id: 29
title: Read Pressure Stall Information where the kernel publishes it
type: feature
status: backlog
milestone: v1.0
created: 2026-09-07
updated: 2026-09-07
priority: p1
area: collect
---

## Problem

Pressure Stall Information is the most direct answer to "why is this slow" that
Linux publishes, and poptop does not read it. One number —
`io full avg10=4.72` — means *4.72% of the last ten seconds, every runnable task
was stalled waiting on IO*. No amount of `%util` or `iowait` says that as
plainly, because both can be high on a machine that is getting its work done.

## This reverses an earlier decision, carefully

PSI was rejected earlier in this project as the layout centrepiece, on the
grounds that it was absent on the kernel here and on the distro images checked.
Re-measured, it is present and populated:

```text
/proc/pressure/io      some avg10=4.72 avg60=2.12 avg300=4.26 total=738078054
/proc/pressure/cpu     some avg10=0.40 avg60=0.27 avg300=0.47 total=106158967
/proc/pressure/memory  some avg10=0.00 avg60=0.00 avg300=0.00 total=545396
```

The caveat matters more than the reversal. Checking four distro images tests
**one kernel**, because containers share the host's — the near-identical
`total=` across `debian:12`, `ubuntu:24.04`, `alpine:3.20` and `rockylinux:9`
proves it. So this says PSI exists here, and says nothing about a machine
running an older kernel, `CONFIG_PSI=n`, or a distro that needs `psi=1` on the
kernel command line.

That is exactly why it belongs as an **optional signal, detected at runtime**,
never as the centrepiece. The original objection was right about the risk and
wrong about the availability; making it the headline would repeat the taskstats
mistake of building a view around something a third of users cannot see.

## What should happen

- Read `/proc/pressure/{io,cpu,memory}`, `some` and `full`, `avg10`.
- Absent file → `None` → em dash, like `forks` and `iowait` already do.
- A header figure, ranked below the existing ones, and a timeline series that
  appears only when the kernel publishes it — the same rule `WAIT` already
  follows.

macOS has no equivalent and reports `None`.

## Acceptance criteria

- [ ] `/proc/pressure` read when present, `None` when not, never zero
- [ ] Availability decided once at startup and said in `notes()`
- [ ] Nothing in the default view *depends* on PSI being there
- [ ] A test proves the absent case renders an em dash rather than 0.0
