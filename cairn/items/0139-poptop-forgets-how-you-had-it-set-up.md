---
id: 139
title: poptop forgets how you had it set up
type: feature
status: done
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

- [x] Each starting state is settable in the config and by flag, and matches what the key produces
- [x] `hide-columns` drops columns; the width freed goes to the command. Reordering is filed as 0141, with the reason
- [x] A column a platform cannot fill is unchanged: it already renders as an em dash, and hiding is the reader choosing on top of that
- [x] The defaults are exactly today's, proven by a test against a fresh `App`

## How it was resolved

**The starting state**, seven settings, each taking the values its key cycles through: `view`, `sort`, `zoom`, `tree`, `group`, `kernel-threads`, `io-columns`. `Sort`, `View` and `Grouping` gained a `parse` beside their labels, so the name in the file is the name on screen. The defaults are exactly what `App::new` produces, which a test asserts against a fresh app rather than against a list written by hand.

Two combinations cannot hold, and both warn and carry on rather than refusing to start:

- `tree` with `group`, which the keys already make exclusive — a grouped tree is a tree of things that are not processes.
- a `sort` the starting `view` cannot show, which would be an ordering with nothing on screen to explain it. It falls back to that view's own first sort, naming both.

**`hide-columns`** takes column names — `bars`, `rss`, `state`, `thr`, `io`, `mem`, `hist`, `pid`, `user`, `cid` — and drops them before the width ladder runs, so the room they would have taken goes to the command rather than to whatever the ladder would have dropped next. A rendering test hides two columns and checks both that they are gone and that `COMMAND` starts further left.

**Reordering was not done, and is filed as 0141.** The order is not data anywhere: cells are pushed in a fixed sequence in three render paths (processes, threads, folded rows), the width ladder drops columns in a hand-written order of usefulness, and the header is built to match. Making the order a list means rewriting the table renderer, which is the one piece every other feature draws through. That is worth doing deliberately, not as a rider on a settings change.

**Not every column can be hidden.** The command is what a row *is*, and the CPU figure is what the table is sorted by and drawn for. `Column::ALL` is the list of the ones that can go without leaving a row that cannot be read.

**Tests:** every setting from a file; the defaults against a fresh `App`; each bad value naming what it takes and keeping the default; the rendering test above; and `tests/cli.rs` for the file reaching `--config`, the `tree`/`group` warning, and a bad value warning while poptop still runs. `docs/reference/configuration.md` documents all eight.

**Fixed on the way past:** `--write-config` wrote `hide-columns = ` with an empty value when nothing was hidden, and poptop refuses to read a line with no value — so the file it wrote was not one it could read. A setting with nothing to show is now written as a comment.

