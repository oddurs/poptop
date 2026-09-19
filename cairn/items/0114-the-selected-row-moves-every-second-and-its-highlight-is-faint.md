---
id: 114
title: The selected row moves every second and its highlight is faint
type: feature
status: done
milestone: r5
assignee: Oddur Sigurdsson
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

- [x] The selected row is identifiable in the monochrome tier (it already was: reverse video; now it is marked in the margin as well)
- [x] A decision on whether the selected row stays put, recorded here

## How it was resolved

**A mark in the margin.** The table now has a one-column margin on the left, like an editor's sign column. The selected row gets `▶` there, in the theme's accent colour, where the eye starts reading a row (`>` on the ASCII glyph tier). The row's own highlight is unchanged, so on a colour terminal the selection is carried by the mark and the background together. On a monochrome terminal it was already reverse video, which is what the first criterion asked for. The mark follows the row's place in the scrolled list and isn't drawn when nothing is selected.

The margin costs one column, taken from the command through the same width ladder as everything else (0105). Two tests calibrated to the exact width at which a long Chrome name elides legibly now use one column more, which gives the same command width as before. The README's sample table was regenerated from what poptop draws, which is the old table shifted right by the margin.

**Decided: the selected row doesn't stay put.** The table's order is its meaning. It's sorted by CPU, and pinning one row where it was would make the order lie about that row and every row it displaced. The viewport already keeps the selection on screen as it moves (`last_row_to_keep`), and with the mark it's easy to follow.

Tested in `the_selected_row_is_marked_in_the_margin`: exactly one marked row, and it's the selected process; `>` in ASCII; no mark with nothing selected.

Checked for cost while here: an idle instance used 0.14–0.17 s of CPU over 15 s, both before and after the change.
