---
id: 52
title: The per-process history is shown at one percent of its value
type: feature
status: backlog
milestone: v1.0
created: 2026-09-07
updated: 2026-09-07
priority: p2
area: ui
---

## The opportunity

htop has per-process screens for `lsof`, `strace` and file locks. btop has a
detail panel. Both show *more attributes* of the process.

poptop holds something none of them do: the whole retained history of that
process — CPU, memory, threads, disk, its state at every sample in the buffer.
Today that is rendered as a ten-column sparkline in a table row, which is about
1% of the screen for the thing the tool is built around.

A detail view for the selected process would be the differentiator at full size,
and it is the one feature in this whole survey that no competitor could copy
without first building the buffer.

## What it would show

Full-width, over the same window the timeline shows:

- CPU and memory as real graphs rather than a ten-cell sparkline
- Thread count over time — the shape that says "leaking" rather than "busy"
- Disk read and write over time
- State over time, which is the one that answers "was it *running* or stuck"
- The moments it was absent from the buffer: when it started, and when it went

That last one is worth as much as the graphs. "This process did not exist before
14:32" is often the whole answer, and it is a fact about the buffer rather than
about the process.

## What needs deciding

- **Where it goes.** An overlay, a replacement for the timeline, or a third
  panel. The timeline is already the panel about time; a detail view is the same
  question asked of one process, which argues for replacing it rather than
  crowding beside it.
- **What it costs.** Nothing to collect — the data is already retained. It is
  entirely a rendering question, which makes it unusually cheap for its value.
- **Whether it scrubs.** It should: the cursor is shared with the timeline.

## Acceptance criteria

- [ ] A key opens a full-width history for the selected process
- [ ] It draws from the retained buffer, with no new collection
- [ ] Where the process is absent from the buffer is shown, not interpolated
- [ ] The time cursor is the same one the timeline uses
