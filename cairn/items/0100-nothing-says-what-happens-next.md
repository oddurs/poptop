---
id: 100
title: Nothing says what happens next
type: feature
status: backlog
milestone: v4.1
depends_on:
- 84
created: 2026-09-13
updated: 2026-09-13
priority: p2
area: ui
---

## Problem

Memory is at 84% and climbing. The question is whether it matters, and the
question behind it is **when it hits the ceiling** — in four minutes or next
Thursday. poptop shows the level and the graph and leaves the extrapolation to a
person squinting at a slope.

## What it does

One line, on a figure that is actually trending, with the arithmetic shown:

```text
  MEM 84.1%   +2.1G over 40m   →  full in ~1h20m   (fit over 40m, ±25m)
```

And where the data does not support a projection, the refusal, which is most of
the time:

```text
  MEM 84.1%   flat over 40m    →  no trend
  CPU 61.2%   →  not projected: sawtooths, no trend to extend
```

## The rule that keeps this from being astrology

**A projection is a claim about the future and poptop's whole position is that it
only says what it measured.** So three constraints, none optional:

- **Linear only, over a stated window.** No seasonality, no curve fitting. A
  straight line through the recent past is a thing a reader can check by looking
  at the graph, which means they can disagree with it.
- **An error, always.** A projection without one is a prediction; with one it is
  an extrapolation. The residual of the fit gives it honestly and cheaply.
- **Refusal is the common case.** CPU does not trend, it oscillates. Disk does
  not trend. The figures that genuinely trend are memory, disk *fullness*, file
  descriptors, thread counts and log volume — things that leak. Anything that
  fails a straightness test gets `no trend`, and that must be the default rather
  than the exception.

## Why poptop can and Prometheus cannot

`predict_linear()` exists in PromQL and is used for exactly this. The difference
is not the arithmetic — it is that when poptop says "full in about an hour" it
can also say **which process is doing it**, from the same sample, because the
process table is there. "Memory full in an hour" is an alert; "memory full in an
hour, and it is `node` growing 40 MB a minute since its restart at 03:04" is a
resolution.

That join is 0082's attribution applied to a slope instead of a level.

## What needs deciding

- **Which figures are eligible.** A whitelist of the ones that leak, not
  everything. Projecting a figure that cannot trend is how a feature like this
  gets a reputation.
- **The window.** Too short and every wobble is a trend; too long and a leak that
  started ten minutes ago is invisible. Probably several windows, and a trend is
  only reported when they agree — which is also a cheap honesty check.
- **What "full" means.** For memory it is not 100%: a box is in trouble when it
  starts swapping or reclaiming, which is earlier and machine-specific. The
  baseline from 0084 knows where this machine usually sits.
- **Whether it ever escalates.** It should not. A projection that turns a figure
  red is a threshold with extra steps, and the printed heat scale would stop
  meaning what it says.

## Acceptance criteria

- [ ] A trending figure states when it reaches its ceiling, with an error
- [ ] The window the fit used is printed, so the claim can be checked
- [ ] A figure that does not trend says so rather than being extrapolated
- [ ] Only figures that can leak are eligible, and the list is stated
- [ ] The projection names the process responsible for the slope
- [ ] No projection changes any colour anywhere
