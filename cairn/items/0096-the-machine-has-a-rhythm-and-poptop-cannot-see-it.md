---
id: 96
title: The machine has a rhythm and poptop cannot see it
type: feature
status: backlog
milestone: v4.0
depends_on:
- 84
created: 2026-09-13
updated: 2026-09-13
priority: p0
area: ui
---

## Problem

A spike at 03:17 is either the backup or an incident, and which one it is decides
whether anybody gets woken. poptop shows the same red figure either way.

The information needed to tell them apart is already on disk. A week of logs
contains the fact that there is a spike at 03:17 **every day**, and that its
shape is the same each time, and that today's is forty percent bigger than usual.

## What it does

Find events that repeat, name them by the process responsible, and state the
period:

```text
recurring on this machine, from 7 days of log

  03:17 daily    ~40s   cpu to 62%    mandb              7/7 days
  04:00 Sun      ~6m    io to 91%     logrotate          1/1 weeks
  every 5m       ~2s    cpu to 30%    prometheus-node…   1974 times
  14:02 weekdays ~3m    mem +2.1G     deploy.sh          5/5 weekdays

today at 03:17 — cpu to 89%, which is 1.4× the usual peak
```

The last line is the point. Everything above it is context a new operator does
not have; the last line is the judgement, and it is only possible because the
lines above it exist.

## Why poptop and nowhere else

Recurrence detection over *metrics* is ordinary — every time-series database can
do seasonality. Recurrence detection that can say **which process** caused each
occurrence, and whether it is the same process each time, needs the process table
at each occurrence. Prometheus and netdata do not keep it. atop keeps per-interval
records, which blur a forty-second event at its default ten-minute interval.

An event that is "the same" because `mandb` ran each time is a different claim
from an event that is "the same" because CPU rose — and only the first is useful.

## How, without becoming a research project

Folding by candidate period and scoring agreement is enough: try daily, hourly,
weekly and the small fixed periods, fold the log onto each, and keep the ones
where occurrences land in a tight phase band with a consistent responsible
process. No spectral analysis, no model.

The honest failure is over-claiming from thin data. Three occurrences is not a
pattern, and the panel must say `3 seen` rather than `daily` until it has earned
the word.

## What needs deciding

- **What counts as an event.** A threshold crossing is the obvious definition and
  a poor one — it makes the answer depend on `warn`. A local peak that stands
  above its own neighbourhood is better and has no knob.
- **Same-event identity.** Phase, duration, magnitude, and the responsible
  process. Requiring all four is too strict — a backup that grows will drift in
  duration and magnitude. Phase and process are probably the pair that matter.
- **Where it lives.** It is not a panel; it is a report. `--report` gains a
  recurrence section, and the timeline can mark known recurrences so an operator
  sees "this is the 03:17 one" without asking.
- **Refusal.** With three days of log, "daily" is a guess. The rule from 0084
  applies unchanged: say how much history there is and decline to characterise.

## Acceptance criteria

- [ ] Repeating events are found from the log, with their period and phase
- [ ] Each names the process responsible, and whether it is the same one each time
- [ ] Today's occurrence is compared against the usual one
- [ ] An event seen too few times is counted, not characterised
- [ ] Nothing depends on `warn` or any other threshold the user set
- [ ] The cost of the scan is measured against a full retention window
