---
id: 90
title: A jump to 02:30 on the night the clocks change has no defined answer
type: bug
status: done
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

- [x] Tests with `TZ=Europe/London` and `TZ=America/New_York` for both transitions
- [x] A nonexistent or ambiguous time produces a visible note naming which instant was chosen
- [x] Day-file boundaries (`log::date_of`) are checked on a 23-hour and a 25-hour day

## How it was resolved

**Decided:** a time the clocks skipped moves forward by the length of the gap, to the instant it would have been had they not changed. `01:30` on London's spring night becomes 02:30 BST. A time they showed twice takes the first occurrence, because a reader scrolling forward through the night reaches it first. Both say so in the jump note, beside whatever the landing adds:

- `01:30 did not happen on 2026-03-29 — the clocks skipped it; this is 02:30:00`
- `01:30 happened twice on 2026-10-25 — this is the first; the second is 1h later`

**How.** `at_local` used to ask `mktime` once with `tm_isdst = -1` and take whatever the C library chose, which differs between platforms, and it said nothing. Now it asks three ways (summer time in force, not in force, unknown) and keeps each answer only if `localtime_r` reads it back as the wall time asked for. One survivor is an ordinary time. Two are a time shown twice. None is a skipped time, and the latest answer is the one the clock would have reached. This does not depend on a zone's rules or on the platform's choice for `-1`. `24:00` and the leap second are resolved as the second before them and then moved on, as before.

**Tests.** `TZ` cannot be changed under tests that run in parallel, so `the_nights_the_clocks_change_in_london` and `…_in_new_york` each run a child copy of the test binary with `TZ` set. They use `Europe/London` and `America/New_York`, and the same rules as POSIX strings (`GMT0BST,M3.5.0/1,M10.5.0`, `EST5EDT,M3.2.0,M11.1.0`), which need no zone files. In each zone:

- the skipped time is noted, moves forward, and is exactly an hour after the time an hour before it;
- the repeated time is noted, is the first, and the same clock reading is an hour later;
- an ordinary day says nothing;
- the spring day is 23 hours from midnight to midnight and the autumn day 25, and `date_of` puts each day's last second in that day and the next second in the next day.

They pass on macOS and glibc. Changing the skipped case to take the earlier instant makes them fail.

In London the spring gap is 01:00–02:00, not 02:30 as the title has it: `02:30` does not exist in New York, and `01:30` does not exist in London.

