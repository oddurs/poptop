---
id: 139
title: poptop forgets how you had it set up
type: feature
status: backlog
milestone: v1.1
created: 2026-09-20
updated: 2026-09-20
priority: p2
effort: l
area: ui
---

## Problem

The view, the sort column, the zoom, the tree, the grouping, kernel threads and the IO columns are all key presses, and every one of them resets on the next launch. Someone who always wants the memory view sorted by RSS with kernel threads shown has to press four keys every time. The columns themselves are fixed: a reader who never looks at `THR` cannot have the width back, and one who wants `PID` first cannot move it.

## Proposal

Settings for the starting state — `view`, `sort`, `zoom`, `tree`, `group`, `kernel-threads`, `io-columns` — each taking the same values the keys cycle through. A `columns` setting naming the table's columns in order, defaulting to today's, so a column can be dropped or moved. An unknown column name warns and is ignored, like every other bad line.

## Acceptance criteria

- [ ] Each starting state is settable in the config and by flag, and matches what the key produces
- [ ] `columns` reorders and drops columns; the width freed goes to the command
- [ ] A column that a platform cannot fill is still refused politely, not blank
- [ ] The defaults are exactly today's, proven by a test
