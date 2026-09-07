---
id: 27
title: Per-process CPU is clamped on Linux and not on macOS
type: bug
status: done
milestone: v1.0
created: 2026-09-06
updated: 2026-09-06
priority: p3
effort: s
area: collect
---

## Problem

The Linux backend clamps per-process CPU to `cores * 100` and explains why: a
pid reused between samples diffs the new process against the old one's counter
and can otherwise report thousands of percent. htop guards the same way.

The macOS backend has no such clamp — `cpu: p.cpu_usage()` straight from
sysinfo. Nothing measured here exceeded the bound, so this is an asymmetry
rather than an observed failure: the same guard is either necessary or it is
not, and one backend having it is a question about the other.

Two backends can legitimately need different guards. What they should not do is
differ silently, because the next person reading `parse_proc_stat`'s comment
about pid reuse will reasonably assume it holds everywhere.

## Proposal

Decide it once. Either the clamp belongs to the model — applied wherever a
`ProcSample` is built, so it cannot be forgotten by a third backend — or it is
genuinely Linux-only and the comment should say so.

The model is the better home. The claim being made is about what a machine can
deliver, which is not a fact about `/proc`.

## Acceptance criteria

- [ ] One decision, stated where a new backend would find it
- [ ] Both backends agree, or the difference is documented with its reason
- [ ] A test covers whichever rule is chosen, on both platforms
