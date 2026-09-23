---
id: 170
title: A finding cannot leave the terminal it was found in
type: feature
status: backlog
milestone: v5.0
depends_on:
- 72
- 74
created: 2026-09-13
updated: 2026-09-13
priority: p2
area: store
---

## Problem

Somebody finds the moment their machine went wrong. To show anyone else, they
take a screenshot.

The log is a day of samples in poptop's own format — the right thing to send is
the window around the moment, and there is no way to cut one out. `--export`
emits the whole day or the live sample; neither is "the ninety seconds either
side of 03:04, with the process tables".

## Why this matters more for poptop than for the others

For htop there is nothing to send: the screen *is* the state. For atop the
answer is "send me /var/log/atop/atop_20260913", which is the whole day and
needs atop at the other end anyway.

poptop's unit is a window of complete samples, and it is small: at the measured
87 KB an entry, ninety seconds at the logging interval is a few hundred
kilobytes. Small enough to attach to a ticket, complete enough that the person
at the other end can scrub it, filter it, explain it and report on it with the
same keys.

That is the support workflow: **"send me your poptop capture"**.

## What needs deciding

- **What a capture contains.** The window, obviously. Also the version, the
  schema, the platform, the interval, and which optional sources were on — a
  capture that cannot say what was *not* collected will be misread as a machine
  that had no cgroups.
- **How the window is chosen.** Around the cursor, around a `--report` finding,
  or a time range. All three are one flag with three spellings.
- **What must never be in it.** Command lines carry secrets: tokens in argv,
  paths under a home directory, database URLs. A capture is a thing people send
  to strangers. There has to be a redaction pass, it has to be the default or
  very loudly optional, and what it redacted has to be visible in the capture
  rather than silent — a blanked argument the reader cannot see is a capture
  that lies about what was running.
- **Whether it is the log format or a new one.** The log format, unless that
  turns out to be impossible. A second format is a second reader and a second
  thing to keep in step.

## Acceptance criteria

- [ ] A window around any moment can be written to one file
- [ ] That file opens with `--read` and scrubs like any other history
- [ ] It records what was not collected as well as what was
- [ ] Command lines are redacted by default, and the redaction is visible
- [ ] Its size is measured at a realistic process count, and stated
