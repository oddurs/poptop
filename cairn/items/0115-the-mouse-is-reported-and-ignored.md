---
id: 115
title: The mouse is reported and ignored
type: feature
status: done
milestone: v3.6
created: 2026-09-15
updated: 2026-09-15
priority: p1
area: ui
depends_on:
- 114
---

## Problem

Every terminal poptop is likely to run in reports mouse events, and poptop does
not enable them. Clicking a menu title does nothing. Clicking a process row does
not select it. Dragging on the timeline does not scrub.

The menu bar made this louder rather than quieter: a bar you can see and cannot
click reads as a bar that is broken.

## The fix

`EnableMouseCapture`, and four bindings that are all obvious:

- Click a menu title to open it; click an item to run it; click away to close.
- Click a process row to select it.
- Click or drag the timeline to scrub to that column.
- Wheel to scroll the table, shift-wheel to zoom.

## What needs deciding

- **Selection versus mouse capture.** Capturing the mouse takes terminal text
  selection away, which is how people copy a PID. A modifier usually restores it
  (most terminals: hold Shift), and that has to be said somewhere rather than
  discovered.
- **Whether to make it a setting.** `mouse = on | off`, defaulting on, is the
  cheap answer to anyone whose terminal handles it badly.

## Acceptance criteria

- [x] A menu can be operated entirely with the mouse
- [x] Clicking the timeline scrubs to that moment
- [x] Text selection is still possible, and how is stated
- [x] A terminal that reports no mouse loses nothing
