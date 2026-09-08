---
id: 62
title: Nothing says which processes a container is running
type: feature
status: done
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

- [x] A process names the container it is in, where the platform says
- [x] Processes can be grouped and filtered by container
- [x] Parsed from fixtures covering docker, podman, containerd and plain cgroups
- [x] A process in no container says nothing rather than showing a blank column
- [x] Cost measured; cached per process like the command line

## How it was resolved

PR #82. Twelve characters of the container id from `/proc/<pid>/cgroup` — what
`docker ps` shows and what atop falls back to. The pod *name* is not in that
path; atop reads it from the runtime with superuser, and an id is what poptop
can know without asking anybody's permission.

**A grouping choice, not a second mode.** `g` cycles: by name, by container,
off. Folding by container is 0050's machinery with a different key, so it does
not add a fifth exclusive layout. Processes in no container are dropped rather
than folded into a heap called "none" — the question is what each container is
doing.

**Fixtures per runtime, because no machine has them all**: docker under systemd,
cgroupfs and v1; podman; containerd under Kubernetes; cri-o; a login session; a
plain service; and `0::/../..`, observed from inside a container here, which
carries no id at all. Two are the traps — a pod's own `…-pod<uuid>.slice` is
long enough to look like an id, and `session-3.scope` is hex-ish but short.

**Cached for the life of the process**, since a process cannot change container;
the start time stops a recycled pid inheriting the dead one's answer. Retained
size 65 -> 70 bytes a process, which the record-size test caught by design.

**Review found a leak the design invited**: every other per-pid map expires on a
refresh slot, so stale entries go anyway — this one is deliberately valid for a
process's whole life, so nothing removes an entry unless the prune does. Also
that the column's width was missing from the elision arithmetic, that it was
gated on the IO columns' minimum and so never appeared on a hundred-column
terminal, and that a container holding one process was labelled with that
process's name.
