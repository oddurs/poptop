---
id: 103
title: macOS fault counts are in proc_taskinfo, which poptop already reads
type: feature
status: done
milestone: later
assignee: Oddur Sigurdsson
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

- [x] Both columns filled on macOS for the processes the user owns, and an em dash for the rest
- [x] A test holds our own process's counts against `ps -o majflt,minflt` or `top -stats faults`
- [x] The README's "Not on macOS" note is corrected

## How it was resolved

Both counters come from the `proc_taskinfo` buffer poptop already reads for every process it owns: `pti_faults` at 52 and `pti_pageins` at 56, checked at compile time against the offsets either side of them. Minor faults are all faults less the ones that went to disk; major faults are the page-ins.

They are reported as rates over the interval, like the `/proc` backend's, with the previous sample's counters kept per `(pid, started)` so a recycled pid cannot inherit a dead process's totals and read as a storm of faults. A process on its first sighting reads zero rather than its whole life divided by one interval. A process the kernel will not describe to this user keeps its em dash.

`ps -o majflt,minflt` prints `-` on macOS, so the cross-check is against the item's other option, `top -stats faults`: `the_fault_columns_are_rates_and_agree_with_top` touches 64 MiB a page at a time and holds poptop's rate over the window against what `top` counted across it. Live, `MAJF/s` now reads 0 on an idle machine where it read `—`.

The README's claim that sysinfo publishes no fault counts is corrected: true of sysinfo, and beside the point.
