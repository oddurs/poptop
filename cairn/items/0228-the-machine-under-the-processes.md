---
id: 228
key: r9
title: The machine under the processes
type: milestone
status: done
labels:
- collect
created: 2026-09-22
updated: 2026-09-22
priority: p1
---

Hardware and cadence sprint. poptop reads the kernel's accounting closely and
the hardware not at all: on the ten-core Mac it was built on, forty temperature
sensors sit readable by any user and none reaches the screen, the disk panel is
empty although the OS keeps per-volume counters, and nothing says whether the
machine is on battery or what the GPU is doing.

The polling half is the other reason this is one sprint. Collection runs on the
thread that reads keys, so every millisecond a new source costs is a
millisecond the interface is frozen — and the sensors cost forty of them on
this machine, measured. Moving collection off that thread comes first because
everything after it would otherwise make the tool feel slower. And ticks land
wherever the process happened to start, so two poptops, or a day and the one
after it, never sample the same instant.

The rule from 0068 holds: what the OS publishes to an ordinary reader is read,
and a source that can be refused degrades to absent, never to a zero.
