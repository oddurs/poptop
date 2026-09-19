---
id: 100
title: A torn log entry can pass for whole when the next torn write fills it out
type: bug
status: done
milestone: r1
assignee: Oddur Sigurdsson
labels:
- validation
created: 2026-09-18
updated: 2026-09-18
priority: p3
effort: m
area: log
---

## Problem

Found by `fuzz/log_torn` while fixing 0085. Suppose a day-log entry is cut short by a crash, and the next append is also cut short. If the second fragment supplies exactly the bytes the first was missing, the first entry's length is satisfied and its bytes decode. If a whole entry comes after them, it starts exactly where the first entry claimed to end. The reader can't tell anything was torn, and returns a sample whose last few fields came from the other write.

For this to happen, the first write has to tear within about 13 bytes of its end. With 14 or more bytes missing, the next entry's header and magic fall inside the claimed span, and the reader rejects it. The second write must also be torn. The case is rare, but when it happens the reader returns wrong numbers without any warning.

## Proposal

Add a checksum per entry (CRC32 of the block, beside the length). Frame each entry as `len, crc, block`. The reader can tell new frames from old ones by where the magic sits (offset 8 rather than 4), so existing logs keep reading. An older poptop reading a new log counts the new entries as "a different version". That matches what 0059 promised.

## Acceptance criteria

- [x] New entries carry a checksum; old logs still read
- [x] `a_torn_entry_filled_out_by_the_next_torn_write_is_a_known_limit` flips to asserting the torn entry is dropped
- [x] `fuzz/log_torn` asserts exactly the whole entries come back, with no allowance for torn ones
- [x] Measured cost per entry, in bytes and in append time

## How it was resolved

The checksum sits inside the length, after the block: `len, block, sum`. No new framing was needed. The proposal assumed an older poptop would count checksummed entries as "a different version", but it doesn't. Its reader trusts the length, decodes the block with `decode_reporting`, and never asks the decoder to reach the end, so the four bytes after the block go unnoticed. `a_log_with_checksums_reads_in_a_poptop_from_before_them` runs that reader against new entries. This reader checks the sum when it's there and reads an entry without one the old way, so days written before this, or across the upgrade, read whole (`a_day_written_across_the_upgrade_to_checksums_reads_whole`).

A torn checksummed entry fails twice. Its sum doesn't match, and read the old way its block would have to decode to exactly four bytes more than it is. `a_torn_entry_filled_out_by_the_next_torn_write_is_caught_by_its_checksum` covers cuts of 1 to 100 bytes. The old-framing case stays pinned as a known limit, since nothing in those entries can say.

The sum is a word-at-a-time multiply-rotate, not a CRC. A bytewise CRC is slower than decoding the entry it guards. Measured on 12 MB: the reader takes 11.3 ms against 7.9 ms for decoding alone, so about 4 GB/s for the sum. Each byte of a block is shown to change it (`every_byte_of_an_entry_is_in_its_checksum`). It guards against accidents, not adversaries: anyone who can write the file can write a matching sum.

`fuzz/log_torn` now picks each entry's framing from its input, and asserts exactly the whole entries for checksummed ones.

Cost per entry: four bytes. That's 0.1% of an entry at twenty processes and less at four hundred. Append time: one sum over a block already in memory.
