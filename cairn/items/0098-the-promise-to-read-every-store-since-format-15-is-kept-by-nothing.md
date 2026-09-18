---
id: 98
title: The promise to read every store since format 15 is kept by nothing
type: chore
status: backlog
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

- [ ] A corpus with one store and one log per format version from 15 to the current one
- [ ] A test reads every file and checks sample counts and a few known values
- [ ] The release checklist or a CI check requires a corpus entry for a new format
- [ ] A file from before format 15 is refused with a message naming its version, tested
