---
id: 144
title: A day being written cannot be followed
type: feature
status: done
milestone: v1.2
created: 2026-09-20
updated: 2026-09-20
priority: p1
effort: m
area: log
---

## Problem

The day log is the thing poptop has that other monitors do not, and nothing can stream it. `--export json DATE` reads the file once and exits, so a consumer that wants today's samples as they are appended must poll the whole file and re-read what it has already seen. The file is append-only with a length in front of each entry, which is exactly the shape a tail wants — the reader just does not exist.

## Proposal

`--export json today --follow`: read what the file holds, then wait for more, decoding each entry as it lands. A torn tail — a writer mid-append — is a wait, not an error; r1's reader already knows how to find the next whole entry.

## Acceptance criteria

- [x] A follower reads entries appended after it attached, in order, without repeating any
- [x] An entry half-written when the follower reaches it is waited for, not skipped or reported as damage
- [x] Following a day that rolls over at midnight moves to the new file
- [x] A test writes a day from one process and follows it from another

## How it was resolved

`--export=json|line DATE --follow`: what the day already holds, oldest first, then each entry as it is appended. The writer is any poptop running with `--log=on` — the one you are watching from, another session, or a cron job — which is what makes this a subscription rather than a second copy of the same process's output.

**`log::Follower`, beside `read_blocks` rather than inside it.** The two readers answer opposite questions about the same bytes. For a file that has stopped changing, a length running past the end of the file is a torn entry and the reader resyncs past it; for a file being written it is `append` partway through, and the answer is to wait. `Follower::drain` checks that first — `at + LEN + len > bytes.len()` is a wait, not damage — and only then hands the entry to the same `step` the whole-file reader uses. The offset moves past an entry only when it decoded from exactly the bytes it claimed, which is what makes "nothing twice" structural rather than remembered.

Everything else is the whole-file reader's recovery, reused so the two agree: an entry that will not decode with another entry behind it is a different version's and is skipped; anything else resyncs on the next entry's magic; and an entry that decoded only by borrowing the bytes of the entry after it is taken back — `next_block` starting inside the span just read — which is the case the checksum was added for. `a_follower_reads_a_day_a_crash_tore_the_same_way_the_reader_does` holds them to the same samples and the same note.

**Midnight.** Rolling on the local clock would lose the entry for the sample taken at 23:59:59, which is written a moment after midnight and filed by the sample's own date. The signal used instead is the writer's: when the current file has gone quiet and a file exists for a later day, the follower moves to the oldest such day and says so on stderr. A follower of a day that is over does not roll — it was asked for that day. Reading incrementally rather than by the clock also means a machine that was asleep for a week resumes on whatever day poptop was next run.

**Polled, at 250 ms.** Entries arrive a `log-interval` apart — ten minutes by default — and a poll is a `stat` and usually nothing else. inotify and kqueue are two platform APIs, two failure modes and a descriptor per file, to learn a quarter of a second sooner.

**Notes go to stderr**, where every other path's warnings go, each said once however many polls see it: a consumer parsing records must never have to parse prose.

**Tests.** In `src/log.rs`, against a file written entry by entry: a day followed as it is written (in order, none twice, none skipped); an entry cut in half mid-frame, which is waited for and then read whole when the rest lands, with nothing said about damage; midnight, where the last entry of the old day and the first of the new both arrive and a fixed-date follower stays put; and the torn-day comparison above. In `tests/export.rs`, the cross-process shape: one poptop writes the day with `--once --log=on` while another follows it and reads each record as it lands, then stops on SIGTERM. A day with nothing in it yet is waited for rather than refused, which is the one place `--follow DATE` differs from `--export DATE` (exit 1, "nothing recorded on").
