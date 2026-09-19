---
id: 108
title: The header's network figure changes interface and counts loopback
type: bug
status: backlog
milestone: r5
labels:
- ui
created: 2026-09-19
updated: 2026-09-19
priority: p2
effort: s
area: ui
---

## What happens

The header's network figure named `lo0 14.4K/s 14.4K/s` in one run and `en0 46.4K/s 2.1M/s` in the next. It seems to pick whichever interface is busiest at the moment. Loopback traffic is not network traffic, so for a machine talking to itself the figure is meaningless. The two numbers also aren't labelled, so you can't tell which is received and which is sent.

## What should happen

- Loopback is never the headline interface.
- The headline interface is stable: the busiest over the buffered history, or the one carrying the default route, not whichever wins each sample.
- Received and sent are labelled, `↓46.4K/s ↑2.1M/s` or `rx`/`tx`, matching the NET graph.
