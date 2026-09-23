---
id: 255
title: A layout cannot be saved, named or shared
type: feature
status: backlog
milestone: r13
labels:
- ui
created: 2026-09-23
updated: 2026-09-23
priority: p1
---

## Problem

The layout is a build-time constant, so there is nothing to save. Once there is
a tree there is a thing worth keeping: the arrangement somebody built while
watching an incident is exactly what they want next time, and what they want to
send to the person on the next shift.

## Proposal

Layouts are configuration, like themes and key maps: a named tree in the config
file, and `--layout=storage` to open one. Presets ship for the shapes people
actually watch — overview, storage, network, containers, one process — and are
the documentation for the format, because a preset that is also an example
cannot go stale.

## Acceptance criteria

- [ ] A layout is TOML in the config file, and `poptop --layout=NAME` opens one
- [ ] Shipped presets, reachable by name, with the file that defines them checked by the same test that checks the config reference
- [ ] The current stack is a preset, and the one poptop opens with on a narrow terminal
