---
id: 120
title: Recording, replay and machine-readable output are spread through an essay
type: docs
status: backlog
milestone: r6
labels:
- docs
created: 2026-09-19
updated: 2026-09-19
priority: p2
effort: m
area: docs
---

## Problem

`--store`, `--log`, `--read`, `--days`, `--report`, `--export`, `--schema` and `--once` are the difference between a live monitor and one you can ask about yesterday, and each is explained inside a section of the README arguing for it. There is no page that says what to turn on, where it goes, what it costs and how to read it back.

## Acceptance criteria

- [ ] `docs/guide/recording.md` and `docs/reference/output.md`
- [ ] The formats are shown as output, with the schema's own description
- [ ] Retention, the byte budget and where files live are stated with their defaults
