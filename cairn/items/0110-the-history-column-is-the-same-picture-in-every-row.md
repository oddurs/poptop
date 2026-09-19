---
id: 110
title: The history column is the same picture in every row
type: feature
status: backlog
milestone: r5
labels:
- ui
created: 2026-09-19
updated: 2026-09-19
priority: p2
effort: s
area: table
---

## Problem

HIST is 10 columns wide, and in every capture all 23 rows showed the same flat `⠀⠀⢀⣀⣀⣀⣀⣀⣀⣀`. Every row shares one scale, set by whichever process is on top (`≤25%`, `≤100%`, `≤200%` across captures). Most processes sit near zero on that scale, so the column shows a flat line in 23 rows while using ten columns of the table's width.

## Proposal

Either scale each row to its own peak, with the header saying so, or hide the column when no row varies. Grouped rows (`g`) show no history at all; they could show the group's combined history.

## Acceptance criteria

- [ ] Two processes with different CPU shapes draw visibly different lines
- [ ] The header says what the scale is
- [ ] Grouped rows show a history, or the column says why not
