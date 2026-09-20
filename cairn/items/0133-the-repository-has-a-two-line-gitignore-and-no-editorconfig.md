---
id: 133
title: The repository has a two-line .gitignore and no .editorconfig
type: chore
status: done
milestone: r7
assignee: Oddur Sigurdsson
labels:
- repo
created: 2026-09-19
updated: 2026-09-19
priority: p3
---

## Problem

`.gitignore` is `/target`. Everything else a working checkout accumulates —
editor directories, `.DS_Store`, fuzzing artefacts, scratch files, and the
instruction files agent tooling drops in a repository — is one `git add -A`
away from being committed. This repository forbids tool attribution in
tracked files and enforces it three ways; an agent instruction file landing
by accident is exactly the case the enforcement exists for, and the cheapest
place to stop it is here.

There is no `.editorconfig`, so anyone whose editor is not configured for
this project gets tabs or trailing whitespace in a patch and finds out from
`cargo fmt --check`.

## Acceptance criteria

- [ ] `.gitignore` covers editors, OS files and a scratch directory
- [ ] `.editorconfig` matching what `cargo fmt` and the markdown already do
- [ ] RELEASING.md's reference to a README section that has moved is fixed

## How it was resolved

`.editorconfig` records what the repository already is, measured rather than
assumed: four spaces everywhere including the shell scripts (`check` has no
tabs in it), two in YAML, JSON and TOML, 80 columns for markdown prose —
the distribution tops out at 79 and 80 — and no trailing-whitespace stripping
in markdown, where two trailing spaces are a line break.

`.gitignore` covers editors, the files a desktop leaves behind, and a
`/scratch/` directory.

It deliberately does **not** list the instruction files assistant tooling
drops into a checkout, and cannot: writing one of those names into a tracked
file is exactly what `.githooks/no-attribution` refuses, and the first
version of this change failed `./check` on its own `.gitignore`. The check
was right. Such a file names its own tool in its contents, so the same check
refuses the commit anyway — the stronger guard was already there.

RELEASING.md's reference to a "Stability" section in the README now points
at `docs/design/recording-and-output.md`, where it moved in r6.
