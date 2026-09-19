---
id: 112
title: The default table spends its rows on copies of one program
type: feature
status: backlog
milestone: r5
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
