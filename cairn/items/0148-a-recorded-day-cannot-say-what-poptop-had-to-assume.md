---
id: 148
title: A recorded day cannot say what poptop had to assume
type: feature
status: done
milestone: v1.2
created: 2026-09-20
updated: 2026-09-20
priority: p2
effort: m
area: log
---

## Problem

The collector's notes — the exit listener that could not register, the IO columns withdrawn after the probe, the page size that had to be guessed, a source the budget gave up — are printed once at startup, or into the footer, and then gone. A day opened a week later shows the numbers without any of the reasons they are the shape they are. "A source poptop chose not to read" and "a figure the kernel does not publish" are the distinction the whole export format exists to keep, and the log keeps neither.

## Proposal

Record the notes in the day file, as their own kind of entry, with the time they were first said. `--read` shows them against the moment they belong to; `--verify` and `--report` list them; the export carries them.

The entry is a new record in the store's schema, which the schema block already makes readable by an older poptop — it is skipped as an unknown record rather than breaking the file.

## Acceptance criteria

- [x] A note said during a session is in that day's file, with its time
- [x] An older poptop reads a day containing notes without complaint
- [x] `--read` and `--report` show what was assumed, beside the samples it applies to
- [x] The corpus (0098) gains a file with notes in it

## How it was resolved

**A field on the sample, not an entry of its own.** The proposal asked for a new kind of entry in the day file. A new record would have been a second thing for every reader to agree about — the follower, `--verify`, retention, the corpus — and an older poptop would have met it as an entry it could not decode and called the day damaged. A field on `Sample` is the thing the schema block already makes safe: an older build reads the day whole, names the field it skipped, and misreads nothing after it. `a_day_recorded_with_notes_reads_in_a_build_that_does_not_know_them` simulates that build by taking `notes` out of what this one declares, and asserts exactly that, over two samples — with one, a skip that ate the rest of the file would pass.

`notes: Option<Vec<Arc<str>>>`. `None` is a recording that does not carry them, `Some([])` a moment at which nothing had to be assumed: absent is not empty here any more than anywhere else in this format.

**One drain.** `Collector::sample` now takes the backend's notes into the sample it is returning, and nothing else drains them. `open_collector` used to, which is why whatever opening the collector had to assume never reached a day file; now it reaches the first sample, and the paths that print warnings read them back from there. The interactive loop keeps each distinct note for the lines printed at exit, and `--once` and the one-shot `--export` print them from the two samples they take.

**Where they show.** The table's status line shows what was assumed at the moment under the cursor — from the sample, so a day opened a week later carries its own reasons rather than this session's — ranked with the other omissions. `--report` lists each distinct note once, with the time it was first said, under the figures it qualifies. Both formats of `--export` carry the field, which falls out of the schema-driven writer.

**The corpus** (0098) gains a directory written by this build, in a Linux container as the README requires: a store on this machine would carry the process table of whoever ran it, and the repository is public.

**Tests.** The day-file round trip with the time and the empty-versus-absent distinction; the older-build read above; a report that says a note once and at the moment it was first said, and says nothing for a day that assumed nothing.
