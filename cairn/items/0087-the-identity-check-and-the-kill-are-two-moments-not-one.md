---
id: 87
title: The identity check and the kill are two moments, not one
type: bug
status: backlog
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

- [ ] Linux uses pidfd where available, falling back to the current behaviour with a stated reason
- [ ] A test forks a child, lets it exit, and proves the pidfd path refuses rather than signalling
- [ ] The documented safety claim matches what each platform actually guarantees
