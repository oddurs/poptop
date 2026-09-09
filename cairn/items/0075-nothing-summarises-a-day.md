---
id: 75
title: Nothing summarises a day
type: feature
status: done
milestone: v2.2
depends_on:
- 73
created: 2026-09-08
updated: 2026-09-09
priority: p2
area: ui
---

## Problem

`atopsar` reads a logfile and prints reports: CPU over the day, disk over the
day, the busiest processes, at an interval you choose. It is how atop is used
non-interactively — a cron job that mails you what happened.

poptop can show you any instant and cannot tell you about a period. "What was
the worst hour yesterday" is a question the buffer contains the answer to and
the interface cannot express.

## Why the buffer makes this different from atopsar

atopsar summarises samples. poptop retains full process tables, so a summary can
say *which process* was responsible for the worst minute — not just that the
minute was bad. That is a report atop cannot generate from its own logs at
default settings, because its process records are per-interval rather than
per-second.

## What needs deciding

- **What a summary is.** Peak, mean, and time-above-threshold are three
  different questions. The peak-not-mean rule the timeline already follows says
  peak matters most, and a report that only gives peaks hides a sustained
  problem.
- **Whether it is a mode or a subcommand.** A report is not interactive.
- **Whether it says what caused it.** The interesting version does, and that is
  where the retained process tables earn their space.

## Depends on

Replay, since a report is over recorded history rather than the live buffer.

## Acceptance criteria

- [x] A period can be summarised without watching it
- [x] The summary names what was responsible, not only that something was
- [x] Peak and sustained are distinguished, not conflated into a mean
- [x] Runs without a terminal, for cron
