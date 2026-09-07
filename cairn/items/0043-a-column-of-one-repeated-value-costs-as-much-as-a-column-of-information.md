---
id: 43
title: A column of one repeated value costs as much as a column of information
type: feature
status: done
milestone: v1.0
created: 2026-09-07
updated: 2026-09-07
priority: p2
area: ui
---

## Measured

Across the twelve rows visible in a default 120x26 frame:

| column | width | distinct values |
|---|---|---|
| `USER` | 10 | **1** (`oddurs`) |
| `DISK R` | 9 | 3 (ten of them `0`) |
| `DISK W` | 9 | 1 (`0`) |
| `CPU%` | 6 | 12 |
| `COMMAND` | 19 | 12 |

`USER` costs ten columns — more than `CPU%` — to repeat one word twelve times.
`DISK W` costs nine to repeat `0`. Meanwhile `COMMAND`, which is different on
every row and is how a reader identifies what they are looking at, takes
whatever is left and has to elide.

This is the single-user laptop case, which is most of the time for most people.
On a shared box `USER` earns its width immediately.

## The rule

poptop already filters *rows* by measurement rather than by name: a device
appears once it has completed an operation, an interface once it has carried a
byte, a filesystem once it reports blocks. The same test applies to *columns*.

**A column whose visible values are all identical is telling you one fact, and a
fact belongs in a sentence rather than a column.** Fold it into the section
title — `processes (783) · all oddurs` — and give its ten columns to the one
that runs out.

## What needs deciding

- The rule has to be about what is *visible*, not what exists, or scrolling
  changes the layout under the reader. Deciding once per frame from the rows
  actually drawn is stable enough; deciding from the whole table is stabler
  still and nearly always the same answer.
- Which columns are eligible. `USER` and the two disk rates are the candidates;
  `S` at two columns is not worth the machinery, and `PID` is never constant.
- A column that comes and goes as processes churn would be worse than the waste.
  A hysteresis — collapse only when it has been constant for several samples —
  may be needed, and if so that is a reason to scope this carefully rather than
  a reason to skip it.

## Acceptance criteria

- [x] A column with one distinct value across the visible rows yields its width
- [x] What it said appears once, in the section title
- [x] The layout does not oscillate as processes come and go
- [x] Measured: columns returned to `COMMAND` on a single-user machine
