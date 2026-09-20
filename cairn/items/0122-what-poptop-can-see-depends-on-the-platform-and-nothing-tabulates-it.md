---
id: 122
title: What poptop can see depends on the platform, and nothing tabulates it
type: docs
status: done
milestone: r6
assignee: Oddur Sigurdsson
labels:
- docs
created: 2026-09-19
updated: 2026-09-19
priority: p2
effort: s
area: docs
---

## Problem

A third of a Mac's process table is unreadable without root; exited processes need `CAP_NET_ADMIN` on Linux; PSI needs 4.20; cgroups need v2; `/proc/<pid>/io` needs `CAP_SYS_PTRACE`. Each is explained where it bites, in a message or a README paragraph. A reader who wants to know what they will get, before they install it, has nowhere to look — and a reader who has just seen `196/781 need root` has nowhere to look either.

## Acceptance criteria

- [ ] `docs/reference/platforms.md`: a table of metric against Linux and macOS, with what each needs
- [ ] `docs/guide/troubleshooting.md`: every message poptop prints about something it could not read, and what to do

## How it was resolved

`docs/reference/platforms.md` is a metric-by-platform table with what each
one needs: cgroup v2, PSI on 4.20, `CAP_NET_ADMIN`, `CAP_SYS_PTRACE`, root on
macOS for other users' processes.

`docs/guide/troubleshooting.md` collects every message poptop prints about
something it could not read or would not do — the panel-title notes, the
timeline labels, the start-up refusals with their exit codes, the config and
theme warnings, and the log's "entries could not be read" — each with what to
do. The messages were taken from the running binary, not from the source.
