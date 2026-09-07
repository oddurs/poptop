---
id: 22
title: Show disk IO in the default process table
type: feature
status: backlog
milestone: v0.2
created: 2026-09-06
updated: 2026-09-06
priority: p1
effort: s
area: ui
---

## Problem

The per-process disk IO columns are behind the `i` key, so the default table
cannot name the culprit in the failure mode the research says is most common.
A user who has just been told by the header that the machine is blocked on IO
has to know to press a key before the table will say which process is doing it.

htop carries the same complaint upstream: disk IO counters should be visible
without manual configuration on every system, precisely because IO is the
common bottleneck.

## Proposal

Show the IO columns by default wherever they can actually be read, and keep `i`
as the way to hide them.

The ratchet stays exactly as it is. Collection already starts when the columns
are first shown and never stops, so history has one clean boundary between "not
collected" and "collected"; defaulting to shown simply moves that boundary to
the start of the session. Scrubbing into a region collected without IO still
renders an em dash rather than a fabricated zero.

`/proc/<pid>/io` is mode 0400 and owned by the process owner, so reading other
users' processes needs `CAP_SYS_PTRACE`. Where the columns would be almost
entirely em dashes, defaulting them on would be worse than useless — so the
default is conditional on how much is actually readable, and the existing
`io_denied` count is what decides it.

## Acceptance criteria

- [ ] IO columns shown by default where a useful share of processes are readable
- [ ] Not shown by default where they would be mostly em dashes
- [ ] `i` still toggles, and the collection ratchet is unchanged
- [ ] Narrow terminals drop the columns as they already do
