---
id: 263
title: The octant table needs Unicode data poptop cannot see
type: feature
status: backlog
milestone: r11
labels:
- ui
- graph
created: 2026-09-23
updated: 2026-09-23
priority: p2
---

## Problem

Octants — a 2×4 cell, braille's grid drawn solid — are the best a character
set can do, and they are the one surface r11 did not ship. They were added in
Unicode 16, and the machine this was built on carries Unicode 15.1: the block
is not in its tables, so the codepoints could only have come from memory.

A table written from memory is how a set ships tofu, and the card would have
shown a row of boxes with nothing to say why.

## Proposal

The surface is already a table (0246), so this is data rather than code: derive
the mapping the way the sextant one was derived — from the names Unicode gave
the glyphs, `BLOCK OCTANT-…`, which number the subcells from the top left — and
check it in a test that spot-checks the codepoints against those names.

Needs `UnicodeData.txt` for 16.0, or a toolchain whose `unicodedata` knows the
block.

## Acceptance criteria

- [ ] An octant surface, derived and checked the way the sextant one is
- [ ] It appears on `--check-glyphs`, so a reader with a font that has it can see that it does
- [ ] Fonts without it are why the default does not change
