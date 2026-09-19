---
id: 108
title: The header's network figure changes interface and counts loopback
type: bug
status: done
milestone: r5
assignee: Oddur Sigurdsson
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

## How it was resolved

The header and the NET graph now follow one interface, chosen by `App::headline_link`: the interface that carried the most over the last 60 samples (a minute at the default interval), never loopback. The window ends at the cursor, so when you scrub back the figure follows the interface that was busy at that time.

- **Loopback** is recognised by name (`lo`, `lo0`, `lo12`…; not `low0` or `lo-`). A machine with only loopback shows no network figure rather than a local one.
- **Stable.** Picking per sample, the header named `lo0` one second and `en0` the next. Over a minute, the choice moves only when another interface has really taken over.
- **The graph was worse than the header:** it drew the busiest link of each sample, splicing interfaces into one line. It now draws the headline interface throughout.
- **Labelled:** `en0 ↓47.3K/s ↑1.8M/s`, or `rx`/`tx` on the ASCII glyph tier.

`NetStat::busiest` had no callers left, and the dead-code lint said so, so it's removed. The macOS collector test that pushes bytes over 127.0.0.1 now looks up the loopback link by name, since loopback is what it measures.

Live, three runs in a row: `en0 ↓25.1K/s ↑558.1K/s`, `en0 ↓47.3K/s ↑1.8M/s`, `en0 ↓69.7K/s ↑2.9M/s`. Tests: `the_header_names_one_real_interface_and_says_which_way_the_bytes_go` has en1 winning single samples and lo0 winning every sample, and the headline must stay on en0 from the second sample on. `loopback_is_known_by_name` covers the names.
