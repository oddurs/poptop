---
id: 86
title: Nineteen unsafe blocks and five SAFETY comments
type: chore
status: backlog
milestone: r2
labels:
- review
created: 2026-09-18
updated: 2026-09-18
priority: p1
effort: m
area: safety
---

## Problem

There are `unsafe` blocks in `signal.rs` (3), `log.rs` (3), `collect/taskstats.rs` (5), `collect/procinfo.rs` (7) and `collect/linux.rs` (1). The codebase has five `// SAFETY:` comments. The FFI declarations (`kill`, `localtime_r`, `mktime`, and the netlink and `proc_pidinfo` calls) are written out by hand rather than taken from `libc`, so a wrong signature or struct layout would compile and misbehave.

## Proposal

Review each block. Write a `SAFETY:` comment stating the invariant and why it holds, or remove the block. Check every hand-declared extern signature and `#[repr(C)]` struct against the platform headers for both targets, including the arm64 and x86_64 layouts. Turn on `clippy::undocumented_unsafe_blocks`.

## Acceptance criteria

- [ ] Every `unsafe` block carries a `SAFETY:` comment that a reviewer has checked
- [ ] Every hand-written extern and `repr(C)` struct is checked against headers, with a size and alignment `const` assertion where it can be written
- [ ] Buffer lengths from netlink and `proc_pidinfo` replies are validated before any read that depends on them
- [ ] `clippy::undocumented_unsafe_blocks` is denied in CI
- [ ] The tests run under Miri for the modules Miri can execute, or the reason they cannot is written down
