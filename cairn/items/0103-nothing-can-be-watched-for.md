---
id: 103
title: Nothing can be watched for
type: feature
status: backlog
milestone: v5.0
depends_on:
- 102
created: 2026-09-13
updated: 2026-09-13
priority: p1
area: ui
---

## Problem

"Tell me if this happens again" is the last thing anybody says at the end of an
incident, and poptop's answer is to sit and watch the screen.

Every monitoring product answers this with alerting, and alerting is a category
poptop should stay out of — rules, routing, silences, on-call, an evaluation
engine and a delivery pipeline. That is not a thing you add to a terminal tool;
it is a thing you become.

## The version that belongs here

A watch is a query (0102) with a threshold, evaluated **over the buffer and
forward**, and its entire delivery mechanism is the screen:

```text
  watch: mem > 90 for 1m

  ⚑ 03:04:11 → 03:06:40   fired, 2m29s          node (pid 2201)
  ⚑ 14:22:03 → 14:22:51   fired, 48s            node (pid 2201)
    ────────────────────────────────────────────────────────────
    marked on the timeline; b jumps to one
```

The thing that makes this poptop's rather than a bad imitation of Prometheus: the
watch is evaluated **over history the moment you write it**. Every other alerting
system starts counting when you save the rule. poptop already holds the last ten
minutes — or seven days, with the log — so writing the rule tells you
*immediately* whether it would have fired, how often, and on what. A rule you can
test against the past before you trust it is a different object from one you have
to wait for.

## What it deliberately does not do

- **No delivery.** No mail, no webhook, no exit code that a cron job scrapes.
  A watch marks the timeline and puts a line in the panel. Anybody who needs a
  page has a monitoring system, and `--export` is how poptop feeds it.
- **No persistence beyond the session and the config.** Named watches live in the
  config file, where a human can read them.
- **No severity, no routing, no silences.** Those exist because delivery exists.

## What needs deciding

- **Whether even this much is a mistake.** The argument against: it is the first
  feature whose value depends on poptop being left running, and this tool's whole
  position is that it does not need to have been. The argument for is that it
  costs nothing when nobody writes a watch, and that testing a rule against
  retained history is genuinely a thing nothing else can do.
- **What "fired" means across a gap.** A watch cannot have fired during a stretch
  that was not observed, and it must say so rather than reporting a clean run.
  The same rule `--report` follows.
- **Whether a watch can be attached to a moment.** "This, again" — select a
  recurrence from 0096 and watch for it — is a much better gesture than typing an
  expression, and it composes.

## Acceptance criteria

- [ ] A watch is an expression in the same language as everything else
- [ ] Writing one immediately reports what it would have done over retained history
- [ ] Firings are marked on the timeline and reachable with the jump key
- [ ] A watch reports nothing for a stretch that was never observed, and says so
- [ ] poptop delivers nothing anywhere; `--export` remains the integration point
- [ ] Named watches live in the config, in a form a human can read and edit
