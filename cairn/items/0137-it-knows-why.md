---
id: 137
key: v5.1
title: It knows why
type: milestone
status: backlog
created: 2026-09-13
updated: 2026-09-13
priority: p2
due: 2028-04-01
---

The layer under the one above. poptop can name the process; it cannot say what
that process is waiting on, or where it is spending its time.

This is the territory of `bcc`, `bpftrace`, `perf` and `py-spy` — a hundred and
fifty tools, each answering one question, none of them installed on the box
during an incident and several of them needing a kernel you do not have. The
opportunity is not to reimplement them. It is that two of the highest-value
answers they give are reachable from `/proc` alone, and nobody joins them up:
what a blocked process is blocked *on*, and which file that fd points at.

Where an answer genuinely needs more than reading files — stack sampling,
tracepoints — the reading rule from v2.1 decides it, and the result is opt-in
the way signals are.
