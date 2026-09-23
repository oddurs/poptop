---
id: 220
title: The row count is stated three times in three consecutive rows
type: bug
status: done
milestone: r10
labels:
- ui
created: 2026-09-20
updated: 2026-09-23
priority: p3
---

## What happens

```
 CPU   Memory   Disk   sort CPU · avg 5s        All processes · 761
── processes (761) · 31 fish (g folds them) ! io: 275/761 need root ──
 761 shown · CPU 80.5% · MEM 11.4G (71%)
```

761 appears in the scope, in the panel rule and in the summary strip — three
rows, one after another.

Separately, `761 shown` is not true: seven are shown. It means "761 are in the
list", which is what the scope line above it already said.

## What should happen

"What the column said, said once" is the rule the `USER` column folds under.
Three rows of chrome stating one number is the same waste with none of the
excuse.

## Proposal

The scope line owns the count — it is the line that may never disappear and it
is the one a filter changes. The rule drops `processes (N)` and keeps what only
it says: the omissions and the events. The summary strip drops `N shown` and
keeps the totals, which are the thing it is for and which nothing else states.
