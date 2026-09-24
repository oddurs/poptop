---
id: 254
title: Every tile is about the machine
type: feature
status: backlog
milestone: r13
labels:
- ui
created: 2026-09-23
updated: 2026-09-23
priority: p1
---

## Problem

`d` swaps the timeline's subject to one process. `C` swaps the table for
cgroups. `y` expands threads. Three keys, three mechanisms, each hard-coded to
one target, and none of them composes with a layout: there is no way to say
"this tile is postgres, that one is nvme0n1".

## Proposal

A tile is bound to a **subject**: the machine, a process, a thread, a cgroup, a
container, a device, a filesystem, a link. A subject resolves to the series a
tile can draw and to the identity it prints. Binding is a gesture rather than a
key per target: put the cursor on a row, send it to a tile.

The subject is what makes the dashboard poptop's rather than generic: every
subject is scrubbable, because the buffer holds the whole table at every
instant.

## Acceptance criteria

- [ ] A subject type resolving to series, identity and thresholds
- [ ] Existing keys (`d`, `C`, `y`) become bindings of it rather than parallel mechanisms
- [ ] A subject that is absent at the cursor's instant says so rather than drawing zero, as the detail panel already does
