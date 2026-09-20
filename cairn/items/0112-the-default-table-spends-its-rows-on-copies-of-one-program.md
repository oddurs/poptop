---
id: 112
title: The default table spends its rows on copies of one program
type: feature
status: done
milestone: r5
assignee: Oddur Sigurdsson
labels:
- ui
created: 2026-09-19
updated: 2026-09-19
priority: p2
effort: m
area: table
---

## Problem

The default view showed 7 to 12 identical `agent --long-optio…-other-option` rows. `g` turns them into `agent ×19  18.1%  3.6G`, and 24 `plugin-container` rows and 99 `git` rows into one line each. That was the most readable screen of the critique, and it's off by default and not advertised.

## What needs deciding

- Group by default, or group automatically when one name fills more than some share of the visible rows.
- How a grouped row expands (Enter, or `→` on it) without losing the ability to select one process.
- Whether `x`/`X` on a grouped row signals the whole group, or refuses.

## Decided

**The default stays ungrouped, and grouping is offered when it would help.**

- **Group by default:** no. Signalling acts on one process, and a folded row isn't one, so `x` would stop working until the reader ungrouped. The pid is how you act on a process. The default view has to be the one you can act from.
- **Group automatically when one name fills the table:** no. That changes the table's mode under the reader. The title's existing suggestion states the principle: "Named, never imposed. A table that reorders itself under the reader is worse than one that does not."
- **What was done:** when one name has at least five processes in the displayed sample, and the table is neither grouped nor a tree, the title says so and names the key: `· 100 git (g folds them)` (live on this Mac). The count is taken over the whole sample, not the rows on screen, so scrolling doesn't make the offer come and go (the 0110 lesson). It's ranked just below the constraint suggestion it sits beside.
- **Expanding a group in place:** not built. `g` toggles the whole view in one key, and with the offer on screen that's a single keypress either way.
- **`x` on a grouped row:** already refuses, and says why: a folded row is selected but isn't a process.

Tested in `a_table_crowded_by_one_program_offers_to_fold_it`: offered for eight, not for three, not while grouped, not in the tree.
