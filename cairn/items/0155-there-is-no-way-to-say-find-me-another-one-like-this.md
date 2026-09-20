---
id: 155
title: There is no way to say "find me another one like this"
type: feature
status: backlog
milestone: v6.0
depends_on:
- 146
created: 2026-09-13
updated: 2026-09-13
priority: p1
area: ui
---

## Problem

You are looking at a shape on the timeline — a ramp, a sawtooth, a cliff — and
the question is whether it has happened before. 0088 lets you search for
*values*: `cpu > 80 for 30s`. It cannot search for a **shape**, and shape is what
you are actually looking at.

The difference matters because the interesting pattern usually is not extreme.
Memory sawtoothing between 60% and 75% every eleven minutes is a leak with a
restart loop on top, it never crosses a threshold, and no value query finds it.

## What it does

Select a region of the timeline and ask for its recurrences:

```text
  ▔▔▔▔▔▔▔▔▔▔  selected 03:02:11 → 03:06:40  (4m29s)

  5 similar in the last 7 days

  Mon 03:02  ████████░░  92% alike    cc1plus       ← selected
  Tue 03:01  ███████░░░  88% alike    cc1plus
  Wed 03:04  ███████░░░  85% alike    cc1plus
  Thu 14:22  ██████░░░░  71% alike    rustc         different process
  Sun 09:10  █████░░░░░  64% alike    —             nothing accounted for it
```

The right-hand column is what makes it poptop's: two stretches that look
identical and were caused by different processes are a different finding from two
that were caused by the same one, and only poptop can tell you which.

## The method, and why it is not a research project

Normalised sliding-window distance — z-normalise the query window, slide it over
the buffer, keep the best non-overlapping matches. This is the well-trodden
motif-discovery approach and the naive form is O(n·m), which at 600 samples and a
300-sample window is 180,000 float operations: nothing. A day of ten-minute log
is 144 samples. The matrix profile literature exists for streams orders of
magnitude larger than this and is not needed here.

**Z-normalising is the decision that matters.** It makes the search about shape
rather than level, which is the whole point — a ramp from 10% to 40% matches a
ramp from 60% to 90%. It also means a flat line matches every other flat line, so
the query has to refuse windows with no variance rather than return a thousand
matches.

## Why nobody else has this

Not because it is hard. Because their history is aggregated: a monitor that
stores a five-minute mean has destroyed the shape before the question is asked.
poptop retains the samples at their collected resolution, so the shape survives —
and it retains the process tables, so each match can be attributed.

## What needs deciding

- **How a region is selected.** A mark key and the cursor, as 0083's diff needs
  too. Worth doing once for both.
- **Which series.** The selected panel's, obviously. Across several series at once
  is a much better question — "find me when the machine as a whole looked like
  this" — and a much harder one. Start with one.
- **What "alike" means as a number.** Anything shown as a percentage will be read
  as a probability. Either give it an honest scale or do not print a number at
  all and rank instead.
- **Trivial matches.** The window overlapping itself is the top match in every
  naive implementation, and every neighbour of it is second. Exclusion zones are
  standard and must not be forgotten.

## Acceptance criteria

- [ ] A selected stretch of timeline can be searched for elsewhere in history
- [ ] Matches are ranked, and each names what was responsible at that time
- [ ] The match is on shape, not level, and that is stated
- [ ] A featureless selection is refused rather than matching everything
- [ ] The window cannot match itself or its immediate neighbours
- [ ] Search cost measured against a full retention window, not the live buffer
