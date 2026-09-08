---
id: 65
title: Close on the empty buffer the page opened by filling
type: feature
status: done
milestone: web
depends_on:
- 56
created: 2026-09-08
updated: 2026-09-08
priority: p2
effort: s
area: web
---

## Problem

The page currently ends with an install command and a list of links. That is a
functional close and a narratively flat one: the last thing a reader sees is
navigation.

## Proposal

End where the story began. The install line, the platform truth stated plainly —
Linux reads `/proc` directly, macOS goes through sysinfo and is the platform
poptop is developed on, one binary and nothing written to disk unless you ask —
and beneath it a small frame that is **empty**.

Caption: *It starts empty. Then it starts remembering.*

The empty frame is the honest picture of the first second after `poptop` starts,
and it closes the loop with the hero, which opened on a buffer that had already
filled.

Keep the onward links, below it, quiet.

## Acceptance criteria

- [ ] The close is the install line, the platform truth, and an empty frame
- [ ] The empty frame is genuinely the renderer with no samples, not an image
- [ ] Onward links remain but do not lead
