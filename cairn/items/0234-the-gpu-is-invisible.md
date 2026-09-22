---
id: 234
title: The GPU is invisible
type: feature
status: backlog
milestone: r9
labels:
- collect
created: 2026-09-22
updated: 2026-09-22
priority: p2
---

## Problem

0068 declined GPU reporting that needs NVML or a daemon, and was right to. It
did not look at what the kernel and the OS publish without either: amdgpu and
i915 expose busy percentage and memory in sysfs, and macOS publishes device
utilisation for the integrated GPU through the IO registry to any user.

## Proposal

A `Gpu` source reading only those: utilisation, and memory where given. A row on
the timeline when a GPU reports; nothing on a machine where none does.

## Acceptance criteria

- [ ] macOS device utilisation from the IO registry
- [ ] Linux amdgpu `gpu_busy_percent` and VRAM
- [ ] Absent, not zero, everywhere else
