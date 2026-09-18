---
id: 96
title: No number poptop shows has been checked against another tool at the same moment
type: chore
status: backlog
milestone: r4
labels:
- validation
created: 2026-09-18
updated: 2026-09-18
priority: p1
effort: l
area: collect
---

## Problem

The tests check parsing and arithmetic against expected values written by the same author, from the same understanding of the counters. 0023, 0024 and 0027 were all cases where poptop was consistent with itself and wrong about the machine. The only protection against the next one is a comparison with an independent reader.

## Proposal

Write a validation harness, a script rather than a unit test, that runs a known load (CPU burn, memory allocation, disk writes, network transfer) and samples poptop alongside `ps`, `vmstat`, `iostat`, `/usr/bin/time` and `atop` on Linux, and `ps`, `vm_stat`, `iostat` and `top -l` on macOS. It then reports every metric that falls outside a stated tolerance. Run it before each release and record the results.

## Acceptance criteria

- [ ] A harness in the repo, runnable with one command on each platform
- [ ] Tolerances stated per metric, with the reason for each
- [ ] A first run on Linux and macOS, with every disagreement fixed or filed
- [ ] The release checklist includes running it
