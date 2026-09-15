---
id: 108
title: A character cell is eight levels and a pixel is one
type: feature
status: todo
milestone: v3.5
created: 2026-09-14
updated: 2026-09-14
priority: p2
area: ui
---

## Problem

The block elements resolve eight levels in a cell. Braille resolves four and
buys width back. That is the ceiling of what characters can do, and it is a
ceiling: a three-row graph has 24 distinguishable heights, and half of the
terminal's actual vertical resolution is unreachable because the glyph set has
no mark for it.

A character cell on a typical terminal is around 10 by 20 pixels. The same three
rows are 60 pixels tall. The graph is throwing away 60% of what the screen can
already show.

## The fix

Draw with the terminal graphics protocols where they exist:

- **Kitty graphics protocol** — kitty, WezTerm, Ghostty, and others.
- **Sixel** — foot, Konsole, mlterm, xterm with `--enable-sixel`, iTerm2,
  Windows Terminal.

Both place an image at a cell position. The glyph sets stay as the fallback and
stay the default over SSH into anything unknown, which is most of the time this
tool is used in anger.

What it buys, beyond resolution: antialiasing, so a line is a line rather than a
staircase; real area fills at a weight below solid; a proper axis with ticks;
and 107's min–max band as an actual band rather than a second row of characters.

## What needs deciding

- **Detection.** Querying for Sixel support means writing an escape and waiting
  for a reply, which hangs on terminals that do not answer. There is a known-good
  approach (primary device attributes with a timeout and a fallback), and it
  needs to fail towards characters, never towards a hang.
- **Redraw cost.** An image is re-sent on every frame unless it is cached by id.
  At one frame a second this is likely fine and needs measuring, not assuming —
  `--bench` exists for this.
- **Scroll and resize.** Images do not reflow. Every resize invalidates.
- **Whether it is worth it.** This is the largest item in the milestone and the
  only one a user can decline by having the wrong terminal. It should land after
  106 and 107, both of which improve the character rendering that most people
  will keep seeing.

## Acceptance criteria

- [ ] Detected, never queried in a way that can hang
- [ ] Falls back to the current rendering with no visible failure
- [ ] `--graph=block` still forces characters on a capable terminal
- [ ] No measurable regression in `--bench`
