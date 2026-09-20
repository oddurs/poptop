---
id: 140
title: Ten tokens is not the whole screen
type: feature
status: backlog
milestone: v1.1
created: 2026-09-20
updated: 2026-09-20
priority: p3
effort: m
area: theme
---

## Problem

A theme names ten colours. The screen has more: the disk and network series share `series_cpu`, the gap marker and the seam share `chrome`, the selected row's text is not separable from its background, and a reader who wants the timeline dimmer than the table cannot say so. A theme file is also read once, so trying a colour means restarting.

## Proposal

Add the tokens the screen actually distinguishes — `series_disk`, `series_net`, `series_swap`, `border`, `selection_fg`, `gap` — each inheriting from the token it shares today, so every existing theme file keeps its meaning. Reload the theme file when it changes, or on a key, so a colour can be tried without restarting.

## Acceptance criteria

- [ ] Every new token is drawn somewhere a reader can see, and documented
- [ ] A theme file written for the ten tokens renders exactly as it does today
- [ ] `--check-theme` measures the new tokens too
- [ ] A theme file changed while poptop is running is picked up, or the reason it is not is written down
