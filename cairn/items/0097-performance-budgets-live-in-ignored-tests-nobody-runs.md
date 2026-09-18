---
id: 97
title: Performance budgets live in ignored tests nobody runs
type: chore
status: backlog
milestone: r4
labels:
- validation
created: 2026-09-18
updated: 2026-09-18
priority: p2
effort: m
area: perf
---

## Problem

`measure_render_with_sparklines`, `show_startup_cost_with_a_full_store` and `cost_of_nfs` are `#[ignore]` measurements. 0038, "Cheap enough to run", was a milestone about cost. Nothing fails if sampling, rendering, encoding or startup gets twice as slow.

## Proposal

State a budget for each hot path: collect at 400 processes, render a frame, encode and decode a full store, cold start with a full store, and resident memory after an hour. Measure each with a benchmark harness (`criterion`, or a plain timed loop with a generous margin), and have `./check` fail when a budget is exceeded. CI timing is noisy, so CI should report the measurements without enforcing them.

## Acceptance criteria

- [ ] Budgets written down, each with the measurement it was set from
- [ ] A bench or `./check --perf` that fails on a budget breach
- [ ] The three ignored measurement tests are replaced by it or linked to it
