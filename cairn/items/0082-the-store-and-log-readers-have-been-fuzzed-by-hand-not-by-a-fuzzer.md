---
id: 82
title: The store and log readers have been fuzzed by hand, not by a fuzzer
type: chore
status: backlog
milestone: r1
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

- [ ] Fuzz targets for `store::decode_reporting`, the schema block, and `log::open_day`
- [ ] Each runs for at least an hour with no crash, hang (over 1s per input), or allocation over 1 GiB
- [ ] Every finding is fixed and pinned by a unit test that fails without the fix
- [ ] `./check` documents how to run the fuzzers; CI runs each for a short, bounded time
