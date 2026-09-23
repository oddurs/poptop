---
id: 237
title: The loudest ink on the screen is furniture
type: feature
status: backlog
milestone: r10
labels:
- ui
created: 2026-09-23
updated: 2026-09-23
priority: p0
---

## Problem

Two full-width rules, each carrying a sentence:

```
── processes (780) · 41 fish (g folds them) ! io: 237/780 need root ──────────
── timeline — 4s of 10m00s buffered ──────────────────────────────────────────
```

They are the heaviest lines on the screen and they carry three different kinds
of thing at once: what the region is, a hint about a key, and a warning about
what could not be read. The eye is drawn to the furniture and then has to sort
out which part of it was news.

## Proposal

A divider names its region and says nothing else. It stays full width — a rule
that stops short reads as a box missing its corners — but it is drawn in the
quietest ink on the screen and holds one label.

Everything else it was carrying goes where it belongs:

- what could not be read, and why, is a *notice*: one line, only when there is
  something to say, in the colour that means "poptop could not look".
- a hint about a key belongs with the thing the key does, not in structure.
- the buffer's span belongs on the timeline's own axis row, which already says
  `4s shown, 1s/slot`.

## Acceptance criteria

- [ ] A divider holds a label and nothing else
- [ ] Notices appear only when there is something to say, and read as messages rather than as structure
- [ ] Nothing that was being said is lost — each has a stated new home
- [ ] The rule is drawn in dim, not in the panel's foreground
