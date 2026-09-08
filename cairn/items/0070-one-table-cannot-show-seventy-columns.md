---
id: 70
title: One table cannot show seventy columns
type: feature
status: backlog
milestone: v2.1
depends_on:
- 69
created: 2026-09-08
updated: 2026-09-08
priority: p1
area: ui
---

## Problem

atop solves this with views: `g` generic, `m` memory, `d` disk, `n` network,
`s` scheduling, `v` various, `c` command line — each a different column set over
the same rows, switched with one key.

poptop has one column set, and 0043 had to fold a column away to make room for
the command. Adding the fields in 0069 to a single table is not possible; the
table is already at its width on an eighty-column terminal.

## Why this is the right shape rather than more columns

The existing table already has most of the machinery: `Unit` describes how a
figure is measured, `num()` right-aligns it, the header ladder decides what to
drop, and 0043's folding decides what is worth space. A view is a named list of
columns over that machinery, not a new renderer.

It also fixes something present today: `DISK R`/`DISK W` are only shown when
they fit, so the disk figures vanish on a narrow terminal with nothing to bring
them back. A disk view is that key.

## What needs deciding

- **How a view interacts with sort.** atop keeps them independent; sorting by a
  column the current view does not show is possible and confusing. 0047's
  constraint suggestion already picks a sort, and would want to pick the view
  with it.
- **Whether the modes collapse.** Tree, grouping and detail are three exclusive
  modes reached by three keys; views are a fourth axis. That is either one
  concept — what the table is *of*, and what it *shows* — or four keys that
  interact pairwise, which is where interfaces go wrong.
- **Whether a view is user-definable.** atop has `o`, a user-defined line, and
  poptop already has a config file.

## Acceptance criteria

- [ ] Column sets switchable with one key, sharing one renderer
- [ ] The disk columns are reachable on a narrow terminal
- [ ] Sort and view interact predictably, and the panel says which is active
- [ ] The mode axes are resolved rather than multiplied
