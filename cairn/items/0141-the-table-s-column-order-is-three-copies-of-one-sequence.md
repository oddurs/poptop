---
id: 141
title: The table's column order is three copies of one sequence
type: feature
status: backlog
milestone: v1.1
created: 2026-09-20
updated: 2026-09-20
priority: p3
effort: l
area: ui
---

## Problem

0139 added `hide-columns`, which drops a column. It could not reorder one, because the order is not data anywhere:

- cells are pushed in a fixed sequence in three render paths — processes, threads, and folded rows — and the three must agree or a row's cells land under the wrong headers;
- `Columns::widths` lists the widths in that same sequence;
- `Columns::fit` drops columns in a hand-written order of usefulness when the terminal is narrow;
- the header row is built to match, separately.

A reader who wants `PID` first, or the sparkline beside the CPU figure, has nothing to say.

## Proposal

One list of columns, each with its header, width, and how to draw a cell for a process, a thread and a folded row. The three render paths walk that list; `widths` and the header are derived from it; `fit` keeps its own order of usefulness, which is a different question from the order they are drawn in. Then `columns = pid, cpu, command` becomes a setting, and `hide-columns` is the same thing said the other way.

The risk is the table, which every other feature draws through: the ui tests are the check, and they are thorough.

## Acceptance criteria

- [ ] One declaration per column; the three render paths and the header all read it
- [ ] `columns` sets the order and the set; an unknown name warns and is ignored
- [ ] Every existing ui test passes unchanged with the default order
- [ ] `hide-columns` keeps working, or is documented as replaced
