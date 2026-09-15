---
id: 106
title: A graph pinned to zero wastes the panel on a high flat series
type: bug
status: todo
milestone: v3.5
created: 2026-09-14
updated: 2026-09-14
priority: p0
area: ui
---

## Problem

The ceiling adapts to the data. The floor does not — it is zero, always. So a
memory series that lives between 72% and 85% is drawn across a panel that spans
0 to 100, and the 72 points below the signal are ink that never changes:

```
  100 ▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄
  MEM ████████████████████████████████████████
    0 ████████████████████████████████████████
```

Three rows, of which one carries information. This was reported twice as "the
graphs look like a wall" and read twice as a question about the character set —
first braille, then the fill. It was neither. The glyph work was worth doing on
its own terms and did not touch this.

The variation that matters — was it 74 or 84, is it climbing — is drawn inside
a single row, at a resolution of one eighth of a cell, while two rows below it
say nothing at all.

## The fix, and the honesty problem it creates

Fit the axis to the data: floor at (a little below) the minimum, ceiling at (a
little above) the maximum. The panel then spends all of its rows on the 13
points that vary.

This is the classic misleading chart if done to **bars**. A bar's area encodes
its magnitude, so truncating the axis makes 74 look like a third of 84. The
convention exists because readers do not check axes, and it is right.

So the form has to follow the axis:

- **Axis at zero → bars.** Area encodes magnitude and the encoding is true.
- **Axis fitted → a line.** A line encodes *change*, and a truncated axis is
  standard and honest for one. The gutter labels the floor, so the baseline is
  never unstated.

That rule is also the answer to the argument this project has already had twice
about lines against fills. Both were right about different data. A series that
spans its range wants bars; a series that lives in a band near the top wants a
line. The graph can tell which it is holding.

## What needs deciding

- **When to fit.** Proposed: fit when the zero-based view would spend less than
  about a third of the panel on the data's span. Needs a hysteresis or the axis
  will flap between forms as the window slides.
- **Where the fitted bounds land.** Not on the raw min and max — the series
  would touch both edges and clip. Round outward to readable numbers.
- **What the thresholds do.** A warn rule at 50 is off the scale of a panel
  fitted to 72–85, which is correct and already the behaviour for an off-scale
  rule. Worth confirming it does not silently look like "nothing is wrong".
- **Whether it is overridable.** `scale = auto | zero | fit` at minimum; 112
  wants `shared` as well.

## Acceptance criteria

- [ ] A series in a narrow high band uses the whole panel
- [ ] A fitted axis is never drawn as bars
- [ ] The floor is labelled whenever it is not zero
- [ ] The form does not flap as the window slides
- [ ] A series that genuinely spans its range is unchanged
