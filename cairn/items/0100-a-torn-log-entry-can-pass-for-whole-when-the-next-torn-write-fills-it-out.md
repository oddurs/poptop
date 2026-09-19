---
id: 100
title: A torn log entry can pass for whole when the next torn write fills it out
type: bug
status: backlog
milestone: r1
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

- [ ] New entries carry a checksum; old logs still read
- [ ] `a_torn_entry_filled_out_by_the_next_torn_write_is_a_known_limit` flips to asserting the torn entry is dropped
- [ ] `fuzz/log_torn` asserts exactly the whole entries come back, with no allowance for torn ones
- [ ] Measured cost per entry, in bytes and in append time
