---
id: 146
title: The buffer can be addressed but not searched
type: feature
status: backlog
milestone: v5.2
depends_on:
- 73
created: 2026-09-13
updated: 2026-09-13
priority: p1
area: ui
---

## Problem

Six hundred samples are retained. The ways through them are: one arrow key at a
time, ten with shift, a jump to a timestamp you already know, and a report over
the whole period.

None of those answers "when did this happen". The filter language already
answers "which processes match this" at one instant —

```text
state = D and write > 1mb
```

— and evaluating the same expression across the buffer instead of within one
sample turns it into a search over time. That is a different tool, and it is one
no other monitor in this category can have, because none of them has a buffer to
search.

## What it looks like

The filter box with a modifier, or a second prompt:

```text
find: cpu > 80 and name ~ cc1plus
      4 matches — 03:02:11, 03:04:40, 03:11:02, 03:14:18   n / N to step
```

The timeline marks every match, so the distribution is visible before you visit
any of them: three in a burst and one an hour later is a different story from
four evenly spaced.

## What needs deciding

- **Process match or machine match.** `cpu > 80` could mean the machine's CPU or
  any process's. Both are wanted and they are different searches. The existing
  language is about processes; machine figures need a name that cannot be
  confused with them — `sys.cpu`, or a separate prompt.
- **Duration.** "Above 80 for thirty seconds" is the query people actually mean,
  and it is the one that separates a spike from a problem. `--report` already
  distinguishes peak from sustained; the search language should be able to say
  the same thing rather than making the reader eyeball it.
- **Cost.** Six hundred samples times four hundred processes is 240,000 rows per
  search. That is fine once; it is not fine on every keystroke, so this cannot
  be as-you-type the way the filter is. Enter to run.
- **What a match *is*.** A sample where the expression held, or a run of them?
  A run, collapsed — four hundred consecutive matching samples are one event and
  listing them all is not a result anybody can use.

## Acceptance criteria

- [ ] An expression can be searched across the buffer, not only applied at one moment
- [ ] Matches are marked on the timeline, so their distribution is visible
- [ ] Consecutive matches collapse into one event with a duration
- [ ] Machine figures and process figures are both searchable, and distinguishable
- [ ] A search that matches nothing says so rather than leaving the cursor put
- [ ] Search cost measured at a full buffer and a realistic process count
