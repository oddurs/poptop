---
id: 50
title: Twelve rows of the same program crowd out everything else
type: feature
status: backlog
milestone: v1.0
created: 2026-09-07
updated: 2026-09-07
priority: p2
area: ui
---

## The lesson

bottom groups processes with the same name behind `Tab`: one row per name, the
PID column showing how many were folded in.

poptop's own interface survey shows exactly the case it exists for — twelve of
the fifteen visible rows were the same program:

```text
26622   oddurs   5.9   422.9M  S  41   claude
91942   oddurs   4.6   457.2M  S  40   claude
90394   oddurs   3.6   405.8M  S  26   claude
93272   oddurs   3.5   468.0M  S  40   claude
32359   oddurs   3.4   287.4M  S  39   claude
21159   oddurs   3.2   256.6M  S  40   claude
```

Six rows to say one thing. Individually each is unremarkable; together they are
21% of a core and 2.3GB, which is the fact worth knowing and the one the table
cannot state. The same happens with browser helpers, worker pools, and anything
that forks per request.

## Why it is an interface fix as much as a feature

The table has around a dozen rows on a normal terminal. Spending half of them on
one program is the same waste as
[0043](0043-a-column-of-one-repeated-value-costs-as-much-as-a-column-of-information.md)
identifies in the `USER` column, one axis over: repetition that costs space and
conveys nothing.

## What needs deciding

- **What "same" means.** bottom groups by name. With
  [0048](0048-every-node-process-looks-the-same-because-the-table-shows-comm.md)
  done, grouping by name would fold together processes the reader has just been
  given the means to tell apart — so it probably wants to group on the name and
  show the distinguishing arguments in the expanded form.
- **What aggregates and what does not.** CPU, RSS and thread counts sum. State
  does not. A per-process sparkline does not — the group's history is not the
  sum of the members', because membership changes as processes come and go, and
  that is exactly the kind of fabricated continuity this codebase refuses
  elsewhere.
- **Whether it is a mode or a default.** A mode is safer and is what bottom
  chose.

## Acceptance criteria

- [ ] One row per program, with the count of processes folded into it
- [ ] Figures that sum are summed; figures that do not are shown as absent
- [ ] The group's history is not fabricated from changing membership
- [ ] Tree mode and grouping cannot both be on, as in bottom
