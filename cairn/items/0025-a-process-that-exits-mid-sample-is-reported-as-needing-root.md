---
id: 25
title: A process that exits mid-sample is reported as needing root
type: bug
status: backlog
milestone: v0.2
created: 2026-09-06
updated: 2026-09-06
priority: p1
effort: s
area: collect
---

## What happens

`read_proc_io` collapses every failure into one:

    let text = read_into(path, buf).map_err(|_| ())?;

and the caller counts that as `io_denied`. But the two failures it can hit mean
opposite things, and the kernel already tells them apart:

    /proc/999999/io: NotFound          -- the process exited
    /proc/1/io:      PermissionDenied  -- needs CAP_SYS_PTRACE

A process that exits between the directory listing and the read is normal and
needs nothing from anyone. poptop reports it as one more process that "needs
root", in a count it prints in the panel title.

Two consequences beyond the wrong wording:

- On a box with heavy process churn — a build server, which is exactly where
  someone reaches for this — the count is inflated by processes that were never
  unreadable.
- Item 0022 turned that count into a decision: more than half denied and the IO
  columns withdraw themselves. Transient exits now feed a threshold.

## What should happen

Count `PermissionDenied` as denied. Treat `NotFound` as what it is: the process
is gone, there is nothing to read and nothing to report.

This is the same distinction the codebase already makes elsewhere — an em dash
for "cannot know" against a figure for "know it is zero" — applied to the
reason rather than the value.

## Acceptance criteria

- [ ] `io_denied` counts only permission failures
- [ ] A process that exits mid-sample is not counted as anything
- [ ] The probe in 0022 decides on the corrected count
- [ ] A test covers both error kinds
