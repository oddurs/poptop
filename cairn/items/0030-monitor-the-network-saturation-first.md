---
id: 30
title: Monitor the network, saturation first
type: feature
status: backlog
milestone: v1.0
created: 2026-09-07
updated: 2026-09-07
priority: p2
area: collect
---

## Problem

Network is the one subsystem poptop does not monitor at all. The timeline draws
CPU, WAIT and MEM; btop's draws CPU, memory, network and disk. For a box whose
problem is the network, poptop currently shows a calm machine and no reason.

## Build the saturation half, not just the throughput half

Every monitor draws rx/tx throughput graphs, and throughput is *utilisation* —
it rarely answers "why is this slow". A link at 3% of capacity dropping 2% of
its packets is slow; a link at 90% is usually fine. The figures that matter:

- `/proc/net/dev` — per-interface bytes, packets, and crucially `errs` and
  `drop`, in both directions.
- `/proc/net/snmp` — `Tcp: RetransSegs`, the single best "the network is
  unhealthy" number on a server.
- `/proc/net/netstat` — `TcpExt: ListenDrops`, `ListenOverflows`, which say the
  accept queue is overflowing: a service failing to keep up, invisible in every
  other figure.

Throughput still gets drawn — it is what makes the timeline legible — but the
header figure should be the one that indicates trouble.

## macOS

`sysinfo::Networks::refresh` costs **425us** for 27 interfaces, measured — about
10% of a sample, affordable. It gives bytes, packets and errors per interface,
but no TCP retransmit counters, so those are Linux-only and render `—`.

Interface selection needs thought: this machine has 27 interfaces, 25 of them
idle `utun*` tunnels. Aggregating all of them hides the real one; picking "the
default route's interface" is right but needs a route lookup.

## Out of scope

Per-process network attribution, which needs `/proc/net` inode matching or eBPF
and is its own project. Already documented as such in the README.

## Acceptance criteria

- [ ] Per-interface throughput, plus errors and drops, both platforms
- [ ] TCP retransmits and listen-queue drops on Linux, `—` on macOS
- [ ] Idle interfaces do not crowd out the real one
- [ ] A timeline series for throughput, following the existing degradation ladder
- [ ] Measured: cost on both platforms
