---
id: 101
title: The single-binary claim borrows the tool's rhetoric
type: docs
status: done
milestone: web.1
created: 2026-09-08
updated: 2026-09-08
priority: p2
effort: s
area: web
---

## Problem

`web/README.md` justifies embedding content, fonts and stylesheets with "the
same property that lets poptop run with nothing set up first". The artifact that
deploys is 47 static files in a directory. Nobody was ever going to lose a
content directory.

The architecture is sound; the sentence justifying it is borrowed. The tool's
version of that principle is about a user's machine at three in the morning. The
site's version is about a build directory, and using the same words spends
credibility on a property nothing depends on.

## Proposal

Keep the design, cut the analogy. Say what it actually buys, which is checkable:
no build step, no asset pipeline, no npm in the deployed path, and an export
that is byte-identical between builds because everything it draws from is
compiled in.

## Acceptance criteria

- [ ] The README claims only what a reader can verify
- [ ] The genuinely unusual property — a single static binary with `/healthz` —
      is still stated, because it is true
