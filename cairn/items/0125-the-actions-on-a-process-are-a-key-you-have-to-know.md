---
id: 125
title: The actions on a process are a key you have to know
type: feature
status: backlog
milestone: v4.0
created: 2026-09-15
updated: 2026-09-15
priority: p2
area: ui
depends_on:
- 114
- 123
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

## What needs deciding

- **Whether it lives on the row or in the key hints.** On the row it competes
  with the command name, which is the widest and most useful column. In the
  hints it is one more thing on a line that already has eight.
- **What "cannot" looks like.** Signalling a process from a recorded day is
  already refused; the affordance should say so before the attempt rather than
  after.

## Acceptance criteria

- [ ] What can be done to the selection is visible without a menu
- [ ] What cannot be done says so before it is attempted
- [ ] Nothing appears when nothing is selected
