---
id: 58
title: Adding a metric costs four edits and throws away history
type: chore
status: done
milestone: v2.0
created: 2026-09-08
updated: 2026-09-08
priority: p0
area: collect
---

## Problem

`Sample` is a flat struct of nineteen fields and `ProcSample` of eleven. Adding
one metric means editing:

1. `sample.rs` — the field and its doc
2. `collect/linux.rs` — the read
3. `collect/darwin.rs` — the `None` that says this platform will not answer
4. `store.rs` — the write, the read, **and a `VERSION` bump**
5. every test fixture that constructs a `Sample` or a `ProcSample`

Measured against this session rather than guessed: `clock_ceiling` was one
`Option<f32>` and touched seven files. `cmd` touched ten. Both bumped the store
version.

atop reports on the order of two hundred metrics. At the current cost that is
not a scaling problem, it is a different project.

## The part that is not merely tedious

**A `VERSION` bump discards every stored sample.** `store::decode` refuses a
file whose version is not exactly current, so today's upgrade path for a user
with history is to lose it. atop ships `atopconvert` precisely because throwing
away twenty-eight days of logs on an upgrade is not something an institution
tolerates — and poptop is about to ask people to keep much more history than
ten minutes.

So this is not a tidy-up. It is the thing standing between poptop and being
depended on.

## What needs deciding

- **What replaces the flat struct.** A registry of metric descriptors — name,
  unit, platform, cost, how to read it — with `Sample` holding a keyed
  collection, versus keeping named fields and generating the store code. The
  first is more flexible and loses compile-time field access, which is most of
  what makes the current code readable. The second keeps the ergonomics and
  needs a macro.
- **Whether the UI keeps compile-time access.** `s.mem.used_pct()` is checked by
  the compiler today. A map lookup returning `Option<f64>` is not, and every
  call site becomes a place to get a name wrong.
- **The tension with `Unit`.** `ui::Unit` already describes how a series is
  measured. A metric descriptor should carry that rather than the UI deciding
  per-row, which would also give the axis, the threshold rules and the
  formatting for free — see 0032.

## Acceptance criteria

- [x] Adding a system metric is a single declaration plus its read
- [x] Adding a per-process metric is the same
- [x] macOS absence is expressed once, in the declaration, not per call site
- [x] The cost of the change is measured with `--bench`, both directions
- [x] Every existing metric is expressed in the new form, none left special

## How it was resolved

PR #75. The second of the two shapes offered above — named fields kept,
store code generated — with one change: the struct stays hand-written and the
macro takes a list of its field names beside it.

**Why not one list.** Generating the struct too would have put the doc comments
that document what poptop measures inside a macro invocation, in the file most
often read. And struct order is a readability choice while wire order is a
compatibility contract; collapsing them means reordering fields for legibility
silently changes the format.

So criterion one is met as *one declaration in `sample.rs` that the compiler
will not let you leave incomplete*, rather than literally one line. The reader
cannot build a struct it is missing a field for and the writer destructures
`Self` exhaustively, both checked by mutation.

**macOS absence** is `Sample::unknown()`, the base a collector defaults through.
The reasons macOS cannot answer a given metric stayed in `darwin.rs`, grouped
above the construction — they are macOS-specific measurements and would be wrong
in the platform-neutral model.

**Measured:** collection unchanged (6.08/6.41ms → 6.20/6.19ms); store identical
at 15.8 MB for 600 samples of 400 processes; decode 5.94 → 5.00ms. The first
attempt came out 0.7 MB larger, which was `state` widening from one byte to
four — `Codec for char` now writes a byte and a test pins the record at 65.

**Found on the way:** a corrupt store could panic the reader at startup on the
very first field, reachable by flipping a single bit. Fixed, with a sweep that
asserts no single-byte corruption panics.

**Not fixed here:** `VERSION` 13 → 14 still discards stored history. That is
0059, and it is a much smaller change now that there is one place to add a tag.
