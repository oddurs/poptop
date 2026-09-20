---
id: 123
title: The README is 2,312 lines and cannot be the front of anything
type: docs
status: done
milestone: r6
assignee: Oddur Sigurdsson
labels:
- docs
created: 2026-09-19
updated: 2026-09-19
priority: p1
effort: m
area: docs
---

## Problem

The README is the whole documentation: positioning, prior art, a design essay per feature, the reference, the development guide. It is good writing and the wrong shape — nobody can find anything in it, and a newcomer cannot tell in thirty seconds what poptop is or whether to install it.

The reasoning must not be thrown away. It is why the design is what it is, and several sections are the only record of an argument that was had.

## Proposal

The README becomes a front page: what it is, what it looks like, how to install and run it, the shape of the thing, and links into `docs/`. The essays move to `docs/design/` intact, beside `docs/roadmaps/`.

## Acceptance criteria

- [ ] README fits on a screen or two, with a rendered frame and links
- [ ] Every essay section survives under `docs/design/`, with nothing lost
- [ ] `docs/README.md` is an index of everything

## How it was resolved

The README is 177 lines: what poptop is, the rendered frame, install, the
shape of the command line, the three things it does that a conventional
monitor does not, a table of links into `docs/`, contributing, roadmap,
status, attribution, licence.

The 2,100 lines of essay moved to `docs/design/` intact — twelve files,
split on the section boundaries, with heading levels promoted and nothing
rewritten: prior art, the keys and the questions they answer, reading the
screen, configuration, recording and output, what it measures and what it
will not, colour, the graphs, what it costs to watch, how it works, notes
from reading the others, tests.

`docs/README.md` indexes all three kinds of page and says what each is for.
