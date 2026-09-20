---
id: 139
title: The timeline fills, where it should draw a line
type: bug
status: backlog
milestone: v5.0
created: 2026-09-13
updated: 2026-09-13
priority: p0
area: ui
---

## Problem

The timeline fills every cell from the baseline to the value. A machine sitting
at eighty percent memory draws three rows of solid ink and puts all of its
information in the top one:

```text
  100 ⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿
  MEM ⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿
    0 ⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿
```

**Braille is not the cause.** `--glyphs=block` draws the same wall. The fill is.

## The setting

One setting names the **alphabet**; the renderer picks area-or-line from the
panel's height (0091's level-of-detail ladder). A one-row strip is a sparkline
in any alphabet and reads well; a tall panel is a line.

```ini
graph = block      # ▁▂▃ strip, ▄▀█ line — widest font support   (default)
      = braille    # ⣿   finest resolution, four sub-rows a cell
      = line       # ╭─╯ box-drawing line, the cleanest chart
      = ascii      #     no Unicode at all
```

`block` is the default because the block elements are in every font that draws a
terminal, and because the strip form — `▁▂▃▄▅▆▇█` — is the classic sparkline and
is what most panels will be at most widths.

`glyphs` keeps working as an alias for `braille`/`block`/`ascii`, since it is in
the README, in `--help` and in people's config files.

## What was measured

Eight renderings against the same traces — a build ramping up, a spike, then
idle, plus memory drifting 72→81%. The four kept are above; the rejected ones,
so nobody tries them again:

- **Half-block line without joining** — reads as scatter, not a line.
- **A five-mark ladder** (`▁ ▄ ─ ▀ ▔`) — more vertical resolution, reads ragged,
  because the marks are different weights.
- **Shaded area** (`░` under a bright top edge) — better than a solid fill and
  still mostly ink; no better than a line at anything.
- **Horizon strip** (density carries value) — very compact and it puts magnitude
  in a visual channel poptop reserves for identity, which collides with the rule
  the CVD work is built on.

The braille-versus-box trade, stated because it is not obvious: braille gives
four levels a row against box-drawing's one, and **the extra resolution works
against you**. Memory drifting 72→81% is genuinely flat; box draws it flat with
one step, braille draws it as wandering noise. The cursor readout already carries
the exact number, so legibility of the shape beats precision of the glyph.

## What this breaks, which is the actual work

A prototype of the renderer was about sixty lines and left 574 of 593 tests
passing. The nineteen are not mechanical:

- **Tests that find a graph by asking for the braille range.** They were tests of
  braille rather than of the graph, and they go blank the day the default
  changes. Needs a glyph-agnostic "is this a graph cell" and an ink measure that
  reads both alphabets.
- **The cursor's half-cell marker.** `▌`/`▐` said which of a cell's two samples
  the cursor was on. A line has one sample a cell, so that distinction is gone
  and the marker needs re-deciding rather than patching.
- **The threshold rules.** They filled cells the area did not reach, a minority
  of the panel. Against a line nearly every cell is empty, so the old spacing
  paints half the graph in chrome. Every fourth cell looked right.

## Acceptance criteria

- [ ] A machine at a steady high value draws a line, not a filled block
- [ ] `graph` selects the alphabet, and `block` is the default
- [ ] `glyphs` keeps working, as an alias
- [ ] The form follows the panel's height, not the setting
- [ ] The threshold rule is legible without competing with the signal
- [ ] The cursor still identifies exactly one sample, and says which
- [ ] No test asserts on a particular glyph alphabet
