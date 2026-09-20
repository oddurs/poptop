---
id: 225
title: Eliding the middle removes the part of a command that identifies it
type: feature
status: backlog
milestone: r8
labels:
- ui
created: 2026-09-20
updated: 2026-09-20
priority: p1
---

## Problem

`elide_middle` keeps the head and the tail. For a program path that is right —
the head is the name. For a command *line* it is close to the worst choice
available, because the head is the program every one of them shares and the
tail is whatever the last argument happens to end with:

```
claude --dangero…skip-permissions
claude --resume …97b-a1ebc3ceb02d
poptop --interva…l=60s --store=on
OrbStack Helper …872c248 -handoff
```

Two `claude` processes, and what distinguishes them has been cut out of the
middle and replaced with the flag fragments that do not. The README's own
argument for showing the command line is that four `node` services sort as four
identical `node`s; this elision undoes that for exactly those rows.

It also cuts inside words — `--interva…l=60s` — which reads as a typo rather
than as an omission.

## Proposal

The name and the arguments are different things and should be elided
differently. Keep the program whole, then spend what is left on the arguments
from the *left*, cutting at an argument boundary:

```
claude --dangerously-skip-perm…
claude --resume 3f2a…
poptop --interval=1s --window=…
```

The first token is what the eye anchors on and it is short; the discriminating
argument is usually the first or second, not the last.

## Acceptance criteria

- [ ] Two processes of the same program, differing in their first argument,
      are told apart at 80 columns
- [ ] No elision falls inside a word where an argument boundary was available
- [ ] A bare program name with no arguments is unchanged
