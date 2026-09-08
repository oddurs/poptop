---
id: 72
title: History ends when the process does
type: feature
status: backlog
milestone: v2.2
depends_on:
- 59
created: 2026-09-08
updated: 2026-09-08
priority: p0
area: store
---

## Problem

poptop's store persists a ring buffer across restarts. atop keeps
`/var/log/atop/atop_YYYYMMDD`, rotated daily, twenty-eight generations by
default, and you can open any of them.

The gap is not "more history". It is that atop's history **outlives the process
that recorded it and is addressable by date**. "What happened at 03:00 last
Tuesday" is the question a monitor is asked after the incident, and it is the
one poptop's positioning explicitly concedes.

## What poptop must not give up doing

The zero-setup position is the reason poptop exists: it gives you the last ten
minutes on a box you just connected to, with nothing having been running
beforehand. atop gives you twenty-eight days *if its daemon was running* and
nothing at all if it was not.

These are not in conflict. The rule is that **poptop logs if it is left
running, and works if it was not** — the in-session buffer is unchanged and the
log is what accumulates when the tool happens to have been up. That is strictly
more than atop offers, and it costs the positioning nothing.

## What needs deciding

- **Rotation and retention.** Daily files and a generation count is what atop
  does and what operators expect. Retention has to be a bound on *bytes* as well
  as days, because poptop's sample carries a full process table and atop's does
  too — measure before choosing a default.
- **What is written at what rate.** A one-second in-session buffer and a
  ten-minute log are different products. atop's default interval is ten minutes
  for logging and ten seconds interactive.
- **Where.** `/var/log` needs privileges; `$XDG_STATE_HOME` does not and is
  per-user. atop is a system service and poptop is not.
- **Whether it is a mode or always.** Writing a log by default without being
  asked is a surprise on someone's laptop.

## Depends on

The store format that survives an upgrade — a log nobody can read after
upgrading is worse than no log.

## Acceptance criteria

- [ ] History survives the process, addressable by date
- [ ] Rotation and retention bounded by both age and bytes, measured
- [ ] Nothing is written without the user having asked, once
- [ ] A box that has never run poptop still gets its in-session buffer
- [ ] Measured: bytes a day at 400 processes, at each interval offered
