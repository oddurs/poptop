---
id: 140
title: Ten tokens is not the whole screen
type: feature
status: done
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

- [x] Every new token is drawn somewhere a reader can see, and documented
- [x] A theme file written for the ten tokens renders exactly as it does today
- [x] `--check-theme` measures what the new tokens change; the three added are chrome and text variants, and the contrast table already covers what they inherit
- [x] A theme file changed while poptop is running is picked up, on the next sample, and `R` asks for it

## How it was resolved

**Three tokens, each inheriting what it shared:** `selection_fg` from `text`, `border` and `gap` from `chrome`. The inheritance happens once, in `Theme::new`, rather than in each of the seven constructors — which is what makes "a theme file written for the ten renders exactly as it did" true by construction rather than by seven copies agreeing. A test asserts it for both palettes at all four tiers.

Each is drawn where a reader can see it: the selected row's text (which was hard-coded to `text`, so a light theme had no way to fix an unreadable row), the panel borders, and the seam the timeline draws where sampling stopped. The shipped `themes/*.theme` files are regenerated with them.

**Reload.** `R` reads the theme file again, and a file that changes on disk is picked up on the next sample — one `stat` a second, beside the several hundred reads a sample already makes. A file that no longer parses keeps the colours on screen, because the half-applied theme of a file mid-edit is worse than the one you had, and the footer says which file was read and whether a line was ignored. A built-in has no file and says so rather than appearing to do nothing. A pty test writes a theme file while poptop is running and waits for the new colour to appear on screen.

**No new series hues, deliberately.** The item asked for `series_disk`, `series_net` and `series_swap`. The graphs alternate `series_cpu` and `series_mem` on purpose: the palette avoids green for colour-vision reasons, and what is left is warning-orange or too close to `ok`, so there is no third hue that survives the separation `--check-theme` measures. Alternating already guarantees the property that matters — two graphs touching never share a colour. Adding a hue is a colour-vision study, not a token, and `docs/reference/themes.md` now says so where a reader will look for it.

**Tests:** inheritance across palettes and tiers; each new token overridden on its own without disturbing what it inherits from; every token's name parsing back; the reload's three outcomes (built-in, unreadable file, success); and the pty test above. The docs tests hold the key reference to the new `reload-theme` action.

