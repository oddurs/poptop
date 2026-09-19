---
id: 107
title: The tree view opens on a page of question marks on macOS
type: bug
status: backlog
milestone: r5
labels:
- ui
created: 2026-09-19
updated: 2026-09-19
priority: p1
effort: s
area: ui
---

## What happens

Pressing `t` as a normal user on macOS fills every visible row with root-owned daemons that poptop can't read: `cfprefsd`, `trustd`, `backupd`, `automountd`… each showing `0.0  0B  ?  —  —  ?`. No tree lines are visible, and none of the user's own processes appear on the first screen. The view looks broken the moment it opens.

The row also contradicts itself. Memory reads `0B` and CPU `0.0`, while threads and disk read `—`. A process poptop can't read isn't one using zero bytes.

## What should happen

- A value poptop can't read shows `—` in every column, memory and CPU included.
- The tree is ordered so the first screen shows something: branches that contain the user's own processes, or the most active branches, first. Unreadable leaves go last, or fold into a single "N processes not readable without root" row.
- The tree lines are visible from the first row.

## Reproduction

1. Run poptop on macOS as a normal user.
2. Press `t`.
