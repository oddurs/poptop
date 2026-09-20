---
id: 209
title: The actions on a process are a key you have to know
type: feature
status: done
milestone: v4.0
created: 2026-09-15
updated: 2026-09-15
priority: p2
area: ui
depends_on:
- 198
- 207
---

## Problem

Activity Monitor puts three controls in its title bar — quit, inspect, more —
and they are attached to the selection. They are visible, they are few, and
which of them are available tells you what can be done to what you have picked.

poptop has `x`, `X`, `d`, and a menu that lists them. The menu was the right
move and it is one level of nesting away from the row you are looking at.

## The fix

An action affordance on the selected row itself — the terminal's version of the
title-bar buttons — showing the two or three things that can be done to it and
greying what cannot:

```
  88.4 ███▌    512.0M ▏  S   4   824 postgres        ⏎ inspect · x quit
```

Attached to the selection, so it appears when something is selected and says
nothing when nothing is.

## What was decided

**The footer, not the row.** The footer already changes with the mode — a
filter error, a jump note, a pending confirmation — and a selection is a mode.
On the row it would compete with the command name, which is the widest and most
useful column.

**It is last in that row's ladder.** A pending confirmation, a jump note and a
filter error are about something the reader just did; this is about something
they are still looking at, so the question wins the row.

**"Cannot" needed the reason to have two lengths.** `Refused::Scrubbing`
is a hundred and twenty characters, written for a row with nothing else on it,
and pasting it into a shared row dropped the whole bar including the name of the
thing selected. `Blocked` is the reason rather than the sentence, with
`short()` for the footer and `why()` for the note — one decision about
whether, two lengths of it.

## Acceptance criteria

- [x] What can be done to the selection is visible without a menu
- [x] What cannot be done says so before it is attempted
- [x] Nothing appears when nothing is selected
