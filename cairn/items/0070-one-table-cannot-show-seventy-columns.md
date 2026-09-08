---
id: 70
title: One table cannot show seventy columns
type: feature
status: done
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

- [x] Column sets switchable with one key, sharing one renderer
- [x] The disk columns are reachable on a narrow terminal
- [x] Sort and view interact predictably, and the panel says which is active
- [x] The mode axes are resolved rather than multiplied

## How it was resolved

PR #83. `v` cycles generic, memory, disk — a named list of columns over one
renderer.

**The concrete fix:** `DISK R`/`DISK W` are shown only when they fit, so on a
narrow terminal they vanish with nothing to bring them back. In the disk view
they are the point, so they are exempt from that width test, and the view makes
room by dropping the bars and the thread count rather than pushing the command
off the edge.

**Sort and view cannot disagree.** `s` cycles within the current view's columns,
and switching views brings the sort along when the new one cannot show it. atop
keeps them independent, which allows an ordering the reader cannot see the
reason for. The panel names both.

**The axes, resolved:** what the table is *of* (flat, tree, folded by name or
container, expanded to threads, or cgroups) and what it *shows* (the column
set). `d` is neither — it changes the timeline panel, and counting it as a table
mode is what made this look like four axes.

**Not user-definable yet.** atop has `o` and poptop has a config file, so the
hook exists — but a user-defined column list wants the column descriptors 0069
will produce, and inventing them now would mean inventing them twice. The three
views are thin for the same reason; memory gets interesting when 0069 lands PSS,
swap and fault counts.

**Self-reviewed**, the review agent having stalled twice. It found one real bug:
switching to the disk view sorted by `Sort::Disk` even where the column is not
collected — the shuffle `Sort::next` already refuses. And it produced the test
this refactor most needed, which asserts the command cell begins under its
header across every view and row shape, since a header/cell mismatch misaligns
silently rather than panicking.
