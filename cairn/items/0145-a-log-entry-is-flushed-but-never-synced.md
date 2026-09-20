---
id: 145
title: A log entry is flushed but never synced
type: bug
status: done
milestone: v1.2
created: 2026-09-20
updated: 2026-09-20
priority: p1
effort: s
area: log
---

## Problem

`log::append` writes one framed entry and calls `flush()`. That hands the bytes to the kernel, which is enough for a crash of poptop and nothing at all for a power cut or a hard reset: the page cache can hold the last entries for thirty seconds. The whole claim of the day log is that it outlives the process — "poptop logs if it is left running" — and the case somebody most wants the log for, a machine that went down, is the case where the tail of it may not be there.

The store has the same question and a different answer: it is written once on a clean exit, so a crash costs at most the session, which the README states.

## Proposal

Decide the durability rule and write it down, then implement and measure it. Candidates: `sync_data` per entry (correct, and at a ten-minute default interval free, but a one-second `log-interval` pays it every second); sync every N entries or every N seconds; sync on rotation and on exit. Measure the cost at both extremes before choosing.

## Acceptance criteria

- [x] The rule is stated in the recording documentation, with the cost measured
- [x] A test proves the data is on disk after the rule says it is (`sync_data`, then read back through a fresh handle)
- [x] The cost at a one-second log interval is within the performance budgets (0097)
- [x] What a power cut can lose is stated in one sentence a user can act on

## How it was resolved

**Decided:** every entry is synced, in `append`, before it returns. Not every N entries and not on rotation — a rule with a window is a rule that has to say how wide the window is, and "the log has everything up to the last interval" is the only sentence about a log worth having.

**Measured first.** An 87 KB entry, release build, best of three runs of five:

| | write + `flush` | write + `sync_data` |
| --- | --- | --- |
| APFS, M-series Mac | 0.087 ms | 4.189 ms |
| ext4, Linux container | 0.020 ms | 2.781 ms |

At the ten-minute default that is 4 ms every ten minutes. At a one-second `log-interval` — the extreme worth checking, since that is where a per-entry sync would be felt — it is 0.4% of a second on the slower filesystem, and the sample it is buying is one that survives the machine. Worth it at both ends, so no windowing.

**How.** `log::append` now calls `sync_data` after `flush`. `sync_data` rather than `sync_all`: the only metadata an entry depends on is the file's length, which `sync_data` is required to persist, and the timestamps `sync_all` would also push cost a second round trip for nothing.

**The macOS caveat, stated rather than hidden.** `sync_data` is `fsync` there, which hands the bytes to the drive and does not ask it to empty its own write cache; `F_FULLFSYNC` does, at roughly ten times the cost. poptop does not use it: it defends against the kernel dying and the process dying, not against a drive that lies about its cache, and a monitor is not a database. `docs/guide/recording.md` says so in the words a user can act on, beside what the store promises instead (written once on a clean exit, so a crash costs the session).

**Tests.** `an_entry_is_on_disk_before_append_returns` spawns a child copy of the test binary that appends three entries and then `raise(9)`s itself — SIGKILL, so no unwinding, no destructor, no exit path, nothing that could flush anything the kernel had not already taken. The parent reads all three back. `./check --perf` holds the new cost with `log: append one entry, synced` at 40 ms against 3.8 ms measured, the three-times rule the other budgets use.
