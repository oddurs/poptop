---
id: 59
title: A store upgrade must not discard history
type: bug
status: done
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

- [x] A file written by an older poptop is read by a newer one, with fields the
      newer one does not know skipped and fields it expects absent rather than
      wrong
- [x] A file written by a newer poptop is read by an older one, or refused with
      a message that says which version wrote it
- [x] Adding a metric does not require a version bump
- [x] A test writes a file with a synthetic unknown field and reads it back
- [x] Measured: bytes per sample before and after, at 400 processes

## How it was resolved

PR #77. The first of the two shapes offered above — a schema block, not a tag
per value.

**Why.** Tagging every value costs a few bytes per field, and the thing being
stored is 600 samples of 400 processes, so a per-field cost is paid 2.6 million
times — roughly a fifth of the file. Declaring the shape once per file costs
**1,501 bytes total** and nothing per sample. Bytes per sample: unchanged.
Encode 23.1 → 22.4ms, decode 5.3 → 4.8ms.

Same-version reads keep the old speed through a fast path: the reader decides
once per file whether the schema is exactly its own, and if so skips the
per-field comparison entirely. The merge is for the one run after an upgrade.

**The other three decisions.** How far back to read: every file from format 15
on, forever — a field difference is no longer a version difference, and anything
older is refused with a message naming the version that wrote it. An unknown
metric: skipped, and said once through the existing warning channel. Ring buffer
and log: still one format; splitting them is 0072's decision, and the schema
block is what makes a daily log viable at all.

Matching is on a hash of the type structure, so a field whose type changed is
skipped rather than misread — and reported, because a column emptying out after
a downgrade with nothing said is the failure this item is about.

**Found on the way.** Making the file self-describing made the schema block
corruption-controlled input, which opened three ways to hang or crash the
reader: a self-referential record recursing until the stack overflowed, and a
zero-width record letting a four-billion-element list consume no input, in both
the skip path and the merge path. A parsed schema is now validated once for
cycles and minimum width. A cycle *through a list* escaped the first version of
that check.
