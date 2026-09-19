---
id: 87
title: The identity check and the kill are two moments, not one
type: bug
status: done
milestone: r2
labels:
- review
created: 2026-09-18
updated: 2026-09-18
priority: p1
effort: m
area: signal
---

## Problem

`signal.rs` refuses to signal a process whose `(pid, started)` no longer matches, which is the right rule. But the check and `kill(p.pid, …)` (`src/signal.rs:273`) are separate system calls. If the process exits and its pid is reused in between, the signal reaches the new process. The window is small, and pid reuse is fast on a busy machine with a small `pid_max`. The module doc claims poptop is "the safest place" to send a signal from, so this gap matters more here than it would elsewhere.

## Proposal

On Linux 5.3+, open a pidfd, re-verify identity through it, and send with `pidfd_send_signal`. The identity is then pinned by the file descriptor, not by the pid number. On macOS and older kernels, keep the check, re-check immediately before `kill`, and state the remaining window in the docs and the confirmation prompt instead of implying it is closed.

## Acceptance criteria

- [x] Linux uses pidfd where available, falling back to the current behaviour with a stated reason
- [x] A test forks a child, lets it exit, and proves the pidfd path refuses rather than signalling
- [x] The documented safety claim matches what each platform actually guarantees

## How it was resolved

The gap was wider than the item said. `check` compared the pid against the newest *sample*, which can be a whole interval old: one second by default, a minute at `--interval=60s`. So the window was never only between two system calls. It was everything from that sample to the keypress. `send` now asks the kernel again at the moment of sending, through `collect::start_of`: field 22 of `/proc/<pid>/stat` on Linux, and `sysctl(KERN_PROC_PID)` on macOS with the same layout check as the table. Each is in the unit its platform's samples use.

- **Linux 5.3+:** `pidfd_open`, then the start-time read, then `pidfd_send_signal` on the same descriptor. A process that still has the chosen start time after the pidfd was opened has held that pid the whole time, so the pidfd is that process. A signal through it reaches that process or nobody.
- **Fallback:** `ENOSYS` (an older kernel) or `EPERM` (a seccomp filter that does not know the call) falls through to the re-read and `kill`. `ESRCH` from `pidfd_open` is reported as gone.
- **macOS:** the re-read, then `kill`. The remaining window is two consecutive system calls, and the module notes and the README say that in those words.

`send` returns `Failed::Refused` when the kernel disagrees with the sample, so the note names the reason the way `check`'s refusals do: `pid 4823 is another process now, not postgres — nothing was sent`.

**Tests**, run on real children on both platforms: a chosen process is signalled and dies of `TERM`; a live process whose start time differs is refused and is still running afterwards; a child that was killed and reaped after the sample is refused as gone or recycled. On Linux, `a_pidfd_outlives_its_process_and_signals_nothing_after_it` opens a pidfd on a child, kills and reaps the child, and requires the signal through the pidfd to fail with `ESRCH`. Ran on kernel 7.0 in a container; the syscall numbers 424 and 434 were checked against the headers for x86_64 and aarch64. `start_of` is checked against the start time `parse_proc_stat` gives the same process, including for a name containing `) (`.

