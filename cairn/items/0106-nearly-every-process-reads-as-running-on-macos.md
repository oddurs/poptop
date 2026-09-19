---
id: 106
title: Nearly every process reads as running on macOS
type: bug
status: done
milestone: r5
assignee: Oddur Sigurdsson
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

## How it was resolved

sysinfo's status on macOS is the BSD `p_stat`, which is `SRUN` for nearly every process, running or asleep; the real state is in the threads. The answer was already inside a call poptop makes for every process it owns: `proc_taskinfo`, read for the thread count, also carries `pti_numrunning`, the number of threads runnable right now. The state now comes from the threads alone:

- `R` if any thread is runnable, as `ps` decides it.
- `S` if none is.
- `?` for a process whose task info the kernel won't give this user.

Stopped and zombie still come from the process table.

**sysinfo's status can't be used even as a gate.** The first version kept sysinfo's `Run` and refined only that case. The CI runner's macOS showed why that was wrong: there, sysinfo reported `Sleep` for a process spinning flat out, whose task said one thread was runnable (`status Some(Sleep), task Some(Task { threads: 2, …, running: 1 })`, twenty samples in a row). So sysinfo's status means `p_stat` on one release and something else on another, while the runnable-thread count is right on both.

The offset (88, straight after `pti_threadnum`) is checked at compile time against the structure's size, and the value is rejected unless it lies between 0 and the thread count. There are no new system calls.

Live on this Mac: the top 29 rows went from about 20 `R` to 2 `R` and 27 `S`, in line with `ps` (5 R of 755). `a_sleeping_process_is_not_reported_as_running` collects a real `sleep` child and a real `yes` through the real collector over up to twenty samples. The sleeper must never read as running, and the spinner must read as running at least once, which is the claim that holds on a loaded three-core runner. If it fails, the message carries each sample's sysinfo status, task info and CPU, which is how the CI case above was diagnosed.

Linux is unchanged: `/proc/<pid>/stat` states were already right.
