---
id: 88
title: The keymap should drive the frame it documents
type: feature
status: done
milestone: web
created: 2026-09-08
updated: 2026-09-08
priority: p2
effort: s
area: web
---

## Problem

The keymap section lists ten bindings and the hero frame sits above it
implementing all of them. A reader learns the keys by reading a table, when
they could learn them by using them.

The section's claim is *it fits on one screen* — that poptop is small enough to
hold in your head. Nothing demonstrates that better than the reader operating
the whole keymap in ten seconds without leaving the page.

## Proposal

Make the keycaps in the keymap real controls for the frame above. Clicking `←`
scrubs it; `Space` toggles it; `+` and `-` zoom it. The page scrolls the frame
into view if it is off screen, so the effect is never invisible.

Bindings the web frame does not implement — `t` for the process tree, `i` for
the IO columns, `/` for the filter, `q` — stay as documentation and say so
rather than pretending: they are marked as belonging to the terminal, not to
this page. Claiming a key works and having it do nothing is worse than the table
we have now.

## Acceptance criteria

- [ ] The keys the frame implements are operable from the keymap
- [ ] Keys that only exist in the terminal are marked as such
- [ ] The frame is scrolled into view before it responds
- [ ] Every control is a real button, reachable by keyboard
