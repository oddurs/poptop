---
id: 98
title: The promise to read every store since format 15 is kept by nothing
type: chore
status: done
milestone: r4
labels:
- validation
created: 2026-09-18
updated: 2026-09-18
priority: p1
effort: m
area: store
---

## Problem

0059 resolved that poptop reads "every file from format 15 on, forever". There is no corpus of files written by those versions. A synthetic unknown-field test proves the mechanism, but only a file written by the old code proves the promise.

## Proposal

Check out each tagged or merged version from format 15 onward, write a small store and a day log with each, and commit the files as a corpus with a manifest recording the version and what the file contains. A test reads every file in the corpus and asserts the manifest's expectations. Each future format change adds a file.

## Acceptance criteria

- [x] A corpus with one store and one log per format version from 15 to the current one
- [x] A test reads every file and checks sample counts and a few known values
- [x] The release checklist or a CI check requires a corpus entry for a new format
- [x] A file from before format 15 is refused with a message naming its version, tested

## How it was resolved

**What "each format version" means now.** The format number has been 15 since #77. Since then fields are added through the schema block, not by bumping the number. Eleven merges changed what a sample holds, found by fingerprinting the `codec!` declarations at every first-parent commit: #77, #78, #80, #81, #82, #84, #85, #86, #87, #89 and #91. Nothing has changed since #91. The day log appeared in #92, and its entries gained a checksum in r1.

**The corpus**, `tests/corpus`: fourteen directories, each written by the poptop of that commit. There is one per schema version, `15-pr92` for the first (pre-checksum) day log, `15-pr115` for today's store and checksummed log, and `14-before-pr77` from the last format-14 build. `tests/corpus/write COMMIT LABEL` exports the commit, then builds and runs it in a Linux container. It runs `--store=on` on a terminal and quits before a second sample, so the store holds exactly one. Where the version has a log, it runs `--once --log=on` twice, so the day holds exactly two. The manifest records the commit, the format, what was run, and the machine's cores and memory from `/proc`.

It runs in a container because a store is a whole process table. The first pass wrote them on the Mac, and each store carried every command line and home path running at the time, in a public repository. Those files were deleted before any commit. The corpus is 136 KB.

**A second mistake, caught before commit.** `git archive` dates files at their commit, so cargo judged every old tree fresh against the shared target directory, and all fourteen files were written by one binary. It showed as fourteen identical sizes and a "format 14" store stamped 15. The script now touches the exported sources and refuses a binary that was not rebuilt.

**The tests** (`store::corpus`), on both platforms:

- `every_store_since_format_15_is_read_whole`: each decodes with no field reported dropped, holds the manifest's sample count, and every sample has the machine's core count and memory and a timestamp inside the writing window.
- `every_day_log_is_read_whole`: the same for both day logs, before and after checksums.
- `a_store_from_before_format_15_is_refused_by_its_version`: the real format-14 file is refused with exactly `the stored history is format 14, this poptop reads 15; discarded`.
- `the_corpus_has_a_file_with_this_builds_schema`: fails in CI if what a sample holds changes and no corpus file has the new shape. It also checks that #77's file is *not* taken for the current shape, so the check can fail.

`RELEASING.md` is new, and item 2 of its checklist is writing the corpus entry from the release commit. `tests/corpus/README.md` says how.

