---
id: 82
title: The store and log readers have been fuzzed by hand, not by a fuzzer
type: chore
status: done
milestone: r1
assignee: Oddur Sigurdsson
labels:
- validation
created: 2026-09-18
updated: 2026-09-18
priority: p1
effort: m
area: store
---

## Problem

`store::decode_reporting` and `log::read_day` / `log::open_day` read files that outlive the process that wrote them. Those files can be truncated by a crash, damaged by a full disk, or written by a different poptop. The existing defence is `no_single_corrupt_byte_can_panic_the_reader` (`src/store.rs:810`), which covers one mutation at a time. The three hang and crash bugs 0059 found in the schema block were found by reasoning about the format. No tool found them, and a self-referential record through a list escaped the first fix.

## Proposal

Add `cargo-fuzz` targets for the store decoder, the schema-block parser on its own, and the daily-log reader. Seed each corpus from files written by the current code at several sizes. Crashes found by the fuzzer become ordinary regression tests. The fuzzer is not the regression test.

## Acceptance criteria

- [x] Fuzz targets for `store::decode_reporting`, the schema block, and `log::open_day`
- [x] Each runs for at least an hour with no crash, hang (over 1s per input), or allocation over 1 GiB
- [x] Every finding is fixed and pinned by a unit test that fails without the fix
- [x] `./check` documents how to run the fuzzers; CI runs each for a short, bounded time

## How it was resolved

Eight targets in `fuzz/`, reached through `src/lib.rs`. That file compiles the program's modules into a library only when the `fuzzing` feature is on, so the normal build is unchanged.

Store and log targets: `store` and `store_body` (the latter starts past the header, so every input reaches the schema block and the merge path), `log_day`, and `log_torn`. `log_torn` checks an answer rather than only the absence of a panic: every whole entry comes back, and no torn checksummed one does. The others are `proc_files`, `nfs`, `taskstats` (Linux; 0083) and `typed` (0084).

**One hour each, on the final code, with no crash, hang or allocation over 1 GiB:**

| target | inputs |
|---|---|
| store | 14.0M |
| store_body | 20.2M (plus 9.0M in an earlier 20 min) |
| log_day | 5.0M |
| log_torn | 1.4M |
| typed | 4.2M |
| proc_files (Linux) | 1.5M (after an earlier 30 min) |
| nfs (Linux) | 39.4M |
| taskstats (Linux) | 85.0M |

Peak RSS stayed under 550 MB throughout. Five runs stopped on the 1 s timeout while the machine ran eight fuzzers, a container and builds at once. Replayed alone, each of those inputs took 2 to 6 ms. So the long runs use `-timeout=10`, and the replays are what showed nothing takes a second.

Everything the fuzzers found is fixed and pinned by a unit test. The store targets found nothing: the store survived 0059's own hardening. The torn-log findings are recorded on 0085 and 0100. `./check --fuzz[=SECONDS]` runs every target, `--linux` adds the Linux ones in a container, and CI runs each for a minute on Linux.
