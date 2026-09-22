---
id: 230
title: Samples land wherever poptop happened to start
type: feature
status: done
milestone: r9
labels:
- collect
depends_on:
- 229
created: 2026-09-22
updated: 2026-09-22
priority: p1
---

## Problem

The schedule starts from the instant the process did, so a sample at
11:29:52.618 is followed by one at 11:29:53.618. Two poptops on the same
machine never sample the same moment, a day and the one after it are offset by
however long startup took, and the clock in the header reads a second that is
already six tenths over.

## Proposal

Tick on wall-clock multiples of the interval: at a one-second interval, on the
second. The schedule stays monotonic — a wall clock stepped back must not
produce a burst or a stall — so the phase is taken from the wall clock once and
then carried on the monotonic clock, re-read only when the two disagree by
more than an interval.

## Acceptance criteria

- [x] At a one-second interval every sample's `at` is within a few milliseconds after a whole second
- [x] Intervals that do not divide a minute still tick evenly
- [x] A wall clock stepped by an hour neither stalls sampling nor bursts it
