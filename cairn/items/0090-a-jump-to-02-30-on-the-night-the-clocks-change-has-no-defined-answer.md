---
id: 90
title: A jump to 02:30 on the night the clocks change has no defined answer
type: bug
status: backlog
milestone: r2
labels:
- review
created: 2026-09-18
updated: 2026-09-18
priority: p2
effort: s
area: log
---

## Problem

`log::parse_when` turns typed times into instants through `mktime` (`src/log.rs:240`). On a spring-forward night 02:30 does not exist, and on a fall-back night 01:30 exists twice. `mktime` resolves both cases silently, in a way that depends on the platform and on `tm_isdst`. `b 02:30` could land an hour away from what was meant, or on a sample from the wrong half of a repeated hour, with no message saying so.

## Proposal

Decide what each case should do: a nonexistent time moves forward and says so; an ambiguous time takes the first occurrence and says so, or asks. Test it with `TZ` set to a zone with DST on both platforms.

## Acceptance criteria

- [ ] Tests with `TZ=Europe/London` and `TZ=America/New_York` for both transitions
- [ ] A nonexistent or ambiguous time produces a visible note naming which instant was chosen
- [ ] Day-file boundaries (`log::date_of`) are checked on a 23-hour and a 25-hour day
