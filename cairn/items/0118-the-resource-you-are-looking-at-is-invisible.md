---
id: 118
title: The resource you are looking at is invisible
type: feature
status: done
milestone: v4.0
created: 2026-09-15
updated: 2026-09-15
priority: p0
area: ui
depends_on:
- 114
---

## Problem

`v` cycles Generic → Memory → Disk. Nothing on screen says which one you are in
until you notice the columns changed, nothing says the others exist, and there is
no way to go back one except going forward twice.

Activity Monitor spends its most valuable strip of screen on this: a segmented
control, always visible, one word per resource, the current one lit.

## The fix

A tab strip below the menu bar:

```
  CPU   Memory   Energy   Disk   Network
 ▔▔▔▔▔
```

`Tab` and `Shift-Tab` move between them; `1`–`5` jump. Not `←`/`→`, which scrub
time and must keep doing so — the timeline is the thing poptop has that Activity
Monitor does not, and its keys come first.

The current tab is marked by an underline rather than by colour alone: five
meaning-bearing hues are already spent, and a tab strip that is invisible at the
mono tier is a navigation bar that fails on exactly the terminals a monitor is
most likely to be opened in.

## What needs deciding

- **Where it goes.** Under the menu bar is the obvious place and costs a row.
  The alternative — folding the tabs into the menu bar itself — saves the row
  and buries the primary navigation inside a dropdown, which is what this item
  exists to stop.
- **What happens on a short terminal.** The strip is the last row to drop, after
  the gutter and the key hints, and when it drops the tab name has to move into
  the scope line (119) rather than vanishing.
- **Whether `1`–`5` conflict.** They are currently free.

## Acceptance criteria

- [x] The resource on screen is named without pressing anything
- [x] The other resources are visible, so they can be discovered
- [x] Reachable by a key, by the menu and by the mouse
- [x] Legible at the mono tier
- [x] Does not take a key the timeline needs
