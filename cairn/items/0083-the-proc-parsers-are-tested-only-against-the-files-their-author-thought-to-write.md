---
id: 83
title: The /proc parsers are tested only against the files their author thought to write
type: chore
status: backlog
milestone: r1
labels:
- validation
created: 2026-09-18
updated: 2026-09-18
priority: p1
effort: l
area: collect
---

## Problem

`src/collect/linux.rs` (4,491 lines) and `procinfo.rs`, `cgroups.rs`, `nfs.rs` and `taskstats.rs` parse text and binary formats that the kernel owns and has changed over time. They include fields that are absent on older kernels, `comm` values containing spaces and parentheses, and `mountstats` lines that vary by NFS version. The 90 tests in `linux.rs` use hand-written fixtures. Nothing checks that a malformed or unexpected file produces "unknown" rather than a panic or a wrong number.

## Proposal

Give each parser a property test: arbitrary bytes never panic, and a well-formed file with any one field removed or duplicated yields `None` for that field and the correct values for the others. Add fuzz targets for the parsers that take attacker-influenced text, such as `comm` and `cmdline`.

## Acceptance criteria

- [ ] Every parser of a `/proc` or `/sys` file has a never-panics property test
- [ ] A `comm` of `a) (b` and one of 16 bytes of invalid UTF-8 are both parsed correctly
- [ ] Missing optional fields (e.g. PSI on a pre-4.20 kernel) yield `None`, not zero
- [ ] Fuzz targets for `stat`, `status` and `mountstats` parsing
