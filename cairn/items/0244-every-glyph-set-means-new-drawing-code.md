---
id: 244
title: Every glyph set means new drawing code
type: chore
status: done
milestone: r11
labels:
- ui
- graph
created: 2026-09-23
updated: 2026-09-23
priority: p0
---

## Problem

`glyph_row` branches on the set, `fill_in_row` knows about rows and sub-rows,
`pairs_in_a_cell` exists because braille columns are independent and block
columns are not. The line mode picks box-drawing corners by inspecting its
neighbours; the fill mode picks a level per column. The two share a scale and
nothing else.

The cost is not tidiness. It is that a fifth set means a fifth branch in every
one of those functions, and that the quality of a line — joins, slopes, the
choice between two glyphs that both nearly fit — is decided by whichever branch
the reader landed in.

## Proposal

One rasterizer. Marks are drawn into a **coverage grid**: for each subcell, how
much of it the mark covers, 0..1. A surface then fits glyphs to that grid —
exact for braille and the block-element sets, nearest-match where a set is
partial.

```
trait Surface {
    fn subcells(&self) -> (usize, usize);   // per cell, x by y
    fn fit(&self, cell: &Coverage) -> (char, Ink);
}
```

That puts every decision in one place: coverage in, glyph out. `Draw::Bars`,
`Draw::Line` and the per-set special cases go away; a mark says what it covers
and the surface says what it can draw.

## Acceptance criteria

- [x] One rasterizer, one fitting step; no drawing code branches on the set
- [x] Existing sets render the same picture they do today, held by the current tests
- [x] A new set is a table, not a branch — proved by adding two in the next item
- [x] Anti-aliasing by level choice where the set has levels, and never where it would invent a value
