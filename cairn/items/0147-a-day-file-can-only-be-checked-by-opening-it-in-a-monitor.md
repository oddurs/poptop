---
id: 147
title: A day file can only be checked by opening it in a monitor
type: feature
status: done
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

- [x] `--verify` reports entries, span, spacing and every skip with its reason
- [x] The exit status distinguishes intact from damaged
- [x] A file damaged in each of the ways r1 tested is reported as that kind of damage
- [x] It reads the file without holding a day's samples in memory

## How it was resolved

`--verify [DATE]`, today unless a date is given:

```console
$ poptop --verify 2026-09-19
poptop-20260919  61.4M
entries 144 whole, 144 samples
period  00:02:11 to 23:57:09, every 10m
bytes   64392101 of 64392101 in whole entries
intact
```

**Exit 0 intact, exit 1 damaged**, so `poptop --verify && …` is a question cron can ask. Everything is on stdout: this is the answer, not a warning about one.

**The reader's decisions, reported rather than repaired.** `log::verify` walks the file with the same `step` `read_blocks` uses, so a file it calls intact is one the reader reads whole and a stretch it names as lost is the stretch the reader skips. The tests assert that directly — after each kind of damage, `check.samples` equals `read_day(...).len()`. It also makes the same "an entry that decoded only by borrowing the bytes of the one after it" check, which is the case the checksum went in for.

**Damage is named for what happened**, because calling a truncated write "a different version" sends somebody chasing an upgrade that never happened: `an entry cut short`, `an entry from a different version`, `empty bytes`, `a fragment at the end` — each with the offset it starts at and how many bytes it covers. The reasons add up: whole entries plus damage equals the file's size, which is asserted rather than hoped for.

**Without holding a day.** One entry is read, decoded, counted and dropped; what survives is the count, the span, the gaps and a line per stretch of damage. Resyncing after damage is a 256 KB window scanned for the magic, with each candidate settled by reading its own head — `starts_store` needs the magic, the version and the schema block, about 1.5 KB, and 64 KB is read for it — so even finding its place again never pulls an entry into memory. `verifying_a_day_does_not_hold_it` measures this process's resident memory across a verify of an 8 MB day and holds the growth under half the file.

**Tests.** A whole day verifying as intact with the right entries, span and spacing; every kind of damage reported as that kind, with the bytes adding up and the reader agreeing about what survived; the borrowed-bytes entry; the memory bound; and one end-to-end run in `tests/cli.rs` — a good day, then a broken one, checking the exit status changes and that a day nobody recorded says so rather than printing an errno.
