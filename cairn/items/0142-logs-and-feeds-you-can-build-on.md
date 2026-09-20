---
id: 142
key: v1.2
title: Logs and feeds you can build on
type: milestone
status: done
created: 2026-09-20
updated: 2026-09-20
priority: p2
due: 2027-06-01
---

The recording half of poptop, hardened and opened up. Today a day log is appended with a length, a block and a checksum, flushed but never synced; retention counts days and bytes across the directory and stops writing when the budget is reached; and every feed is one-shot — `--export` prints a sample or a recorded day and exits. Nothing can follow poptop as it runs, nothing can tail a day as it is written, a day file cannot be checked without reading it into a monitor, and what poptop itself had to assume about the machine is printed once at startup and then lost. This milestone is about a log that survives the machine losing power and says what it contains, and feeds another program can subscribe to.
