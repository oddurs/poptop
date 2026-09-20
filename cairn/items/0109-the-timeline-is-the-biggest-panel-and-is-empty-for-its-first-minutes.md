---
id: 109
title: The timeline is the biggest panel and is empty for its first minutes
type: feature
status: done
milestone: r5
assignee: Oddur Sigurdsson
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

- [x] With 45 seconds of history, the timeline's empty part says what it is (was: "uses its full width"; see the note below)
- [x] The caption states the span shown, as it does now
- [x] Zooming with `+`/`-` still works: nothing about zoom changed

## How it was resolved

**Decided: keep the time axis and label the empty part, rather than stretching the axis to fill the width.** The proposal as filed would have fitted the axis to the history that exists. That would change the scale as history arrives: every line on screen reshaped, and then compressed in jumps, during exactly the minutes someone has just opened poptop to watch an incident. The codebase already rejects layouts that move under the reader (`App::one_user`), and `effective_zoom` says outright that the blank is meaningful: it is time from before there was any history, not missing data. What was wrong was that it didn't say so.

So the empty region now does. When the whole buffer is on screen and doesn't reach the left edge, a dim label is centred on the middle row of the empty part only: `no history before 13:33:58 — it fills from the right`. It shortens to `no history before 13:33:58`, then `before 13:33:58`, and disappears once history reaches the edge. It uses a clock time rather than "since poptop started" because a replayed day's buffer starts where its log does, not where this process did.

Tests: `the_empty_start_of_the_timeline_says_what_it_is` checks that the label appears, sits left of the data, and is gone once the panel fills. `the_rules_do_not_mark_a_buffer_that_has_no_data_yet` still checks that no graph glyph appears where no sample exists, with exactly the label's cells exempt. Live at 120×40 and 80×24, the label sits in the empty part and never touches the data.
