---
id: 148
title: The first run teaches nothing
type: feature
status: backlog
milestone: v5.2
created: 2026-09-13
updated: 2026-09-13
priority: p3
area: ui
---

## Problem

poptop has scrubbing, zoom, a query language, a tree, four groupings, three
views, a detail panel, a cgroup panel, a jump box, signals and a report. The
footer shows eleven key hints, `--help` is two hundred lines, and the README is
two thousand.

A first-time reader sees a good-looking monitor and finds the thing that makes
it different — that you can go backwards — only if they press an arrow key for
no reason.

htop's answer is `F1` and a setup screen. btop's is a menu. Both are discoverable
in a way poptop is not.

## What is actually missing

Not a tutorial. The one thing worth saying, once:

> `←` `→` move through the last ten minutes. The table follows.

Shown on the first run and never again, or until the reader scrubs — whichever
comes first, because somebody who has already scrubbed has learnt it.

Everything else in the tool is discoverable from the footer. This one thing is
not discoverable at all, and it is the whole product.

## What needs deciding

- **Where "first run" is remembered.** A file in the state directory is the
  obvious place, and it is also the tool's first piece of state written without
  being asked — which `store` and `log` both deliberately avoid. A marker file
  is smaller than a history, but the principle is the same one and deserves the
  same sentence of justification or a different mechanism.
- **Whether a setup screen is wanted at all.** Probably not: every setting is in
  a config file that `--help` documents and the ladder already degrades. A setup
  screen is a second way to change the same values and a second thing to keep in
  step with the file.
- **Mouse.** Separate, and worth its own item if anybody wants it. It is on
  every comparable tool and absent here, but it is a convenience rather than a
  discovery problem, and this item is about discovery.

## Acceptance criteria

- [ ] The scrubbing keys are discoverable without reading anything
- [ ] The hint appears once and never becomes furniture
- [ ] It costs no row in the steady state
- [ ] Whatever state remembers it is justified against the "nothing is written
      unless asked" rule, or it does not persist at all
