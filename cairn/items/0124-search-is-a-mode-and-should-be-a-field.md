---
id: 124
title: Search is a mode and should be a field
type: feature
status: backlog
milestone: v4.0
created: 2026-09-15
updated: 2026-09-15
priority: p2
area: ui
depends_on:
- 119
---

## Problem

Activity Monitor's search field is always there, top right. It holds what you
typed, it shows what you typed, and it takes the keyboard only when you click
it. Everything else keeps working while it has content.

poptop's `/` is a mode: it swallows every key until Escape, the text lives in a
box that appears and disappears, and while it is open nothing else responds.

The mode is not the problem — a terminal has to route the keyboard somewhere.
The problem is that leaving the mode hides the fact that a filter is applied.

## The fix

The filter becomes part of the scope line (119) rather than a box: always
visible when set, stated as scope rather than as a dialogue. `/` focuses it,
Escape leaves it focused but not capturing, and a second Escape clears it —
which is the distinction Activity Monitor draws with a field and poptop
currently cannot express at all.

## What needs deciding

- **Two Escapes is a convention nobody has met.** The alternative is an explicit
  key to clear, and a filter that survives an Escape the reader thought would
  undo it.
- **Whether the match count belongs to the field or to the scope line.** `4 of
  760` is scope; `postgres` is the query.

## Acceptance criteria

- [ ] A filter that is set is visible without pressing anything
- [ ] Clearing it is one obvious action, not a guess
- [ ] Typing in it does not stop the rest of the interface working
