---
id: 246
title: A glyph set is code, and a set is all or nothing
type: feature
status: backlog
milestone: r11
labels:
- ui
- graph
depends_on:
- 244
created: 2026-09-23
updated: 2026-09-23
priority: p1
---

## Problem

A set is an enum arm. A font that has the block elements but not the octants
cannot be described, so the reader picks between a set that is too coarse and
one that renders tofu.

## Proposal

A set is data: a table from coverage pattern to codepoint, plus which marks it
can carry. Sets are loaded the way themes are — the built-in ones from the
binary, a reader's own from a file — and validated on load: every pattern the
set claims must be a single codepoint of width one.

A partial set is legal and useful. A set that has solid fills but no line
joints carries areas and hands lines to the next set down.

## Acceptance criteria

- [ ] Built-in sets are tables, not arms
- [ ] A set declares the marks it can carry, and an unsupported mark falls to the next set rather than rendering wrong
- [ ] A set whose glyphs are not one column wide is refused at load, with the offending pattern named
