---
id: 73
title: Draw the landing page's frame with the real renderer, via wasm
type: feature
status: backlog
milestone: later
created: 2026-09-08
updated: 2026-09-08
priority: p3
effort: xl
area: web
---

## Problem

The demo on the landing page is a faithful reimplementation of poptop's
renderer, not poptop. Every caption on the page has to be careful about that,
and a golden test can only detect divergence after the fact.

## Proposal

Compile the actual renderer to WebAssembly and let the browser frame call it.
The demo stops being a likeness; the claim becomes literally true; the second
implementation disappears along with the class of bug it creates.

The honest cost: shipping a ratatui backend and a terminal surface for one
figure on one page, against 32 KB of JavaScript that draws the same cells today.
That trade is not obviously right, which is why this is speculative rather than
planned.

Worth an afternoon of prototyping to find out what it actually weighs, once the
boring findings are closed.

## Acceptance criteria

- [ ] A measurement of what the wasm bundle actually costs
- [ ] A decision recorded either way, with the number behind it
