---
id: 48
title: Every node process looks the same, because the table shows comm
type: bug
status: done
milestone: v1.0
created: 2026-09-07
updated: 2026-09-07
priority: p1
area: collect
---

## Problem

The `COMMAND` column shows `comm` — the name from `/proc/<pid>/stat`, which the
kernel truncates to fifteen characters, or sysinfo's equivalent on macOS. It is
not the command line.

For anything running under an interpreter or a runtime that is nearly useless:

```text
PID     CPU%   COMMAND
4821    31.2   node
5102    12.7   node
5533     9.4   node
7781     4.1   python3
```

Four rows, no way to tell which is the API server, which is the bundler, and
which is the thing that should not be running at all. The information is in
`/proc/<pid>/cmdline`, one file away, and poptop does not read it.

htop shows the command line by default and has `p` to toggle full paths and `m`
to merge `exe`/`comm`/`cmdline`. bottom has `P` for the full command. poptop has
no toggle because it has nothing to toggle to.

## Why it matters more here than elsewhere

The identity column was measured at nineteen columns in
[0044](0044-a-row-s-identity-is-split-across-both-ends-of-it.md), and
[0042](0042-every-number-in-the-process-table-is-left-aligned.md) and
[0035] elide it to keep both ends. All of that work is spent making an
identifier readable that, for a whole class of processes, does not identify
anything.

## What needs deciding

- **Cost.** One file read per process per sample, on a path that reads one
  `stat` and one `fstat` today. `cmdline` is small but it is a third syscall
  against a 1.41ms budget, so it wants measuring and probably gating the way
  per-process IO is.
- **Which to show.** The full command line is often enormous — a Java service
  can run to kilobytes. htop's answer is a toggle; the default probably wants
  to be "the argv[0] basename plus enough of the rest to distinguish", which is
  a heuristic and needs stating.
- **A kernel thread has no cmdline** — the file is empty, and the name in
  brackets is the only identity there is. That is a `None`, not a blank.

## Acceptance criteria

- [x] Processes distinguished only by their arguments are distinguishable
- [x] Measured: the cost per sample, against the current baseline
- [x] An empty `cmdline` falls back to the name rather than showing nothing
- [x] The choice of how much to show is written down with its reasoning
