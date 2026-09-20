---
id: 132
title: The colour work is the part nobody can see without running it
type: docs
status: done
milestone: r7
assignee: Oddur Sigurdsson
labels:
- repo
created: 2026-09-19
updated: 2026-09-19
priority: p2
---

## Problem

poptop's palette is measured: `--check-theme` reports the separation between
every pair of meaning-bearing hues under simulated protanopia, deuteranopia
and tritanopia, and the default replaces green with cyan because green and
yellow separate by only dE 3.7 for roughly 8% of men. That is one of the
better arguments this project has, and every frame in the README and the
guide is monochrome text.

A photograph would be wrong — a screenshot is a claim nobody can check, and
it goes stale silently. An SVG rendered from a real terminal capture is the
same frame, with the colours poptop actually emitted.

## Acceptance criteria

- [ ] An SVG of a real frame, generated from a capture rather than drawn
- [ ] The generator is committed, so the image can be regenerated
- [ ] It renders in GitHub's README
- [ ] The plain-text frame stays: it is what a reader can copy and diff

## How it was resolved

`tools/screenshot.py` runs the release binary under a pseudo-terminal, lets
a terminal emulator interpret everything it wrote, and emits the resulting
grid as SVG with the colours poptop actually asked for. The generator is
committed, so the image can be regenerated after a change.

Two things it had to get right, both found by looking at the output:

- **The alternate screen.** The emulator has one buffer where a terminal has
  two, so a warning poptop printed before it took the screen stayed under
  the frame, in whatever cells the frame did not cover. The stream is now
  split at `ESC [ ? 1049 h` and the emulator cleared there, which is what
  the second buffer would have done.
- **The held warnings.** poptop prints those after it gives the terminal
  back. Feeding them to the emulator scrolled the frame up a line and lost
  the header. The capture stops feeding before quitting, and drains the pty
  without reading it into the screen.

The frame itself is a ten-core Linux box under a workload started for the
purpose — three `rustc`, a `sha256sum`, `xz`, `gzip` and a `dd` — so every
name in the picture is a program that was really running, and anyone with
Docker can reproduce it. It shows `30 rustc (g folds them)`, `47 tasks came
and went`, `████+`, real disk throughput, and the palette: cyan for live,
`#ffd580` at warn, `#ff6666` at critical.

The plain-text frame stays, moved into a `<details>` under the picture.
