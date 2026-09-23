---
id: 261
title: A reader with a patched font gets nothing for it
type: feature
status: backlog
milestone: r15
labels:
- ui
- graph
depends_on:
- 246
created: 2026-09-23
updated: 2026-09-23
priority: p1
---

## Problem

Nerd Fonts are common enough that a reader who has one expects a tool to use
it. poptop uses the same ASCII and box-drawing marks whatever font is under it.

The reason to be careful is real: the patched glyphs live in the private use
area, where widths are ambiguous and a terminal may draw one of them two
columns wide — which shifts every column to its right, the bug
`every_marker_the_chrome_draws_is_one_column_wide` exists to catch.

## Proposal

A small, vetted accent set, opt-in: a handful of marks for the tab strip, the
chips and the tree's spine. Never a figure, never a graph, never the only way
something is said — the same rule colour follows. Each accent is measured with
the width table poptop already uses, and one that measures wide is refused with
the codepoint named.

## Acceptance criteria

- [ ] An opt-in accent set, off by default, with an exact fallback for every mark
- [ ] Every accent is one column wide, held by the existing marker test extended to the set
- [ ] Nothing in the set carries meaning that is not also carried without it
