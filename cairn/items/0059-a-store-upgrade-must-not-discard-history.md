---
id: 59
title: A store upgrade must not discard history
type: bug
status: backlog
milestone: v2.0
created: 2026-09-08
updated: 2026-09-08
priority: p0
area: store
---

## Problem

`store::decode` refuses any file whose `VERSION` is not exactly the current one,
and `VERSION` has been bumped five times in one session. The upgrade path for a
user with stored history is to silently start again.

That is defensible for a ten-minute ring buffer and indefensible for the
multi-day logging v2.2 is about to add. atop ships `atopconvert` as a separate
program because reading last month's logs after an upgrade is a requirement, not
a nicety.

## Why the format cannot stay as it is

It is positional. Every value is written in a fixed order with no tag, so a
reader has no way to skip a field it does not know or to supply one that is
missing. That is what forces the version bump: any change is a breaking change.

`opt_str` already shows the strain — it writes a placeholder index for `None` so
the record keeps a fixed length, and the reader had to be taught not to resolve
it. A tagged format would not have had the problem.

## What needs deciding

- **Tagged fields versus a schema block.** Tag every value and skip unknown tags
  — simple, self-describing, costs a few bytes a field. Or write the schema once
  per file and index into it — compact, and a reader must handle a schema it has
  never seen.
- **How far back to read.** Every version ever, or a stated window with a
  conversion tool for older files.
- **What a reader does with a metric it does not know.** Skipping silently is
  what makes upgrades work; it is also how a field can disappear from a graph
  with nobody noticing. Probably: skip, and say so once.
- **Whether the ring buffer and the log are the same format.** They are today.
  A daily log wants an index and a header per sample; a ring buffer does not.

## Acceptance criteria

- [ ] A file written by an older poptop is read by a newer one, with fields the
      newer one does not know skipped and fields it expects absent rather than
      wrong
- [ ] A file written by a newer poptop is read by an older one, or refused with
      a message that says which version wrote it
- [ ] Adding a metric does not require a version bump
- [ ] A test writes a file with a synthetic unknown field and reads it back
- [ ] Measured: bytes per sample before and after, at 400 processes
