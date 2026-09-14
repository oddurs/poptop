---
id: 81
title: The timeline fills, where it should draw a line
type: bug
status: backlog
milestone: v3.0
created: 2026-09-13
updated: 2026-09-13
priority: p0
area: ui
---

## Problem

The timeline fills every cell from the baseline up to the value. On a machine
sitting at eighty percent memory that draws four rows of solid ink and puts all
of the information in the top one — the other three carry none. It reads as a
wall rather than as a shape:

```text
  100 ⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿
  MEM ⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿
    0 ⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿
```

That is a graph of a real machine carrying no information at all.

Braille makes it worse — at low values the dots read as speckle rather than as
a low line — but braille is not the cause. `--glyphs=block` draws the same wall,
because the **fill** is the problem, not the alphabet.

## What was measured

Four renderings of the same three traces — a build with a spike, memory sitting
high, bursty iowait. The winner, a joined line stroke in half blocks:

```text
  100                     ▄▄▄   ▄▄▄    ▄▄▄      █▀▀█
                        █▀▀ ▀█▄█▀ ▀▀██▀▀ ▀█▄▄   █  █
  CPU                   █                   █   █  █
    0 ▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄█                   █▄▄▄█  █▄▄▄▄▄▄▄▄▄▄▄

  100 ▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄
  MEM
    0
```

Memory is one readable line near the top. The CPU trace has a shape the eye
follows. Both are the same data as the wall above.

## The design, with what was rejected

- **A joined stroke, not a point per column.** The mark covers every sub-row
  between this sample and the next, so a climb is a vertical run and the line is
  continuous. Unjoined marks were prototyped and read as scatter.
- **One weight throughout.** A five-mark ladder (`▁ ▄ ─ ▀ ▔`) buys more vertical
  resolution and reads ragged, because the marks are different weights. `▄ ▀ █`
  is one weight and reads as a stroke. Prototyped and rejected.
- **`--glyphs` keeps all three sets, all drawing the line**: `block` (default,
  two sub-rows a cell), `braille` (four sub-rows — braille is *good* at lines,
  it was only ever bad at fills), `ascii` (`_ - |`).
- **One sample a cell.** The two-samples-a-cell packing existed to double the
  horizontal resolution of an *area* chart; a line has one stroke a column. This
  halves the samples on screen, and `+`/`-` is the answer — zoom already
  aggregates by peak, so a spike survives the compression.

## What this breaks, which is the actual work

A prototype of the renderer was about sixty lines and left 574 of 593 tests
passing. The nineteen are not mechanical:

- **Tests that find a graph by asking for the braille range.** They were tests
  of braille rather than of the graph, and they go blank the day the default
  changes. Needs a glyph-set-agnostic "is this a graph cell" and an ink measure
  that reads both alphabets.
- **The cursor's half-cell marker.** `▌`/`▐` said which of a cell's two samples
  the cursor was on. With one sample a cell that distinction is gone; the marker
  and its tests need re-deciding rather than patching.
- **The threshold rules.** They filled cells the area did not reach, a minority
  of the panel. Against a line nearly every cell is empty, so the old spacing
  paints half the graph in chrome and the reference competes with the signal.
  Every fourth cell looked right in the prototype.

## Acceptance criteria

- [ ] A machine at a steady high value draws a line, not a filled block
- [ ] The stroke is continuous: no column of a moving series is blank
- [ ] All three glyph sets draw the line, and `block` is the default
- [ ] The threshold rule is legible without competing with the signal
- [ ] The cursor still identifies exactly one sample, and says which
- [ ] No test asserts on a particular glyph alphabet
