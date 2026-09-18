---
id: 85
title: A log directory poptop did not make is trusted as if it had
type: bug
status: backlog
milestone: r1
labels:
- validation
created: 2026-09-18
updated: 2026-09-18
priority: p1
effort: m
area: persist
---

## Problem

`persist.rs` and `log.rs` read and write under a directory that can be shared, symlinked, full, read-only, or filled with files poptop did not write. It is not known what happens in each case. The concerns are a symlinked day file pointing somewhere else, a 10 GB file with a valid header, a disk that fills halfway through a write, and two poptops logging to the same directory.

## Proposal

List each hostile or unlucky state, write a test that creates it in a temp directory, and record the behaviour. The expected behaviour is one of three: refuse with a message, skip with a warning, or recover. Silent data loss and following a symlink out of the directory are not acceptable.

## Acceptance criteria

- [ ] A write interrupted at any byte (simulated) leaves the previous day readable
- [ ] ENOSPC and EACCES while logging produce one warning, not a crash or a flood
- [ ] A symlink in the log directory is not followed outside it, or the decision to follow it is documented and tested
- [ ] Two poptops writing one directory either lock or interleave safely; a test proves which
- [ ] A file far larger than any day poptop could write is refused without being read into memory
