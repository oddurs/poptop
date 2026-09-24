---
id: 260
key: r15
title: Bring your own glyphs
type: milestone
status: backlog
labels:
- ui
created: 2026-09-23
updated: 2026-09-23
priority: p2
---

The last milestone, and the smallest: the sets themselves become something a
reader can supply, and the patched fonts many readers already have get used for
what they are good for.

r11 makes a set data rather than code. This makes that data a file, the way
themes are files, with the same validation and the same reload key.

Nerd Fonts are the other half, and the answer is narrower than people expect.
The icons are decoration; poptop's figures are not decorated. But a patched
font does carry a vetted set of marks that read well as *accents* — the tab
strip, the chips in the header, the tree's spine — and a reader who has one
should get that. Under two rules: nothing load-bearing, ever, because meaning
never rests on a glyph the reader's font may not have; and every accent is
measured for width before it is used, because an ambiguous-width glyph breaks a
fixed grid and that is the bug class this codebase has fought twice.
