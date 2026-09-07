---
id: 31
title: Filesystem capacity, as a threshold rather than a graph
type: feature
status: done
milestone: v1.0
created: 2026-09-07
updated: 2026-09-07
priority: p3
area: collect
---

## Problem

A full filesystem is a hard failure, and poptop cannot see one coming. It is the
one item in this group that causes an outage rather than a slowdown.

## Why it is ranked last anyway

It does not fit the two things poptop is built around.

- **Saturation over utilisation**: capacity is neither. It is a threshold, and
  the only interesting question is "how close to full".
- **Rewindable history**: capacity is a *status*, not a rate. Nobody scrubs back
  forty seconds to see the disk was 0.2% emptier. Retaining it per-sample costs
  buffer for a figure whose history is a straight line.

So: a header figure, not a timeline series, and one that appears only when a
filesystem is close enough to full to be worth the space.

## Cost

Read it directly, not through `sysinfo::Disks`, which costs **12.5ms** — three
times an entire macOS sample. A bare `statfs` is **1.5us**, measured, and that
is all this needs. sysinfo also reported the same volume twice on this machine,
which would have to be de-duplicated anyway.

Slower cadence than the sample loop is defensible here in a way it is not for a
rate: capacity that is one second stale is not misleading.

## Acceptance criteria

- [ ] Free and total for mounted filesystems, both platforms
- [ ] Read via `statfs`, not `sysinfo::Disks`, and measured against its cost
- [ ] Pseudo-filesystems and duplicate mounts of the same volume excluded
- [ ] Shown when close to full, absent when not — it should not spend header
      space on a machine with 400GB free
- [ ] Not retained per-sample unless a reason appears
