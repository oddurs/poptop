---
id: 150
title: SIGHUP means quit, where every other logger reopens
type: bug
status: backlog
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

- [ ] With a terminal, SIGHUP still quits cleanly (r3's test keeps passing)
- [ ] Without one, SIGHUP reopens the day file by name and logging continues
- [ ] A test renames the file under a running poptop, signals it, and finds the new file written to
- [ ] The behaviour is in the recording documentation, beside the retention rule
