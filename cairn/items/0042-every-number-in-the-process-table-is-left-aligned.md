---
id: 42
title: Every number in the process table is left-aligned
type: bug
status: backlog
milestone: v1.0
created: 2026-09-07
updated: 2026-09-07
priority: p1
area: ui
---

## What it looks like

```text
PID     USER       CPU%         RSS           S  THR
81977   oddurs     103.4  ████+ 6.5G     █▏   R  33
5531    oddurs     21.3   ▉     62.8M         S  24
13665   oddurs     6.1    ▎     5.4M          R  2
26622   oddurs     5.9    ▎     422.9M   ▏    S  41
```

Every number is left-aligned. `103.4`, `21.3`, `6.1` do not share a decimal
point; `6.5G`, `62.8M`, `5.4M` do not share a unit position. To compare two
rows you have to read both numbers rather than see which is longer.

## Why it matters here more than elsewhere

Scanning a column for the largest value is the single most common thing anyone
does with this table. Right alignment is what makes magnitude a *visual*
property — digits line up, longer numbers stick out to the left, and the eye
finds the outlier without reading. Left alignment throws that away and leaves
the reader parsing eleven strings.

The bars beside `CPU%` and `RSS` partly compensate, which is probably why this
survived: they carry magnitude when the numbers cannot. But `RSS`'s bar is four
columns and the numbers carry mixed units, so `6.5G` and `62.8M` differ by a
factor of a hundred and look nearly the same length.

**The header already does this correctly** — `format!("{:>5.1}%", …)` — so the
two panels disagree about the convention, and the one where column scanning
matters most is the one that is wrong.

## What should happen

Numeric columns right-aligned: `PID`, `CPU%`, `RSS`, `THR`, and both disk rates.
Text columns stay left: `USER`, `COMMAND`, `S`. Column headers follow their
columns.

Bytes want their unit aligned too, not just their digits — `6.5G` and `62.8M`
read best when `G` and `M` share a column, which right alignment gives for free
since the unit is the last character.

## Acceptance criteria

- [ ] Numeric columns right-aligned, text columns left
- [ ] Headers aligned with their columns
- [ ] A test asserts a column of mixed-magnitude values shares a right edge
