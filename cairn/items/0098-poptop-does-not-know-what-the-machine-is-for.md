---
id: 98
title: poptop does not know what the machine is for
type: feature
status: backlog
milestone: v4.0
created: 2026-09-13
updated: 2026-09-13
priority: p1
area: collect
---

## Problem

poptop shows four hundred processes as a flat list with a tree behind `t`. The
machine is not four hundred processes; it is three or four *services*, each a
handful of processes, and one of them is the reason you are looking.

An operator supplies that mapping from memory. A new one does not have it, and on
somebody else's box nobody has it.

## What it does

Infer the services from what is already collected — the process tree, cgroup
paths, container ids, users, and listening sockets — and offer them as a grouping
beside `name`, `user` and `container`:

```text
  SERVICE          PROCS   CPU%     MEM   LISTENING
  postgres            14   38.2    6.1G   :5432
  nginx                9    4.1   220M    :80 :443
  app (node)           4   22.7    1.9G   :3000
  ── unattributed ──  373    3.0    890M
```

`g` already cycles groupings. This is one more, and it is the one people mean.

## How, without configuration

Four signals, none of which needs collecting anything new:

- **cgroup path** — on anything systemd or container-managed this is the answer
  almost by itself. `/system.slice/postgresql.service` *is* the service name, and
  poptop already walks the cgroup tree.
- **the process tree** — children of a supervisor belong to it, which catches
  pre-forking servers that share no cgroup.
- **listening sockets** — what a service *is*, from outside, is a port. This is
  the one signal poptop does not collect yet; `/proc/net/tcp` plus the fd walk it
  already does for 0086 is the join.
- **the executable** — last resort, and the one everybody else starts with.

Ranked, and the panel says which signal it used. A service named from its cgroup
is a fact; one named from its executable is a guess, and the two should not look
alike.

## Why this is poptop's to do and not netdata's

netdata auto-discovers services in order to *collect* from them — it needs the
port to scrape an exporter. That is a different problem with a different failure
mode: it discovers things it has a collector for.

poptop needs no collector. It already has every process's resource use; the
inference only has to decide which rows belong together. That means it works on
services nobody wrote an integration for, which on a real box is most of them.

## What needs deciding

- **Whether inference belongs in a tool this literal.** Everything else poptop
  prints is measured. A service name is derived, and it will sometimes be wrong.
  The honest shape is that the *grouping* is inferred and every figure in the row
  is still summed from measurements — and that the unattributed group is always
  present and always visible, as 0082 requires of every attribution.
- **Whether to read `/proc/net/tcp` at all.** It is a real cost on a box with
  many sockets, and it is the signal that makes the answer good. Probably gated,
  and only while the grouping is on — the rule `Source::Cgroups` already follows.
- **Naming.** `postgresql.service` or `postgres`? The long form is true and the
  short form is what people say. Show the short, keep the long for the detail.
- **Stability.** A service that is renamed between samples because a signal
  flickered is worse than no grouping at all — #93's rule about layout that moves,
  applied to identity.

## Acceptance criteria

- [ ] Processes can be grouped by inferred service, from `g`
- [ ] Every row says which signal named it, and guesses look like guesses
- [ ] The unattributed remainder is a visible row, never dropped
- [ ] A service's figures are sums of measurements, never themselves inferred
- [ ] A name does not change between samples unless the underlying signal did
- [ ] Any new collection is gated on the grouping being on, and measured
