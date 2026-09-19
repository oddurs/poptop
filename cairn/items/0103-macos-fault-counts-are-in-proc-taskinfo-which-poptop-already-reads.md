---
id: 103
title: macOS fault counts are in proc_taskinfo, which poptop already reads
type: feature
status: backlog
milestone: later
labels:
- validation
created: 2026-09-19
updated: 2026-09-19
priority: p3
effort: s
area: darwin
---

## Problem

The minor and major fault columns are em dashes on macOS. The README said this is because sysinfo publishes no fault counts. That is true of sysinfo, but poptop does not need it: `proc_taskinfo`, which it already reads for every process it owns to get the thread count and (since 0096) the virtual size, carries `pti_faults` at offset 52 and `pti_pageins` at 56. Both offsets were checked against the SDK header.

Found by 0096, whose validation against `ps` showed the virtual size was also missing for the same wrong reason.

## Proposal

Read both from the same buffer. Turn them into rates the way the Linux backend does, against the previous sample's counters per `(pid, started)`. `pti_pageins` is the nearer equivalent of a major fault. `pti_faults` counts every fault, so minor = faults − pageins, saturating.

## Acceptance criteria

- [ ] Both columns filled on macOS for the processes the user owns, and an em dash for the rest
- [ ] A test holds our own process's counts against `ps -o majflt,minflt` or `top -stats faults`
- [ ] The README's "Not on macOS" note is corrected
