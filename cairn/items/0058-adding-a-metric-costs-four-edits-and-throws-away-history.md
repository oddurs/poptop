---
id: 58
title: Adding a metric costs four edits and throws away history
type: chore
status: backlog
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

- [ ] Adding a system metric is a single declaration plus its read
- [ ] Adding a per-process metric is the same
- [ ] macOS absence is expressed once, in the declaration, not per call site
- [ ] The cost of the change is measured with `--bench`, both directions
- [ ] Every existing metric is expressed in the new form, none left special
