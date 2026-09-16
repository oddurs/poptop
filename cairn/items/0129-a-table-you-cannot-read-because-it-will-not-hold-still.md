---
id: 129
title: A table you cannot read because it will not hold still
type: feature
status: done
milestone: v4.0
created: 2026-09-16
updated: 2026-09-16
priority: p1
area: ui
---

## Problem

At one sample a second the process table is mostly noise. A row's CPU figure
swings from 3 to 40 and back, and — far worse — the rows swap places while your
eye is on them. Measured on a real machine: the order of the top rows changed in
**fifteen of fifteen** consecutive frames.

Activity Monitor is restful, and the reason is not subtle: it refreshes every
five seconds. Between refreshes nothing moves at all.

## What shipped

Two halves, and only together do they work.

**The figures are averaged** over the last five seconds by default —
`--smooth=SPAN`, or `smooth = off`. poptop keeps every second and averages what
it *shows*, which is Activity Monitor's calm with none of its delay: a spike
still happens at the second it happened, and the timeline still draws it.

**The ordering is over the average, and the average ends on a boundary.** This
is the half that matters and the half that was not obvious. Averaging alone took
the measurement from fifteen frames in fifteen to twelve — better, and nowhere
near calm, because two processes with close averages still cross every second.
Ending the window on a multiple of itself means nothing in the table can change
its mind between one boundary and the next. Same measurement: **three in
fifteen**, which is once per window, which is the point.

## What it deliberately does not do

- **The rows are not quantised, only the averages.** Doing both was tried: a
  process that had just started was not listed for five seconds, and "what is
  running now" is the question the table exists to answer.
- **Scrubbing is not quantised.** The reader is asking about a particular
  moment, and rounding the answer to a boundary shows them a different one.
- **The timeline is not smoothed at all.** Peak there, mean here. That looks
  like a contradiction and is the opposite: the timeline is where a spike must
  be *found*, so averaging it away would be a lie — and the table is a thing you
  read, with the graph directly above it. Each aggregation matches the question
  its panel answers.
- **Absences are skipped rather than counted as zero.** A process present for
  two of five samples was not idle for the other three.

## Acceptance criteria

- [x] The rows stop swapping places while being read
- [x] The figure on a row is the one its position was decided by
- [x] The table says it is averaging, and over how long
- [x] A process that has just started is listed at once
- [x] Scrubbing answers about the moment, not the boundary
- [x] The timeline keeps every sample
