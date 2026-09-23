---
id: 247
title: Nothing says whether this font has the glyphs
type: feature
status: backlog
milestone: r11
labels:
- ui
- graph
created: 2026-09-23
updated: 2026-09-23
priority: p1
---

## Problem

Font coverage cannot be queried. A terminal will happily draw a replacement
box, and poptop cannot tell the difference between a glyph and tofu — so
choosing a default for the reader is guessing with their screen.

Colour had the same problem and answered it: `--check-theme` prints the
contrast it measured and says which palette passes.

## Proposal

`poptop --check-glyphs`: a test card. One row per set, each drawing the same
short series, with the set's name and the codepoints it used. The reader sees
which rows are solid and which are boxes, and sets `graph = octant` once.

Plus a conservative default: the widest set poptop is confident about without a
card is `block`, and the card is what the first-run guide points at.

## Acceptance criteria

- [ ] `--check-glyphs` prints every set, the same series in each, and the setting that selects it
- [ ] It runs without a terminal takeover, like `--check-theme`, so it can be piped
- [ ] The first-run guide names it where it explains the graph
