---
id: 111
title: Columns spend width on nothing while the command is squeezed
type: feature
status: backlog
milestone: r5
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

- [ ] With every visible row owned by one user, the USER column is gone and the title names the user
- [ ] At 80 columns the command gets at least 30 characters
