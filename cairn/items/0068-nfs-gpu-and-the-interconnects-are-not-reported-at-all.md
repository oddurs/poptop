---
id: 68
title: NFS, GPU and the interconnects are not reported at all
type: feature
status: backlog
milestone: v2.1
depends_on:
- 58
- 60
created: 2026-09-08
updated: 2026-09-08
priority: p3
area: collect
---

## Problem

atop reports four subsystems poptop does not touch:

- **NFS** — `NFM` per mount, `NFC` client (`rpc`, `read`, `rpwrite`, `retxmit`),
  `NFS` server (`rchits`, `rcmiss`, `badauth`). On a box whose storage is a
  remote filesystem, poptop's disk figures describe the local disk that is doing
  nothing while the machine waits on the network.
- **GPU** — per-GPU and per-process utilisation and memory. atop needs a Python
  daemon and NVML for this.
- **Infiniband** (`IFB`) and **last-level cache** (`LLC`) — HPC and large-server
  territory.

## The shape of the decision

These are the items where "no compromise parity" and poptop's zero-setup
position actually collide, and the collision should be resolved once, here,
rather than four times.

NFS is a plain `/proc/self/mountstats` read and belongs in the tool. GPU needs
NVML or a daemon; infiniband needs the verbs stack. The proposed rule is that
**poptop reads what the kernel publishes to an ordinary reader, and treats
anything needing a daemon or a vendor library as an optional source that
enriches the picture and is never required for the tool to work** — the same
contract the IO columns already have, one level up.

That rule is worth writing down whatever is built, because it is the answer to
every future "should poptop shell out to X".

## Acceptance criteria

- [ ] NFS client and server activity where the machine mounts or serves it
- [ ] The rule above written into the README, and applied
- [ ] GPU and infiniband either implemented behind an optional source or
      explicitly declined in writing, with the reason
- [ ] Nothing added here makes poptop fail to start when the source is absent
