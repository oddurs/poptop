---
id: 150
title: SIGHUP means quit, where every other logger reopens
type: bug
status: done
milestone: v1.2
created: 2026-09-20
updated: 2026-09-20
priority: p3
effort: s
area: log
---

## Problem

r3 made SIGTERM and SIGHUP quit cleanly, which is right for a monitor on a terminal that has gone away. For the logging half it is the opposite of the convention: SIGHUP is how `logrotate` and every daemon under it says "I have moved your file, open it again". A poptop left running as a recorder, with its logs rotated by the system, exits instead — and the recorder that was meant to outlive the session dies at 03:00 on the night the rotation runs.

## Proposal

Tell the two cases apart. On a terminal, SIGHUP is the terminal going: quit, as now. Started without one — `--export --follow`, or a future recorder mode — SIGHUP reopens the log file by name and carries on. `poptop --once --log=on` from cron never sees the signal either way.

## Acceptance criteria

- [x] With a terminal, SIGHUP still quits cleanly (r3's test keeps passing)
- [x] Without one, SIGHUP reopens the day file by name and logging continues
- [x] A test renames the file under a running poptop, signals it, and finds the new file written to
- [x] The behaviour is in the recording documentation, beside the retention rule

## How it was resolved

**Told apart by the terminal, not by a flag.** SIGHUP means two opposite things, and nobody types `--i-am-a-daemon`: on a terminal it is the terminal going away and poptop quits, giving the terminal back; without one it is what `logrotate` and every daemon under it means by it. `signals()` registers SIGHUP against the stop flag when stdin or stdout is a terminal and against a reopen flag otherwise. SIGTERM always stops. The monitor is unchanged — it needs a terminal on both, so r3's test still finds it exiting 0 on a hangup.

**What reopening means, per feed.** A follower of a day (`--export DATE --follow`) finds the file again by name and carries on from the sample it last handed out — `Follower::reopen` resets the offset and the file identity and leans on the same "do not hand out what has already gone out" machinery a trim to the byte budget needed (0146). So a rotation costs no records and repeats none. A live feed of the machine has nothing to reopen: its records go to stdout, which poptop does not own, and it says exactly that, once, rather than leaving the operator to wonder what their rotation did.

**The writer never needed a signal.** Every `append` opens the day by name, so a file moved out from under a running poptop costs nothing — the next entry creates the name again and what was moved is whole. That is now a test rather than an accident, and it is why the hangup is free to mean "reopen" to the readers.

**Tests.** The rotation under a running follower: the file is renamed away, the feed is signalled, a new sample is logged, and the record that arrives is a later one from the new file with the process still running; the live feed carrying on and saying there is nothing to reopen; the writer's rotation in `src/log.rs`; and the monitor's existing hangup test, unchanged. The documentation says all three cases beside the retention rule.
