---
id: 143
title: Nothing can follow poptop as it runs
type: feature
status: backlog
milestone: v1.2
created: 2026-09-20
updated: 2026-09-20
priority: p1
effort: m
area: export
---

## Problem

Every feed is one-shot. `--export=json` takes two samples an interval apart, prints one object and exits; `--export=json DATE` prints a recorded day and exits. A dashboard, an agent or a `jq` pipeline that wants poptop's numbers as they happen has to start the process again every interval, which pays the whole startup cost — a collector, a first sample to difference against — for every reading, and produces a series whose samples are not evenly spaced.

The interactive monitor already samples on a schedule and already has the export writer. What is missing is the command that joins them.

## Proposal

`--export=json --follow` (and `line`): sample on the configured interval and write one record per sample, forever, flushing each so a reader sees it immediately. It ends on SIGTERM, SIGHUP, a closed pipe or `--for SPAN`.

The line format needs a decision to be followable: its header block is written once per stream, so a consumer that attaches later never sees it. Either repeat the header on an interval, or state that a follower must read from the start.

## Acceptance criteria

- [ ] `--follow` writes one record per interval, flushed, until it is stopped
- [ ] A reader that goes away ends it quietly, as `--once` already does
- [ ] The line format's header rule under `--follow` is decided and documented
- [ ] Sampling stays on the schedule it claims: the gap between records is the interval, checked over a run
- [ ] A test attaches to a live feed, reads several records, and stops it
