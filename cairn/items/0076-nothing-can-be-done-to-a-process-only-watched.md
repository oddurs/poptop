---
id: 76
title: Nothing can be done to a process, only watched
type: feature
status: backlog
milestone: v2.2
created: 2026-09-08
updated: 2026-09-08
priority: p3
area: ui
---

## Problem

atop has `k` to send a signal. htop, btop and bottom all have kill and renice.
poptop has no way to act on anything it shows.

The README lists this under "not there yet" and it has stayed there because
watching is the whole design. Worth being deliberate rather than leaving it
implicit.

## The argument against

A monitor that cannot change the machine is a monitor that cannot break it. That
is a real property: poptop can be run by anyone on anything without a second
thought about what a mis-key does, and its entire privileged surface today is
reading files.

## The argument for

The workflow "find the runaway process, kill it" is one gesture in every other
tool and two tools in poptop. Someone who has just used the constraint
suggestion and the detail view to identify the process has done the hard part;
sending them to another terminal to type a pid they read off a screen is where
the pid gets mistyped.

## What needs deciding

- Whether it happens at all. This is a positioning decision, not a technical one.
- If it does: which signals, what confirmation, and whether the confirmation
  names the process rather than the pid — the pid is what gets misread, and
  poptop knows the command line.

## Acceptance criteria

- [ ] Decided in writing, either way, with the reasoning
- [ ] If built: the process is named, not just numbered, in the confirmation
- [ ] If declined: the README says so and why, rather than listing it as missing
