---
id: 242
title: The overlays are sharp where the rest is soft
type: feature
status: backlog
milestone: r10
labels:
- ui
created: 2026-09-23
updated: 2026-09-23
priority: p3
---

## Problem

The key list, the inspector, the menus and the jump box are drawn with square
corners, a single-line border and no padding, against a screen whose other
surfaces are dim rules and braille. They read as dialogs from a different
program.

## Proposal

One surface style for everything that floats: rounded corners where the glyph
set has them, a blank column inside the border, the title in the same place and
the same ink on every one of them, and the same dismissal line.

## Acceptance criteria

- [ ] Every floating surface is drawn by one function, with one style
- [ ] The ASCII glyph set still gets square corners and looks deliberate
