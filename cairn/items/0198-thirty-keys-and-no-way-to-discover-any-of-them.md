---
id: 198
title: Thirty keys and no way to discover any of them
type: feature
status: done
milestone: v3.6
created: 2026-09-15
updated: 2026-09-15
priority: p0
area: ui
---

## Problem

The footer named six keys. There were about thirty. The rest were in the README,
which is not open when the machine is.

## What was built

`F10` opens a bar — File, Edit, View, Go, Process — with `Alt` plus the first
letter going straight to a title. Every item states the key that also runs it.

The part that is not the drawing: every command is now a named `Action` in
`command.rs`, and the keyboard and the menu both dispatch through
`Action::apply`. `handle_key` became a table mapping keys to actions.

## Why that mattered more than the menu

A menu built as a second implementation works, disagrees with the keyboard, and
says nothing about it. `every_menu_item_agrees_with_the_key_beside_it` walks the
bar and checks each hint against what that key actually does.

## Acceptance criteria

- [x] The bar is always on screen, so it can be found
- [x] Arrows navigate, Enter chooses, Esc closes
- [x] The highlight never lands on a separator
- [x] An open menu takes every key
- [x] It cannot open over the filter, jump or signal boxes
- [x] Menu and keyboard cannot drift, by test
