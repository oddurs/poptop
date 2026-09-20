---
id: 117
title: The site is a second place for a claim to go stale
type: chore
status: done
milestone: r6
labels:
- docs
created: 2026-09-19
updated: 2026-09-19
priority: p1
effort: s
area: docs
---

## Problem

A web app for poptop lives on `feat/0055-the-poptop-website`: 13,168 lines under `web/`, with its own CI workflow. It was never merged, nothing on `master` links to it, and GitHub Pages is not configured. What it would do if finished is restate the README on a second surface — a second thing to build, a second thing to keep true, and the one people will find first when it is wrong.

The branch also carries two files worth keeping, which have nothing to do with the site: `CONTRIBUTING.md` and `CODE_OF_CONDUCT.md`.

## Proposal

Delete the branch, recording its head so it can be brought back. Salvage the two files onto `master`. Give the repository the furniture a reader expects instead: issue and pull request templates, a security policy, and a description and topics on the repository itself.

## Acceptance criteria

- [x] The website branch is gone, and its head recorded here
- [x] `CONTRIBUTING.md` and `CODE_OF_CONDUCT.md` are on `master` and say nothing about a site
- [x] Issue templates, a pull request template and `SECURITY.md` exist
- [x] The repository has a description and topics

## How it was resolved

The branch is deleted. Its head was `33348daf4e96fa29a3b1f9ee0310c9bfcdaa727e` — 13,168 lines under `web/` plus a `site.yml` workflow — and it can be brought back from there if the decision is ever revisited. Nothing on `master` referenced it, and GitHub Pages was never configured, so nothing else had to change.

`CONTRIBUTING.md` and `CODE_OF_CONDUCT.md` are salvaged onto `master` unchanged: both are about the work rather than the site, and every path they cite (`./check`, `.githooks`, `docs/roadmaps/`, `ROADMAP.md`) exists here.

New furniture: issue templates for a wrong figure and for a question poptop cannot answer — both asking for the machine's own evidence, since that is what a report of a wrong number needs — a pull request template that asks for the check and the evidence, and `SECURITY.md`, which says what poptop does to a machine (reads; signals and history only when asked) and how to report something privately.

The repository has a description and ten topics.
