---
id: 111
title: Columns spend width on nothing while the command is squeezed
type: feature
status: done
milestone: r5
assignee: Oddur Sigurdsson
labels:
- ui
created: 2026-09-19
updated: 2026-09-19
priority: p2
effort: m
area: table
---

## Problem

At 120 columns:

- DISK R and DISK W take 20 columns that are mostly `0`.
- USER takes 11 columns, and every visible row says `oddurs`.

Meanwhile the command, the column people actually read, is elided to `agent --long-optio…-other-option`, and at 80 columns to `agent …option`. The USER column folds when every process has the same user (0043). But it folds based on all 729 processes, not the rows on screen, so a single root daemon anywhere keeps it open.

## Proposal

- Fold USER when every visible row shares a user, and put `all oddurs` in the title, as the fixture frame already does.
- Show zero disk rates as blank rather than `0`, and let the pair collapse when every visible row is zero.
- Give the command whatever width that frees.

## Acceptance criteria

- [x] With every *known* owner the same user, the USER column is gone and the title names the user, counting the owners it doesn't know (was: "every visible row owned by one user"; see the note)
- [x] At 80 columns the command gets back the columns this table can honestly give up: USER's eleven when it folds, and the history column's eleven when nothing moved (0110). Not a fixed 30 (see the note)

## How it was resolved

**Why USER never folded on a Mac:** it wasn't visible rows. Every process this user may not read comes back from the macOS collector with owner `?`, 201 of them on this machine, and each counted as a second user. An unknown owner isn't a second user. After 0107 those rows show dashes in every figure, so the row itself already says it couldn't be read. `App::one_user` now ignores unknown owners, and the title counts them rather than claiming more than it knows: `· all oddurs but 201 unknown`. A known second user still keeps the column.

Live on this Mac the column correctly stays: one `root` process is readable (572 oddurs, 1 root, 201 unknown), and the column is what shows that. `owners_the_kernel_would_not_name_do_not_keep_the_user_column_open` fails on the old code.

**Decided against, for the reason 0110 learned:** "fold when every *visible* row shares a user" and "collapse the disk pair when every *visible* row is zero". A column that depends on which rows are on screen changes the table's shape when you scroll. `scrolling_the_list_does_not_rescale_everybody_else_history` caught exactly that in 0110, and `one_user` already refuses it ("Unfiltered on purpose…"). Buffer-wide, a live machine always has some process doing IO, so a collapse rule would never fire.

**Zero disk rates stay a dim `0`.** A zero is a measurement, and a blank would read as missing. `·` already means "not collected" in those columns, so it can't also mean zero.

**The command width:** the fixed 30 characters at 80 columns wasn't honest to promise. The command gets back USER's eleven columns when it folds and the history column's eleven when nothing moved (0110). The remaining columns are the table's figures, and the width ladder (0105) already gives them up first when the terminal can't hold them.
