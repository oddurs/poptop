---
id: 147
title: A day file can only be checked by opening it in a monitor
type: feature
status: backlog
milestone: v1.2
created: 2026-09-20
updated: 2026-09-20
priority: p2
effort: m
area: log
---

## Problem

r1 taught the reader to survive a torn entry, a stranger's bytes and a file that is not a day at all, and r2 gave every entry a checksum. All of that is invisible: the only way to learn what a day file contains is `--read DATE`, which needs a terminal, or `--export json DATE`, which prints the samples and mentions damage in warnings on stderr. There is no way to ask "is this file intact, and what is in it" from a script, and no way to see it without loading a day into memory.

## Proposal

`--verify [DATE]`: walk the file entry by entry and report the count of whole entries, the first and last sample times, the spacing, entries skipped and why (a failed checksum, a torn tail, a version this build cannot read), and the bytes accounted for against the file's size. Exit 0 when everything decoded, 1 when anything was skipped, so cron can ask.

## Acceptance criteria

- [ ] `--verify` reports entries, span, spacing and every skip with its reason
- [ ] The exit status distinguishes intact from damaged
- [ ] A file damaged in each of the ways r1 tested is reported as that kind of damage
- [ ] It reads the file without holding a day's samples in memory
