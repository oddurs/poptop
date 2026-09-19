---
id: 97
title: Performance budgets live in ignored tests nobody runs
type: chore
status: done
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

- [x] Budgets written down, each with the measurement it was set from
- [x] A bench or `./check --perf` that fails on a budget breach
- [x] The three ignored measurement tests are replaced by it or linked to it

## How it was resolved

`src/budget.rs` holds six budgets as ignored tests, each with a comment stating the figure it was set from. All were measured on 2026-09-19, release builds, on an M-series Mac and in a Linux arm64 container on the same machine. A time gets about three times the slower of the two; a size, which does not vary, gets a fifth more. The same table is in the README under "What it costs to watch".

- **Collecting**, the whole sample (25 ms) and per process (30 µs), on the live machine. The per-process figure is only held from a hundred processes up: the container's five cost 0.17 ms, all of it fixed reads, which is 34 µs "a process" and means nothing.
- **Drawing** a 200×60 frame of 900 processes over 600 samples (25 ms). This replaces `measure_render_with_sparklines`.
- **The store**: encoding (90 ms) and decoding (35 ms) a full buffer of 600 × 400, and its size (30 MB, measured at 25.0 MB). The earlier "9.3 MB" in 0059's notes was not this configuration.
- **Cold start**, from a full store's bytes to the first frame (35 ms). This replaces `show_startup_cost_with_a_full_store`, which measured only the decode.
- **Memory after an hour** at 400 processes into the default buffer: 100 MB total, and 8 MB for the fifty minutes after the buffer filled, which measured 96 KB and 0, so flat. It runs in a process of its own. Beside the other budgets it measured 1.5 MB for the hour, because it reused the pages they had freed.
- **NFS**: 100 NFSv4.2 mounts' `mountstats` with 71 per-op lines each (1.5 ms, measured 0.5 ms), on Linux. This replaces `cost_of_nfs`. That test's own warning about fixtures with three per-op lines still applies, and is why the text is built at full width.

`./check --perf` runs them in release, one at a time, fails on a breach and prints the table. CI's test job runs them with `POPTOP_BUDGET=report` on both platforms, which prints the table and fails nothing. A budget run in a debug build fails and says to use `--release`.

