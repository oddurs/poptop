---
id: 134
title: A process on its first sample reports 0.0% CPU, which is a fabricated zero
type: bug
status: backlog
milestone: later
labels:
- data
created: 2026-09-19
updated: 2026-09-19
priority: p2
---

## Problem

CPU per process is a delta between two samples. A process poptop is seeing
for the first time has no previous sample, and both backends answer `0.0`:

`src/collect/linux.rs:1565`

```rust
let cpu = match ctx.prev_jiffies.get(&pid) {
    Some(&prev) if elapsed_secs > 0.0 => { … }
    _ => 0.0,
};
```

`src/collect/darwin.rs:320`

```rust
cpu: if first { 0.0 } else { p.cpu_usage() },
```

That is the thing this project says it does not do. The same row, in the
same sample, gets it right for disk — a process with no previous sample
shows `—` for `DISK R` and `DISK W`, because the delta is unknown. CPU has
exactly the same missing input and prints a number.

It is visible in the frame in the README, which is how it was found: ten
`rustc` that the build had just spawned, each holding 140 MB, each reported
at `0.0` with `—` beside it in the disk columns. A reader sorting by CPU
during a fork storm sees the processes doing the work at the bottom of the
table.

## Proposal

`0.0` becomes "not yet known", drawn `—`, the way the IO columns already
do it. That is a type change — `cpu: f32` to `Option<f32>` — reaching both
backends, the sort, the filter, the export schema and the report, so it is
not a small change and it is not a drive-by.

Two things to decide with it:

- **The export schema.** `cpu` becoming nullable is a compatible change by
  the rules in `docs/design/recording-and-output.md`, but it needs a line in
  `CHANGELOG.md`.
- **Whether a first sample can do better than "unknown".** `/proc/<pid>/stat`
  carries cumulative jiffies and a start time, so an average over the
  process's whole life is available. That is a different measurement from
  the one the column states, so it should not quietly stand in for it — but
  it is worth considering for the detail panel.

## Acceptance criteria

- [ ] A process with no previous sample reports CPU as unknown, not zero
- [ ] It sorts with the unreadable rows, not among the idle ones
- [ ] The filter matches it in neither direction, as it already does for IO
- [ ] Both backends, with a test each
- [ ] The schema change is recorded in CHANGELOG.md
