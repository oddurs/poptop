---
id: 233
title: Nothing says whether the machine is on battery
type: feature
status: backlog
milestone: r9
labels:
- collect
created: 2026-09-22
updated: 2026-09-22
priority: p2
---

## Problem

On a laptop the question behind "why is this slow" is often "because it is on
battery and the OS is holding it back", and the question behind "should I kill
this" is often "what is it doing to the battery". poptop shows neither.

## Proposal

A `Power` source: charge, whether it is charging, and the rate energy is going
in or out where published — `/sys/class/power_supply` on Linux, the power
sources API on macOS. Nothing shown on a machine with no battery.

## Acceptance criteria

- [ ] Charge, state and draw where published, on both platforms
- [ ] Absent on a desktop, not zero
- [ ] Recorded, so a day shows when the machine was unplugged
