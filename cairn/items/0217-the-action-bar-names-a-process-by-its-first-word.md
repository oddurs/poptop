---
id: 217
title: The action bar names a process by its first word
type: bug
status: backlog
milestone: r8
labels:
- ui
created: 2026-09-20
updated: 2026-09-20
priority: p2
---

## What happens

With `UA Mixer Engine -silent` selected, the action bar reads:

```
UA · 789  ⏎ inspect · no signal: signals off
```

The process is identified as `UA`.

## What should happen

The bar exists to say what the reader has selected, so the part it keeps has to
be the part that identifies it.

## Reproduction

`selection_actions` takes `name.split_whitespace().next()`, then the last path
component. That is `argv[0]` thinking: it is right for
`/usr/lib/foo/bar --flag` and wrong for every macOS process whose *name*
contains spaces — `UA Mixer Engine`, `Google Chrome Helper`, `OrbStack Helper`,
which is most of what is running on a Mac.

## Proposal

Elide the name to the room the bar has, the way the table's `COMMAND` column
does, rather than cutting at the first space. The bar has forty-odd columns and
is spending two.
