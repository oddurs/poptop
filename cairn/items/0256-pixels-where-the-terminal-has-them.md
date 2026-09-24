---
id: 256
key: r14
title: Pixels where the terminal has them
type: milestone
status: backlog
labels:
- ui
- graph
created: 2026-09-23
updated: 2026-09-23
priority: p1
---

The fourth milestone, and the first that depends on something outside poptop.

A cell is about 10×20 pixels. Three rows of graph are sixty pixels tall and
twenty-four distinguishable heights, so the character sets throw away most of
what the screen can already draw (0192). Ghostty, Kitty, WezTerm and Konsole
implement the Kitty graphics protocol; xterm, foot, Contour and Windows
Terminal do Sixel; iTerm2 has its own. On those, a graph can be drawn at the
resolution of the screen.

This is an **enrichment tier**, which is the renderer's version of the rule
0068 set for the collector: poptop draws what the terminal shows an ordinary
reader, and anything richer enriches the picture and is never required for the
tool to work. Concretely:

- It is off unless the terminal answers a query, and the reader may turn it off
  for a terminal that does answer.
- It draws **the same picture**: the same scales, the same seams, the same
  thresholds, the same absent-not-zero. Finer, never different. Two poptops
  looking at one machine must not disagree about what happened.
- Everything it can draw, the cells can draw less well. No figure and no mark
  exists only in pixels.

The costs are real and belong in the item that lands it: a pixel graph cannot
be copied out of the terminal as text, needs `allow-passthrough` under tmux,
costs bandwidth over ssh, and cannot appear in the README's capture — which is
itself a reason the character tiers stay the ones poptop is judged on.
