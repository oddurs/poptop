---
id: 109
title: The timeline is the biggest panel and is empty for its first minutes
type: feature
status: backlog
milestone: r5
labels:
- ui
created: 2026-09-19
updated: 2026-09-19
priority: p2
effort: m
area: timeline
---

## Problem

At 80×24 the timeline takes 11 of 24 rows. For the first few minutes about 90% of it is blank braille: the history is drawn 10 minutes wide while the data covers 45 seconds, packed against the right edge. The panel that is poptop's reason to exist looks like it's broken or has nothing to say, at exactly the moment someone has just started it.

## Proposal

Until the buffer fills, fit the time axis to the history that exists, widening as it grows, and switch to the configured window once it's full. The `past … now` caption already states the span, so the reader is told what they're looking at.

## Acceptance criteria

- [ ] With 45 seconds of history, the timeline uses its full width
- [ ] The caption states the span shown, as it does now
- [ ] Zooming with `+`/`-` still works and overrides the fit
