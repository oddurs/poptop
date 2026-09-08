---
id: 71
title: Work accumulates by program but not by user or container
type: feature
status: done
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

- [x] Accumulate by user, by program and by container
- [x] The identity column shows the key, not a member's value
- [x] The same refusals as 0050: nothing summed that cannot be
- [x] Selection follows a group across samples, as it does for programs

## How it was resolved

PR #88. `g` cycles off, by name, by user, by container — atop's `p`, `u` and `j`
on one key rather than three. 0062 added the container key; this adds the user
one, and the arithmetic was 0050's unchanged, exactly as the item predicted.

**The identity inverts with the key.** Grouping by user makes the username the
row's name, so the `USER` column is dropped — drawing it beside the identity is
the same word twice. That is the opposite of 0043's rule, which folds a column
away when every row *shares* a value.

**Review found the selection was broken for two of the three keys.**
`watched_but_absent` resolved a group against the process name, so a username or
container id matched nothing and the panel reported a running user as absent —
the false claim the function exists to prevent. It went through `Grouping::key`
after that, so the lookup and the row that built it cannot disagree.

Also found: macOS falls back to `?` for an unlookupable uid, and folding on a
placeholder heaps every such process into one row presented as one user's usage.

**Not done:** 0047's suggestion does not follow the grouping. The item phrases
it as a *could*, and it means deciding what happens when the reader is already
grouped by something else — left alone rather than guessed at.
