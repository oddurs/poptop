---
id: 147
title: One machine at a time
type: feature
status: backlog
milestone: v5.2
depends_on:
- 74
created: 2026-09-13
updated: 2026-09-13
priority: p2
area: ui
---

## Problem

An incident is rarely one box. The answer today is a terminal per host and a
human doing the correlation by looking between windows.

The fleet answer exists and it is netdata, or Prometheus, or a vendor: agents
installed in advance, a time-series database, a dashboard. All of them are
excellent and all of them require the thing poptop exists to not require.

## The shape that keeps the position

poptop already emits a complete sample as one line of JSON and reads a day from
a file. A remote view needs no new protocol and no agent:

```sh
poptop web01 web02 db01        # ssh, run --export, draw them side by side
```

Each host runs `poptop --export=json` over ssh — a single static binary, copied
if it is not there — and the local poptop draws one row per host with the same
timeline, the same keys, the same explanation.

No daemon, nothing listening, nothing installed in advance, and it works on the
machines you have ssh to, which during an incident is the set that matters.

## What needs deciding

- **Whether it is poptop's job at all.** The counter-argument is real: this is
  the seam where a monitor becomes an orchestration tool, and `ssh host poptop
  --export=json` in a loop is already most of the value with none of the code.
  If the answer is no, the README should say so and show the loop.
- **Getting the binary there.** Assuming poptop is installed is the assumption
  this tool exists to avoid. Copying a static binary over ssh is a real answer
  and it is also the moment poptop becomes something that writes to other
  people's machines, which needs the same deliberateness signals got.
- **What is shown.** Not every host's full table — that is a dashboard, and a
  bad one at eighty columns. One row a host, the figure that is worst, and the
  process responsible for it: the explanation from v5.0, per host, ranked.
- **Clock skew.** Two hosts' timestamps will not agree. A shared timeline across
  hosts whose clocks differ by seconds is a graph that lies, and the honest
  answer is probably to say the skew rather than to correct it.
- **Failure.** A host that is unreachable, slow, or running a different version
  must be a row that says so. A fleet view where a dead host looks idle is worse
  than no fleet view — that is the same rule as every `—` in this tool, at a
  different scale.

## Acceptance criteria

- [ ] Decided in writing, either way, with the reasoning
- [ ] If built: nothing is installed in advance on the target
- [ ] If built: a host that cannot be reached is visibly absent, never idle
- [ ] If built: clock skew between hosts is stated, not silently corrected
- [ ] If built: one row a host, carrying the worst figure and what caused it
- [ ] If declined: the README shows the ssh loop that gets most of the value
