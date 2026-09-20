---
id: 131
title: Nothing watches the dependencies or the actions CI runs
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

Four direct dependencies, each pinned to an exact minor in `Cargo.toml`, and
nothing that notices when one of them publishes a fix. The workflows use
`actions/checkout@v4` and friends by major version, which is the convention
but means CI's behaviour can change without a commit here.

## Acceptance criteria

- [ ] `.github/dependabot.yml` covering `cargo` and `github-actions`
- [ ] Weekly, grouped, so it is not a stream of single-line pull requests
- [ ] The commit message prefix matches this repository's convention

## How it was resolved

`.github/dependabot.yml`: cargo weekly for the crate, cargo monthly for
`fuzz/` — which has its own lockfile — and github-actions monthly, grouped
into one pull request so CI's pins move together. Patch bumps are grouped
too: a patch bump of a transitive crate is not a decision.

Prefixes match the convention here: `chore(deps)` and `chore(ci)`.
