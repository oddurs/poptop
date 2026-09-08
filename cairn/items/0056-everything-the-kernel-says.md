---
id: 56
key: v2.1
title: Everything the kernel says
type: milestone
status: backlog
created: 2026-09-08
updated: 2026-09-08
priority: p2
due: 2027-07-01
---

Breadth and depth: the subsystems atop reports that poptop does not — NUMA,
paging and OOM, swap detail, cgroup v2, containers, NFS, GPU — and the
per-process fields behind atop's eight process views.

Each of these is a small item once v2.0 lands, because adding a metric becomes
a declaration rather than an edit in four files and a store version bump.
