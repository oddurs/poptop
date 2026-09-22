---
id: 229
title: Collection runs on the thread that reads keys
type: bug
status: done
milestone: r9
labels:
- collect
created: 2026-09-22
updated: 2026-09-22
priority: p0
---

## What happens

The interactive loop polls for a key until the next sample is due, then calls
the collector inline. For as long as the collector runs, keys queue unread and
the frame on screen is the last one drawn. On a machine with a few thousand
processes that is already a visible stall once a second; every source this
milestone adds lengthens it — temperatures alone measured 40ms on macOS.

## What should happen

A sampler thread owns the collector and the schedule, and hands finished
samples to the interface over a channel. The interface wakes on either a key
or a sample, so neither waits for the other. The budget still sees what a
sample cost, and `Needs` still reaches the collector — sent the other way, so
a key that opens a panel changes what the *next* sample gathers.

## Acceptance criteria

- [x] No collector call on the thread that draws
- [x] A key is answered while a slow sample is being taken (a test with a collector that sleeps)
- [x] The budget and the logging path see the same samples and costs as before
- [x] Quitting does not wait for a sample in flight to finish
