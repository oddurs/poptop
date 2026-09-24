---
id: 243
key: r11
title: The engine draws, the surface fits
type: milestone
status: backlog
labels:
- ui
- graph
created: 2026-09-23
updated: 2026-09-23
priority: p0
---

The first of five milestones behind one idea: **poptop rasterizes into subcell
coverage, and a surface fits glyphs to it.**

Today the drawing code knows what braille is. `pairs_in_a_cell`, `fill_in_row`
and `glyph_row` each branch on the set, so every new set is new drawing code and
the fill mode and the line mode share almost nothing. That is why there are four
sets and not eight, and why a filled area is drawn with dots.

The engine after this milestone:

```
series → scale → marks → subcell coverage grid → fit glyphs → cells
```

A surface declares two things — how many subcells a cell holds, and which
coverage patterns it has a glyph for. Everything else is shared: one
rasterizer, one set of marks, one degradation ladder. Adding octants becomes a
table rather than a branch, and the pixel tier in r14 becomes another surface
rather than another renderer.

The immediate win is legibility. Braille is dots with gaps between them, which
is what makes the filled graphs look speckled; sextants and octants are solid
at the same resolution. That costs nothing but a table and a fallback.

| set | subcells | fill | notes |
|---|---|---|---|
| ascii | 1×1 | poor | always works |
| block | 1×8 | good vertical | today's default |
| quadrant | 2×2 | coarse | everywhere |
| sextant | 2×3 | very good | Unicode 13 |
| octant | 2×4 | best of the cells | Unicode 16, newest fonts |
| braille | 2×4 | dotty | best for lines and scatter |

Font coverage cannot be probed, so the ladder ends in a setting and a test
card, the way colour ends in `--check-theme`.
