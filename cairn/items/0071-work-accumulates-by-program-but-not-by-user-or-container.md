---
id: 71
title: Work accumulates by program but not by user or container
type: feature
status: backlog
milestone: v2.1
depends_on:
- 62
created: 2026-09-08
updated: 2026-09-08
priority: p2
area: ui
---

## Problem

0050 folds processes sharing a name into one row. atop does the same by
**user** (`u`), by **program** (`p`) and by **container or pod** (`j`).

On a shared box "which user is eating the machine" is the first question and
poptop cannot answer it: 0043 folds the `USER` column away precisely because it
is usually constant, which is the single-user case — and on the multi-user case
where it matters, it is one column of many rows.

## Why it is nearly free

`grouped()` already sums the figures and already refuses to sum what cannot be
summed — state, command line, IO with an unreadable member. It keys on
`p.name`. Keying on `p.user` or on a container id is the same function with a
different key, and `Watched::Group` already follows a group by name across
samples.

The work is choosing what the key is and saying so, not the arithmetic.

## What needs deciding

- **How the key is chosen.** A cycle on one key, or a mode per key. atop uses
  three keys and they are mutually exclusive.
- **What the identity column shows.** Grouping by user makes `USER` the identity
  and the command line meaningless — the folding rule from 0043 inverts.
- **Whether the constraint suggestion follows.** If disk is the constraint and
  one user is causing it, grouping by user is the useful answer and 0047 could
  say so.

## Depends on

Container attribution, for the third key.

## Acceptance criteria

- [ ] Accumulate by user, by program and by container
- [ ] The identity column shows the key, not a member's value
- [ ] The same refusals as 0050: nothing summed that cannot be
- [ ] Selection follows a group across samples, as it does for programs
