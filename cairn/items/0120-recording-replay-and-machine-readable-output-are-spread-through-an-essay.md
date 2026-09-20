---
id: 120
title: Recording, replay and machine-readable output are spread through an essay
type: docs
status: done
milestone: r6
assignee: Oddur Sigurdsson
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

## How it was resolved

`docs/guide/recording.md` and `docs/reference/output.md`.

The three levels are separated — buffer, `--store`, `--log` — with what each
survives. Retention is a table of `--log-interval`, `--log-days` and
`--log-bytes` with their defaults and which one actually binds. The paths
are stated as `$XDG_STATE_HOME/poptop/history` and
`$XDG_STATE_HOME/poptop/log/poptop-YYYYMMDD`.

The cost is measured rather than asserted: a fifteen-second recording at one
sample a second on a ~740-process machine is 1.3M, which is the figure in the
page.

Every format is shown as real output — `--once`, `--export=json`,
`--export=line`, `--schema`, `--report`, `--days` — with `-`/`null` explained
as a figure the platform does not publish.
