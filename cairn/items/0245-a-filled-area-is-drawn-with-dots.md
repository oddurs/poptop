---
id: 245
title: A filled area is drawn with dots
type: feature
status: backlog
milestone: r11
labels:
- ui
- graph
depends_on:
- 244
created: 2026-09-23
updated: 2026-09-23
priority: p0
---

## Problem

The default set is braille, and braille is dots with gaps between them. At the
top of a filled area that reads as texture rather than as a surface, which is
most of what makes the graphs look speckled rather than drawn.

The block elements fill solidly but resolve one column a cell, so a filled
graph is either solid and coarse or fine and speckled.

## Proposal

Two more surfaces, both solid, both at braille's resolution or close to it:

- **Sextants** (U+1FB00–U+1FB3B, Unicode 13): 2×3 subcells a cell.
- **Octants** (Symbols for Legacy Computing Supplement, Unicode 16): 2×4, the
  same grid as braille with no gaps.

Then the default is chosen by what the mark is rather than by what the reader
set: areas fill with octants where the font has them and sextants below that;
lines and scatter keep braille, which is what dots are good at.

## Acceptance criteria

- [ ] Sextant and octant surfaces, added as tables under the engine from the item above
- [ ] Filled marks default to the best solid set the font is known to have
- [ ] Braille stays the default for line and scatter marks
- [ ] Every set still degrades to ascii, and `--graph=` still names the set explicitly
