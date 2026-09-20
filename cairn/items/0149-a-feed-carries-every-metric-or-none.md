---
id: 149
title: A feed carries every metric or none
type: feature
status: backlog
milestone: v1.2
created: 2026-09-20
updated: 2026-09-20
priority: p3
effort: s
area: export
---

## Problem

`--export` writes every field of every record: at four hundred processes a JSON sample is hundreds of kilobytes. That is right for "every metric by name", and wrong for a feed a dashboard reads every second, where the consumer wants four numbers and pays for the process table each time. The only way to narrow it today is to pipe the whole thing through `jq`, which means poptop still built it.

## Proposal

`--fields cpu_total,mem.used,procs.name` — dotted paths into the schema, checked against it, so a wrong name is refused with what it could have been rather than silently dropped. Absent fields stay absent: a filter that turns a missing figure into a zero would undo the one rule the format is built on.

## Acceptance criteria

- [ ] A named subset is emitted, in schema order, for both formats
- [ ] An unknown field is refused, naming the closest match the schema has
- [ ] The absent-versus-zero distinction survives the filter
- [ ] The cost of a narrow feed is measured against a whole one
