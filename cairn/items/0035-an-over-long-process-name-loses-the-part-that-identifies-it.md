---
id: 35
title: An over-long process name loses the part that identifies it
type: bug
status: backlog
milestone: v1.0
created: 2026-09-07
updated: 2026-09-07
priority: p2
area: ui
---

## Problem

The column that says *which process this row is* is the worst-funded on the
line. At 104 columns with the IO columns shown, `COMMAND` gets 19 — one more
than the two disk-rate columns together — and it is truncated from the right:

```text
5613    oddurs     20.0   ▊     514.0M   ▏    S  27   …   Google Chrome Helpe
1063    oddurs     6.3    ▎     101.3M        R  28   …   Google Chrome Helpe
84474   oddurs     3.6    ▏     46.2M         R  24   …   Google Chrome Helpe
```

Three different processes, three identical rows. The part that tells them apart
— `(Renderer)`, `(GPU)`, `(Network Service)` — is exactly the part cut off.

## What should happen

Elide the middle, not the tail: `Google…(Renderer)`. Both ends of a name carry
identity — the head says what program, the tail says which of them — and cutting
either one alone loses a distinction the other cannot supply. The same reasoning
already governs `short_mount` for mount points in the header, where the tail is
what identifies a path.

Worth considering alongside: whether the two magnitude bars should give up their
columns before the identity does. A bar is decoration — the number beside it
already carries the magnitude — and decoration crowding out an identifier is the
wrong way round.

## Acceptance criteria

- [ ] A name too long for its column keeps both ends
- [ ] Processes distinguished only by a suffix are distinguishable on screen
- [ ] A name that fits is untouched, with no elision mark
