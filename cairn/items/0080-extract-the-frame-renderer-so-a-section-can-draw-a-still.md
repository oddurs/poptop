---
id: 80
title: Extract the frame renderer so a section can draw a still
type: chore
status: done
milestone: web
created: 2026-09-08
updated: 2026-09-08
priority: p0
effort: m
area: web
---

## Problem

`assets/js/demo.js` is one closure that finds `[data-demo]`, reads the buffer,
and wires keyboard, pointer and playback to it. Everything it knows about
drawing — the braille packing, the threshold rules that bars absorb, the block
bars, the sparklines, the status thresholds — is locked inside that closure and
reachable only by the one interactive frame on the page.

Five of the sections planned for this milestone need to draw a frame that is
*not* interactive: two frames side by side at different cursors, a plot
aggregated two ways, a buffer that is half empty, a palette redrawn under
simulated colour-vision deficiency. Every one of them needs the same drawing
code, and none of them wants the scrubbing.

Copying the braille packing into each section is how the page ends up with five
drifting implementations of the thing the product is actually about.

## Proposal

Split the file in two.

`frame.js` exports the drawing: given a buffer, a cursor, a width and a set of
options, it returns the rows of a plot, a table body, a header line. It knows
nothing about events, playback or the DOM beyond writing into elements it is
handed.

`demo.js` keeps what makes the hero interactive — keyboard, pointer, playback,
the intersection observer — and calls into `frame.js` to draw.

A section then asks for a still by handing `frame.js` a buffer and a cursor and
never registering a listener. `data-frame` marks a still; `data-demo` keeps
meaning the one interactive frame.

Two options the stills need that the hero does not:

- **aggregate**: `peak` (today's behaviour) or `mean`, so the peak-versus-mean
  section can draw the same samples both ways.
- **from**: a start index, so the empty-buffer section can draw a plot whose
  left third has no samples behind it.

## Acceptance criteria

- [ ] `frame.js` draws without touching an event listener
- [ ] The hero is unchanged in behaviour — `audit/interact.js` still passes
- [ ] A still can be drawn from a buffer, a cursor and a width alone
- [ ] The braille packing, threshold rules and bar glyphs exist in one place
- [ ] Stills are inert to keyboard and pointer, and are not focusable
