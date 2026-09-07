---
id: 34
title: A third of the screen is chrome
type: bug
status: done
milestone: v1.0
created: 2026-09-07
updated: 2026-09-07
priority: p1
area: ui
---

## Measured

On a 30-row terminal, **11 rows are chrome**: title, three describing the
timeline, the process section header, the column header, and the key bar. The
process table — the densest thing on screen — gets 15.

Three of those eleven describe one graph block:

```text
── timeline — 2s of 59s buffered ────────────────────────
   50 ⠀⠀⠀…
  CPU ⠀⠀⠀…
    0 ⠀⠀⠀…
      past                                            now
2s shown, 1s/slot — ←/→ scrub, +/- zoom
```

`past … now` and `2s shown, 1s/slot` are the same statement twice — one as an
axis, one as a caption — and `←/→ scrub, +/- zoom` repeats two of the nine
bindings already listed in the key bar at the bottom of the screen.

## What should happen

One row, carrying the axis and the scale together:

```text
      past ←  2s shown · 1s/slot  → now
```

The keys come out; they are in the key bar, and a hint that is always on screen
is not a hint.

That is one row back, and with the state row folded into the figures (0033) it
is two — 13% more of the process table on a 30-row terminal, which is where the
information actually is.

## Acceptance criteria

- [ ] The timeline costs one chrome row below the graph, not two
- [ ] Nothing the caption said is lost — span, slot size, direction of time
- [ ] Key hints appear once on screen, in the key bar
- [ ] Measured: rows of chrome before and after, at a stated terminal size
