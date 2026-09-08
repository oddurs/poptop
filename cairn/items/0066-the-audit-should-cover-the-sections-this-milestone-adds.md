---
id: 66
title: The audit should cover the sections this milestone adds
type: chore
status: done
milestone: web
created: 2026-09-08
updated: 2026-09-08
priority: p1
effort: s
area: web
---

## Problem

`audit/audit.js`, `audit/interact.js` and `audit/headings.js` cover the page as
it is. This milestone adds five stills, two more interactive regions and a
control with four states, none of which those scripts know to look at.

The bugs this milestone can introduce are exactly the ones the audit exists to
catch: a still that overflows on a phone, a state control that is only
distinguishable by colour, a section heading that reads as body text, a canvas
that never draws because its buffer never arrived.

## Proposal

Extend the three scripts as the sections land:

- Every still draws something — a frame element whose plot is empty is a
  failure, not an empty state.
- The colour-vision control is operable by keyboard and its current state is
  marked by more than hue.
- The monochrome state genuinely removes hue: assert no meaning-bearing colour
  survives it.
- The page's total motion is still one orchestrated moment; nothing else
  animates on load.
- Contrast, heading order and tap targets across the new sections, at all four
  widths and both themes, as before.

## Acceptance criteria

- [ ] Each new section has at least one assertion that would fail if it broke
- [ ] The audit passes at four widths in both themes with zero findings
- [ ] The interaction suite covers the second scrubbable region
- [ ] Running all three scripts is documented in `audit/README.md`
