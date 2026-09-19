---
id: 85
title: A log directory poptop did not make is trusted as if it had
type: bug
status: done
milestone: r1
labels:
- validation
created: 2026-09-18
updated: 2026-09-18
priority: p1
effort: m
area: persist
---

## Problem

`persist.rs` and `log.rs` read and write under a directory that can be shared, symlinked, full, read-only, or filled with files poptop did not write. It is not known what happens in each case. The concerns are a symlinked day file pointing somewhere else, a 10 GB file with a valid header, a disk that fills halfway through a write, and two poptops logging to the same directory.

## Proposal

List each hostile or unlucky state, write a test that creates it in a temp directory, and record the behaviour. The expected behaviour is one of three: refuse with a message, skip with a warning, or recover. Silent data loss and following a symlink out of the directory are not acceptable.

## Acceptance criteria

- [x] A write interrupted at any byte (simulated) leaves the previous day readable
- [x] ENOSPC and EACCES while logging produce one warning, not a crash or a flood
- [x] A symlink in the log directory is not followed outside it, or the decision to follow it is documented and tested
- [x] Two poptops writing one directory either lock or interleave safely; a test proves which
- [x] A file that is not a day poptop could have written is refused without being read into memory (was: "far larger than any day"; see the note below)

## What was done

**Found and fixed: a crash cost the rest of the day.** A write cut short, with a later append after it, still carries the length it meant to have. The reader trusted that length, landed inside the next entry, and every length it read from there was noise, so a crash in the morning silently lost the afternoon. `read_blocks` no longer trusts a length on its own:

- An entry is read only if it decodes using exactly the bytes its length names (`store::decode_exactly`).
- That isn't sufficient, because a fragment can borrow its missing bytes from the next entry and decode into a sample whose last fields belong to that entry. What gives it away is that a whole entry starts inside the span it claimed. This is checked when a step fails or the day ends, not up front. Checking every entry up front cost 28.5 ms against 7.8 ms of decoding for 12 MB. Checking lazily costs almost nothing (`measure_reading_a_day`).
- When a length can't be trusted, reading resumes at the next whole entry. That entry is found by its magic, and it must also have a version and a schema block this build can parse (`store::starts_store`).

**Found and fixed on the way, by a test written for it: a process name could erase the log.** The strict check exists because the magic alone wasn't enough. The store writes each string as a length and its bytes, so a process named `poptophist` looked like an entry starting inside a whole one. The reader then threw the real entries away as torn. Anyone who can name a process could have erased a stretch of somebody's log. A schema block is about 1.5 KB with zero bytes in places no C string can put one, so no string can pass for it. `a_process_named_after_the_magic_cannot_erase_the_log`.

What follows an entry is no evidence about it, since any number of later crashes can leave fragments there. `fuzz/log_torn` builds days from whole and torn entries and checks that every whole entry comes back, in order. It found five distinct failures on the way to this design, and each is now a unit test. One case remains that no reader can detect without a checksum; see 0100.

**Found and fixed: `/dev/zero` and FIFOs.** A day or store file that was a link to `/dev/zero` was read until the OS killed the process for memory, and a FIFO blocked `--read` forever. `store::read_regular` refuses anything that isn't a regular file, and reads a regular file only as far as its length at open. Both the store and the day logs use it.

**Criteria.**

1. Every cut through an entry, with and without later appends, is tested: `a_write_cut_short_*`, `a_torn_entry_*`, `a_whole_entry_followed_by_many_crashes_is_still_read`.
2. EACCES (a read-only directory) and ENOSPC (a day linked to `/dev/full`, Linux) come back from `append` as errors. `run` records each distinct note once, so a full disk is reported once, not once per interval.
3. Links are followed, on purpose: pointing the log directory at a bigger disk is reasonable. Pruning removes a link, never what it points at. Tested in `a_log_directory_that_is_a_symlink_*`.
4. Two writers interleave safely. Each append is one `write` to an `O_APPEND` file, and a test runs two threads × 50 appends and reads back all 100.
5. **Decided: a regular file is read whole.** The criterion as first written, refusing a file far larger than any day poptop could write, turned out to protect nothing. The samples a day decodes into are at least as large as its bytes (`a_day_in_memory_is_no_smaller_than_its_file`), so capping or streaming the raw read wouldn't bound memory; it would only move the peak. What actually threatens memory is a file that isn't a day at all, such as `/dev/zero`, a FIFO, or a file still growing. Those are refused. A real day costs what it costs, and it's as large as the `log-bytes` its writer allowed. Capping at `log-bytes` would also have refused days written under a larger budget that was later lowered.
