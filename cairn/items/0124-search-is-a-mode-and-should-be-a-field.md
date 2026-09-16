---
id: 124
title: Search is a mode and should be a field
type: feature
status: done
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

## What was decided

**Two Escapes was not built.** Escape does the thing every other program uses it
for: it puts back the filter that was there before the field was opened. Enter
keeps what was typed. Both used to commit, so Escape was a second Enter — and
spending the universal undo key on "finish" is the surprising part, not the
absence of a second press.

**Clearing is the menu's `Clear filter`**, which is one action and is named
rather than guessed.

**The field is the scope line.** While the filter is being typed it *is* the
scope, so the query is shown there and nowhere else — the footer, which used to
hold a box echoing it, now carries what the field takes and how to leave it,
which is the only thing the box did that the field does not.

**`/` keeps what is already there.** It used to clear the filter before a key
was pressed, so narrowing a narrowed list meant retyping the first query.

## Acceptance criteria

- [x] A filter that is set is visible without pressing anything
- [x] Clearing it is one obvious action, not a guess
- [x] Typing in it does not stop the rest of the interface working
