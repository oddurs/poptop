---
id: 74
title: The only machine-readable output is one plain-text sample
type: feature
status: backlog
milestone: v2.2
depends_on:
- 58
created: 2026-09-08
updated: 2026-09-08
priority: p1
area: ui
---

## Problem

`--once` prints a fixed set of lines for scripts. atop has `-P label[,label]`
for whitespace-separated output and `-J` for JSON, over a documented label set
— `CPU`, `MEM`, `DSK`, `NET`, `PRC`, `PRG`, `PRM`, `PRD`, `PRN`, `CGR`, `ALL` —
with a stated line format: label, host, epoch, date, time, interval, then the
fields.

That is the difference between a tool you watch and a tool you can build on.
Every metric atop has is reachable from a script, by name, with a format that
does not change under you.

## What poptop has that makes this better than atop's

atop's parseable output is positional and its documentation is the man page.
poptop is about to have a metric registry with names, units and platform
availability — which is a schema, and a schema can be emitted. A consumer could
ask poptop what it reports rather than being told in prose.

## What needs deciding

- **Stability.** The point of machine-readable output is that it does not
  change. That is a promise about the metric registry's names, and it should be
  made deliberately or not at all.
- **Whether it reads history.** `--once` samples now. The interesting question
  is a script asking what happened at 03:00, which is the replay mode with a
  different output format rather than a separate feature.
- **Format.** JSON is obvious and verbose; a fixed-column form is what existing
  tooling parses. atop offers both, which is probably right.

## Depends on

The metric registry — a label set that is maintained separately from the
metrics will drift, and the drift will be silent.

## Acceptance criteria

- [ ] Every collected metric is reachable by name from a script
- [ ] Both a line format and JSON
- [ ] The schema is emitted, not only documented
- [ ] Absence is distinguishable from zero in both formats
- [ ] Output is stable across versions, or its instability is stated
