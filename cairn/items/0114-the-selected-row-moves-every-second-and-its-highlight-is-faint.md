---
id: 114
title: The selected row moves every second and its highlight is faint
type: feature
status: backlog
milestone: r5
labels:
- ui
created: 2026-09-19
updated: 2026-09-19
priority: p3
effort: s
area: table
---

## Problem

Sorted by CPU, the table reorders every sample, so a selected process jumps between positions. Between two captures one second apart, the selection moved two rows. The highlight is a dark grey background (`#3a3a3a`) plus bold. On most themes that's easy to lose among rows that are moving anyway.

## Proposal

- A stronger highlight: a marker in the left margin, reverse video, or the theme's accent colour, still readable in monochrome.
- Possibly: while a process is selected, keep its row still and let the others move around it.

## Acceptance criteria

- [ ] The selected row is identifiable in the monochrome tier
- [ ] A decision on whether the selected row stays put, recorded here
