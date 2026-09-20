---
id: 223
title: Chrome outweighs the table on the commonest terminal there is
type: feature
status: backlog
milestone: r8
labels:
- ui
created: 2026-09-20
updated: 2026-09-20
priority: p1
---

## Problem

Counted from a 120×24 capture, which is the size most terminals open at:

```
row  0  menu bar
row  1  header figures
row  2  per-core meters
row  3  tab strip
row  4  panel rule
row  5  summary strip
row  6  column headers
rows 7-13   seven processes
row 14  timeline rule
rows 15-21  seven graph rows
row 22  the timeline's caption
row 23  action bar and key hints
```

Ten rows of furniture, fourteen of data. 42% of the screen is chrome, and
three consecutive rows — strip, rule, summary — stand between the reader and
the first process.

0034 filed this at "a third of the screen" and it has got worse since, most
recently by moving the tab strip onto the table (which was right, and still
cost nothing new — but the summary strip and the rule both arrived in the same
milestone).

## Proposal

Not a single cut. The three rows above the table each say something, and the
question is which of them says it where it is already being said:

- the rule's subject duplicates the scope line (0220)
- the summary strip's `N shown` duplicates both (0220)
- the column headers and the rule could share a row on a short terminal, as
  the axis and the caption already do in the timeline

Density is the other half: `spacious` should be what buys the third row, and
`compact` should give it back. Today every density draws all three.

## Acceptance criteria

- [ ] At 24 rows the table gets more rows than the chrome above and below it
- [ ] Nothing that only one row says is lost to get there
- [ ] `--density=compact` measurably buys process rows
