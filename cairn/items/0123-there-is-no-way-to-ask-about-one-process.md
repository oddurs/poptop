---
id: 123
title: There is no way to ask about one process
type: feature
status: backlog
milestone: v4.0
created: 2026-09-15
updated: 2026-09-15
priority: p1
area: ui
---

## Problem

Activity Monitor's ⓘ opens a panel about one process: its full command, parent,
user, every figure the tabs show, open files, ports. It is where you go when the
table has told you *which* process and you now need to know *what* it is.

poptop's `d` replaces the timeline with that process's history — which is a
different and genuinely better thing, and it is not this. The full command is
truncated in the table and available nowhere. The parent is known and never
shown outside the tree. `--export` has every field and is not a thing you read
while looking at a row.

## The fix

An inspector: one process, everything poptop holds about it, over the table.

```
  ╭ postgres · 824 ──────────────────────────────────────────╮
  │ /usr/local/pgsql/bin/postgres -D /var/db/postgres        │
  │                                                          │
  │ user     oddurs          started   14:02:11 (2h 14m)     │
  │ parent   1 · launchd     state     S · sleeping          │
  │ threads  4               nice      0                     │
  │                                                          │
  │ cpu      88.4%   peak 102.3%   over the last 10m         │
  │ memory   512.0M  peak 640.1M   growing 2.1M/min          │
  │ disk     1.2M/s read · 0 written                         │
  ╰──────────────────────────────────────────────────────────╯
```

The peaks are the part Activity Monitor cannot do: they come from the buffer,
and they are the answer to "is this normal for it".

## What needs deciding

- **Whether it replaces the timeline or floats over the table.** Floating keeps
  the graph, which is the thing worth keeping.
- **What it does while scrubbing.** The figures should be the cursor's moment,
  and the peaks should be the whole buffer — two time bases in one panel, which
  needs saying on the panel rather than being inferred.
- **Open files and ports.** Both are a `lsof`-shaped problem and neither is
  cheap. Probably not in the first version, and the panel should not look like
  it is missing them.

## Acceptance criteria

- [ ] The full command is readable without exporting
- [ ] Parent, user, state and start time are all in one place
- [ ] Peaks come from the buffer, and say what window they cover
- [ ] Opens and closes without disturbing the timeline
