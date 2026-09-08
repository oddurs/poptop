---
id: 62
title: Nothing says which processes a container is running
type: feature
status: backlog
milestone: v2.1
depends_on:
- 58
created: 2026-09-08
updated: 2026-09-08
priority: p1
area: collect
---

## Problem

On any machine running containers, "which process is eating the box" has a
second half — *whose* process. atop shows `CID/POD` per process, accumulates by
container with `j`, and filters by it with `J`.

poptop shows a pid and a command line. On a Kubernetes node that is a hundred
processes named `node` and no way to tell which pod any of them belongs to.

## Where it comes from

`/proc/<pid>/cgroup` names the cgroup path, and for Docker, Podman and
Kubernetes the container id is a component of it. That is one small read per
process — the same shape as the command line, and cacheable the same way, since
a process does not change container.

The pod *name* is not in the cgroup path. atop needs superuser and reads it from
the runtime. Deciding whether poptop attempts that or shows the id is part of
this item.

## What needs deciding

- **Id or name.** The id is free and unreadable; the name needs the runtime and
  privileges. A truncated id is what atop falls back to, and it is honest.
- **Whether it is a column or a grouping.** 0050 already folds by name. Folding
  by container is the same machinery with a different key, and probably wants to
  be a choice rather than a second mode.
- **The parse.** Cgroup paths differ across runtimes and cgroup versions. This
  wants the split-the-parse-from-the-read treatment and a fixture per runtime,
  because it cannot be tested against a machine that has all of them.

## Acceptance criteria

- [ ] A process names the container it is in, where the platform says
- [ ] Processes can be grouped and filtered by container
- [ ] Parsed from fixtures covering docker, podman, containerd and plain cgroups
- [ ] A process in no container says nothing rather than showing a blank column
- [ ] Cost measured; cached per process like the command line
