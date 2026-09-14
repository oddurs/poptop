---
id: 99
title: Nothing says what you would get back
type: feature
status: backlog
milestone: v4.1
depends_on:
- 82
created: 2026-09-13
updated: 2026-09-13
priority: p1
area: ui
---

## Problem

The decision at the end of every incident is "what do I stop". poptop shows a
table sorted by CPU and leaves the arithmetic to somebody who is under pressure
and reading a screen.

The arithmetic is not hard. It is also not trivial, and it is exactly the kind of
thing that gets done wrong at 3am: summing the CPU of fourteen rows is easy to
get wrong, and summing their RSS gives an answer that is **wrong in a specific
direction** because forked workers share pages.

## What it does

On a selection — a row, a group, a service, a filter's whole result — say what
stopping it would return:

```text
  stop 14 × cc1plus (make -j16)
    cpu     71.2%  of 1400%      returns 10.0 of 14 cores
    memory   5.6G  resident      but 4.1G is shared: expect ~1.5G back
    disk     22 MB/s write
    since   03:02:11 (4m29s)     parent 8841 make -j16 would remain
```

The memory line is the one worth building the feature for. Every other tool shows
RSS and lets you add it up; the sum is an over-estimate whenever processes share
pages, which for a build, a pre-forking server or anything with a big interpreter
heap is always. poptop collects PSS in the memory view already — this is what PSS
was *for*, and it has never been used to answer the question it answers.

## Why it is honest rather than clever

Everything above is a measurement poptop holds, restated. Nothing is predicted.
The one inference — "expect ~1.5G back" — is PSS summed instead of RSS, and it
says which number it used and why the other one is bigger.

The `parent … would remain` line matters for the same reason: killing fourteen
compilers under a `make` gets you fourteen more, and a tool that says "this frees
10 cores" without mentioning that is giving advice rather than evidence.

## What needs deciding

- **Whether it suggests, or only states.** Stating is safe. Ranking — "the three
  things most worth stopping" — is what people want, and the moment poptop ranks,
  it is advising. Probably: state for a selection the user made; never volunteer
  a target.
- **PSS costs a read per process** and is gated on the memory view. This needs it
  for the selection only, which is cheap, but the gate has to learn a third state.
- **What "returns" means for CPU** on a machine that is not saturated. Stopping a
  process at 71% of fourteen cores on an idle box returns nothing anybody wanted.
  The figure is only meaningful against a constraint, and poptop already knows
  what the constraint is — `S` names it.
- **Its relationship with signals.** This is the sentence that should appear
  *above* the `x` confirmation, not a separate feature. 0076 asks "send TERM to
  postgres?"; it should also say what that gets back.

## Acceptance criteria

- [ ] Any selection can be asked what stopping it would return
- [ ] Memory uses PSS and says so, with the RSS over-estimate named
- [ ] A surviving parent that would respawn the work is named
- [ ] CPU is stated against the machine's capacity, not as a bare percentage
- [ ] poptop states the consequence of a choice and never picks the target
- [ ] The figure appears in the signal confirmation, not only in its own panel
