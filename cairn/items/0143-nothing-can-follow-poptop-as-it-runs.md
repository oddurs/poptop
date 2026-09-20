---
id: 143
title: Nothing can follow poptop as it runs
type: feature
status: done
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

- [x] `--follow` writes one record per interval, flushed, until it is stopped
- [x] A reader that goes away ends it quietly, as `--once` already does
- [x] The line format's header rule under `--follow` is decided and documented
- [x] Sampling stays on the schedule it claims: the gap between records is the interval, checked over a run
- [x] A test attaches to a live feed, reads several records, and stops it

## How it was resolved

`--export=json|line --follow`, with `--for SPAN` to stop it on time. A record an interval, written and flushed as it is taken.

**Decided — the line format's header rule: once, at the top of the stream.** The same rule a recorded day follows, so the two are one format rather than two dialects, and a label first seen an hour in brings its own header then. The alternative, repeating the block on a timer so a late reader can attach, was turned down: a consumer that keeps a column map would have several headers to choose between and no way to know whether they agree, and the reader most likely to attach late is a program, for which `--export=json` — every field named in every record — is the honest answer. What this costs is written down rather than left to be discovered: `tail -f` on a redirected feed sees rows with no header. In `docs/reference/output.md` and the recording guide.

**The flush is the point.** stdout is block-buffered when it is a pipe, so the loop that only wrote would deliver a feed 8 KB at a time — nothing for a minute, then forty samples at once. Each record is written and flushed as one write; `BrokenPipe` ends it at exit 0, as every other output path already does, and any other write error is still an error, since a feed that swallowed a full disk would be a silent hole in somebody's recording.

**The schedule.** `next = now + interval`, rebased on the clock after each sample rather than advanced from the last deadline: advancing would make a laptop that slept for an hour write eighteen thousand records as fast as it could on waking. The wait is in `STOP_CHECK` slices so SIGTERM and SIGHUP are noticed within 100 ms and not at the end of an interval that may be an hour. The first record arrives one interval in — every rate is a difference, and the priming read has nothing to difference against — which the guide states rather than leaving as a surprise.

**Refused rather than ignored**, as the rest of the command line is: `--for` without `--follow` (a feed that was never asked for), and `--follow DATE` (a recorded day does not grow — following a day as it is written is 0144).

**Tests.** `tests/export.rs` runs the feed as a consumer does, reading from the pipe while it runs: five records, each whole parseable JSON, with the cadence checked across the run (200 ms samples, 0.15–0.45 s a record — a bound wide enough that one late sample on a shared runner is not a failure and a drifting schedule still is); SIGTERM and SIGHUP each end it at 0; a reader that drops the pipe ends it at 0 with nothing on stderr; `--for 1s` stops itself, and the line format it wrote has exactly one copy of each header and opens with one. `tests/cli.rs` holds the refusals and one end-to-end `--for` run.
