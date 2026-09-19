---
id: 106
title: Nearly every process reads as running on macOS
type: bug
status: backlog
milestone: r5
labels:
- ui
created: 2026-09-19
updated: 2026-09-19
priority: p1
effort: m
area: darwin
---

## What happens

On macOS the `S` column shows `R` for about 20 of every 23 rows. At the same moment, `ps -A -o state` counted 730 processes sleeping and 5 running. Examples: `rsst` (86077), `herdr` (38470) and an agent process (20814) all show `R` in poptop and `S+` in `ps`.

It looks like `sysinfo`'s process status, which on macOS reports the BSD `p_stat`, is being passed straight through. `p_stat` is `SRUN` for most processes that are actually sleeping; the real state lives in their threads. The column is misinformation on this platform. The `state = R` filter and the RUN count in the header may depend on the same value.

## What should happen

Derive the state from what `ps` uses: the task's thread states via `proc_pidinfo`'s `PROC_PIDTASKINFO` / `PROC_PIDTHREADINFO`. 0103 notes the collector already reads `proc_taskinfo`. Failing that, show `—` rather than claim a state.

## Reproduction

1. Run poptop on macOS and read the `S` column.
2. Compare with `ps -o pid,state -p <pids>`.
