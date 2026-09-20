---
id: 142
title: Nothing knows what normal looks like for this machine
type: feature
status: backlog
milestone: v5.0
depends_on:
- 72
created: 2026-09-13
updated: 2026-09-13
priority: p1
area: ui
---

## Problem

poptop reports that memory is at 84%. Whether that is a catastrophe or a
Tuesday is a judgement the reader has to bring from somewhere else. The `warn`
and `critical` thresholds are the current answer and they are a constant the
user sets once — the same number for a build box at 3am and a web server at
peak.

netdata's headline feature is unsupervised anomaly detection: a model per
metric, trained at the edge, telling you what is unusual rather than what is
high. That is the right idea and the wrong mechanism for this tool.

## Why poptop can do this, and do it better

Once `log` is on, poptop has days of samples of **this** machine. The honest
version of "is this unusual" is not a neural network, it is a quantile:

> `mem 84%` — highest this hour has been in 7 days (p50 for 09:00 Mon–Fri: 61%)

That is explainable, auditable, cheap, and a reader can disagree with it. An ML
model that says "anomaly score 0.87" cannot be argued with, which in an incident
is a defect rather than a feature.

The comparison is against **hour of week**, because a machine's shape is a
weekly shape: 09:00 Monday is not 03:00 Sunday, and a model that averages them
calls every Monday morning an anomaly.

## What needs deciding

- **How much history is enough to judge.** With two days of log, "highest in 7
  days" is a lie. The panel must refuse to judge below some coverage and say so:
  *"3 days recorded — too little to call this unusual"*. This is the same rule
  as the em dash everywhere else.
- **What is stored.** Re-reading a week of logs on every frame is not possible.
  A small digest per metric per hour-of-week — count, and a few quantiles — is a
  few kilobytes and can be updated as the log is written.
- **Whether it changes any colour.** It should not, at least at first. The heat
  ramp is a stated scale with a documented threshold; a figure that turns red
  because a digest says so breaks the promise that the scale means what it says.
  Baseline belongs in words, beside the figure.
- **Gaps and reboots.** A machine that was off for three days has not been
  quiet; it has been absent. The digest has to distinguish those or every Monday
  after a maintenance window reads as a record low.

## Acceptance criteria

- [ ] A figure can be compared against this machine's own history, by hour of week
- [ ] The comparison is a quantile a reader can check, not a score
- [ ] Too little history refuses to judge and says how much there is
- [ ] Absence is never counted as a low reading
- [ ] The heat ramp keeps meaning exactly what its printed scale says
- [ ] The digest's size and update cost are measured, not estimated
