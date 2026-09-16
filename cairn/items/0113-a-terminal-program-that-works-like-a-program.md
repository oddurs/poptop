---
id: 113
title: A terminal program that works like a program
type: milestone
status: todo
milestone: v3.6
created: 2026-09-15
updated: 2026-09-15
priority: p1
area: ui
---

## The thesis

poptop had around thirty single-key bindings and no way to find out about any of
them except reading a file. That is normal for a terminal monitor and it is
still bad: the moment you need a system monitor is the moment you have no
patience for learning one.

Fresh — a terminal IDE — makes the opposite bet: menus, mouse, a command
palette, panels, all inside a terminal, all discoverable without a manual. The
bet pays because none of it costs the keyboard anything. The keys stay; the
menu is a *second surface over the same commands*, and that is the whole design
constraint. Two surfaces, one list of commands (`command.rs`), or they drift.

The menu bar (item 114) is done. What follows is the rest of the shape.

## Acceptance criteria

- [x] Every command is reachable without knowing a key
- [x] Every menu item states the key that also runs it, checked by a test
- [ ] The mouse works where a terminal reports it
- [ ] A command palette finds a command by name
- [ ] Panels can be focused, resized and zoomed
- [ ] None of it costs a keystroke to anyone who already knows the keys
