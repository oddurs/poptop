---
id: 148
title: A recorded day cannot say what poptop had to assume
type: feature
status: backlog
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

- [ ] A note said during a session is in that day's file, with its time
- [ ] An older poptop reads a day containing notes without complaint
- [ ] `--read` and `--report` show what was assumed, beside the samples it applies to
- [ ] The corpus (0098) gains a file with notes in it
