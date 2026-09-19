---
id: 93
title: 'The Linux backend is tested on one kernel: whatever GitHub runs'
type: chore
status: backlog
milestone: r3
labels:
- testing
created: 2026-09-18
updated: 2026-09-18
priority: p1
effort: l
area: collect
---

## Problem

The `/proc` backend depends on the kernel version, cgroup v1 or v2, page size (0023 was a 64k-page bug), and whether PSI, taskstats and NFS are present. CI runs `ubuntu-latest`, which is one recent x86_64 kernel with 4k pages and cgroup v2. Every other combination is untested.

## Proposal

Record `/proc` and `/sys` trees (only the files poptop reads) from a handful of real machines: an old LTS kernel without PSI, cgroup v1, arm64 with 64k pages, a container with a restricted `/proc`, and a host with NFS mounts. Check them in as fixtures. Point the collector at a fixture root in tests and assert the parsed sample against values recorded alongside it.

## Acceptance criteria

- [ ] The collector can read from a root other than `/` (tests only, or a hidden flag)
- [ ] At least five fixture trees covering the combinations above, with a script to record more
- [ ] Each fixture has expected values, and a test compares the collector's output to them
- [ ] A missing subsystem in a fixture is reported as absent, never as zero
