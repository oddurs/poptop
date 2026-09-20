---
id: 144
title: A day being written cannot be followed
type: feature
status: backlog
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

- [ ] A follower reads entries appended after it attached, in order, without repeating any
- [ ] An entry half-written when the follower reaches it is waited for, not skipped or reported as damage
- [ ] Following a day that rolls over at midnight moves to the new file
- [ ] A test writes a day from one process and follows it from another
