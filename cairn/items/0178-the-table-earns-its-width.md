---
id: 178
key: r8
title: The table earns its width
type: milestone
status: backlog
labels:
- ui
created: 2026-09-20
updated: 2026-09-20
priority: p2
---

UI sprint, from a critique of the process table alone: the 0.2.0 build on a
ten-core Mac, captured through a terminal emulator at 120×24 with 761 real
processes on the machine, and again at 120 and 80 columns against a fixture
with a container, a folded pair, an unreadable process and a command line long
enough to be elided.

The first five items are the table saying something untrue: a sort that does
not sort, a count that counts the wrong thing, one glyph meaning two things, a
column with two types in it, and a name truncated to two letters. The next five
are width and rows spent on nothing — dots where figures would go, the same
picture in every row, and chrome outweighing the data on the commonest terminal
there is. The last three are design decisions about what a row is for.

The through-line, and the reason this is a sprint rather than five fixes: the
table is a spreadsheet with a fixed column set chosen by tab, and poptop's one
structural advantage — a complete history of every process — reaches it as a
ten-cell sparkline that is the first column dropped for width.
