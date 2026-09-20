---
id: 146
title: At the byte budget logging stops, and the session it was recording is the one it drops
type: bug
status: done
milestone: v1.2
created: 2026-09-20
updated: 2026-09-20
priority: p2
effort: m
area: log
---

## Problem

`log::append` weighs the whole directory against `log-bytes` and returns `false` when it is full, and today's file is never pruned because it is the history of the running session. Together those mean a long session at a short `log-interval` reaches the budget and then logs nothing more, forever, with one note in the footer. The reader who set a one-second interval to catch something is the reader who gets the least of it, and the samples they lose are the recent ones — the ones nearest whatever they are waiting for.

Measured in the code's own comment: 87 KB a sample, so a one-second interval is seven gigabytes a day against a 512 MB default budget. The budget is reached in under two hours.

## Proposal

Make room instead of stopping. Options, in the order they seem worth trying: prune older days first (already done) and then older *parts* of today, which needs today's file to be more than one file — size-based parts within a day, `poptop-20260920.001`; or drop to a coarser interval and say so, which keeps a whole day at lower resolution rather than two hours at full.

Whatever it is, the rule a reader can state should be: poptop keeps the most recent `log-bytes` of history, not the oldest.

## Acceptance criteria

- [x] At the budget, the newest samples are kept and the oldest are dropped
- [x] The rule is one sentence in the recording documentation
- [x] `--days` shows what a day actually holds when it has been trimmed
- [x] A test fills the budget and checks which samples survived

## How it was resolved

**The rule, in one sentence:** poptop keeps the most recent `log-bytes` of history, not the oldest. It is in the recording guide, the configuration reference, the troubleshooting page and the design note, which said the opposite and now says this.

**How room is made, oldest first.** `append` no longer refuses when the directory is at the budget; it calls `make_room`, which deletes days that are over, oldest first, and only when nothing else is left does the day being written give up its own morning. A day that is over is older than every entry of the day in progress, so the order falls out of the rule rather than being a policy of its own.

**Parts of a day, not parts in the filename.** The proposal offered `poptop-20260920.001` as a way to drop parts of today. That would have made a day several files: a second reader, a second thing for `--days`, `date_in_name`, retention and the follower to agree about. Instead `trim` drops whole entries from the front of the one file — found by lengths alone, four bytes an entry, never decoding — and writes the kept tail beside it and renames over, so a reader that opens the day mid-trim gets the old file whole or the new one and never a half-copied one. The temp file is not a day file, so nothing counts it against the budget, and a stale one left by an interrupted trim is removed by the next.

**Cost, measured.** `measure_trimming_a_day`: 25.8 ms to free an eighth of a 25 MB day, about 1.2 ms a megabyte kept. Freeing an eighth of the budget at a time is the bargain chosen — an eighth of the history given up at once, and seven bytes copied for every byte appended once the budget is full. At the 512 MB default that is roughly half a second, paid every twelve minutes at a one-second `log-interval` and never at all at the ten-minute default, where seven days of logs come to a fifth of the budget.

**What `append` says now.** `Appended::{Wrote, Trimmed(String), Full}` instead of a bool. `Trimmed` carries the sentence for the reader and reaches the panel while it is true, as the old "no longer being written to" note did. `Full` is the one case left where nothing is written — a budget that will not hold a single entry — and it is decided before anything is deleted, so a log never gives up a day to make room it still would not have.

**`--days` says what a day holds**, not only what it costs: `2026-09-20  512.0M  from 14:20`. One entry is read for it, not the file.

**The follower had to learn about it** (0144, merged an hour earlier). A trim renames a new file over the old, so a byte offset into it points into the middle of some other entry — and length alone does not notice, since a trim that drops one entry and an append that adds one leave the file exactly as long as it was. `Follower` now remembers the device and inode its offset belongs to, restarts from the front when the file is replaced, and drops what it has already handed out by sample time. `a_follower_of_a_day_that_is_trimmed_underneath_it_repeats_nothing` holds it.

**Tests.** Four in `src/log.rs` — the newest kept and the oldest dropped with the file staying inside the budget and still reading cleanly; older days dropped before the day being written; a budget too small for one entry writing nothing and changing nothing; and no temporary file left behind — plus the follower test above and one end-to-end run in `tests/cli.rs` that fills a two-entry budget from the command line and checks what `--export` and `--days` then say.
