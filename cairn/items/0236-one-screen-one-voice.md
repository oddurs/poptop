---
id: 236
key: r10
title: One screen, one voice
type: milestone
status: backlog
labels:
- ui
created: 2026-09-23
updated: 2026-09-23
priority: p1
---

UI sprint. Not new features and not the table's columns — r8 has those. This
is about how the screen *reads*: the ink poptop spends on furniture against
the ink it spends on data, and whether the parts look like they were drawn by
one hand.

From a 120×24 capture, the size most terminals open at. The loudest ink on the
screen is a full-width rule with a sentence inside it, and there are two of
them. The number of processes is stated three times in three consecutive rows.
The column header speaks three vocabularies at once — `▾CPU%`, `S`, `HIST
≤800%` — and one of them changes width as the machine changes, so the header
shifts under the reader. The sentence explaining that the buffer is still
filling is printed across the middle of a graph row. Every kind of row starts
in a different column.

None of that is wrong. All of it is noise, and noise is what makes a terminal
program look like it was assembled rather than designed.

The rule this milestone works to: **structure is quiet, data is loud, and a
thing is said once, where it is needed.** A divider names a region and says
nothing else. A message appears where the thing it is about is. A number
appears in one place. Everything on a line starts on the same column as the
line above it.
