---
id: 216
title: The sorted column does not descend
type: bug
status: backlog
milestone: r8
labels:
- ui
created: 2026-09-20
updated: 2026-09-20
priority: p0
---

## What happens

The header marks `▾CPU%` and the strip says `sort CPU`, and the column reads,
top to bottom, on a live capture:

```
13.4  5.8  3.5  4.2  3.9  3.0  6.6
```

Four inversions. Reproducible on six processes with ordinary jitter — nothing
pathological, no anti-correlated pair.

## What should happen

A table is monotonic in the column it says it is sorted by. That contract is
more fundamental than any of the calm the ordering is trying to buy: once the
sort column visibly does not sort, the reader has no way to trust the sort, and
then no way to trust the table.

## Where it came from

Smoothing was split so the figure could be live while the ordering stayed
calm — `Smoothing::cpu` ends at the cursor, `Smoothing::settled_cpu` ends on a
beat, and `Sort::compare_with` uses the second. The split fixed a real bug (the
figures were up to a whole window stale, showing `5.0` through a 90% spike and
`56.0` for five seconds after it) and introduced this one. Two readings of the
same quantity, one shown and one sorted by, cannot both be on screen.

## Proposal

Sort by the figure that is displayed. The calm then comes from the width of the
averaging window, which is a setting the reader already has, and from the
weighting — a spike moves a weighted mean far less than it moves a raw sample.

What that gives up is the case `the_order_holds_still_between_boundaries_and_moves_on_them`
was written for: two processes that genuinely take turns at 90/10 will trade
rows. They are taking turns; the table saying so is not a defect.

## Acceptance criteria

- [ ] At every width and every smoothing window, the sort column is monotonic
- [ ] A test walks the drawn rows and fails on an inversion, rather than
      testing the comparator in isolation
- [ ] The staleness this replaced does not come back: the figure still tracks
      a spike while it runs
