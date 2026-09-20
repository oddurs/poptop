---
id: 129
title: There is no changelog, so nothing says what changed between two versions
type: docs
status: done
milestone: r7
assignee: Oddur Sigurdsson
labels:
- repo
created: 2026-09-19
updated: 2026-09-19
priority: p2
---

## Problem

125 cairn items and 130-odd commits, and no file that answers "what changed".
`ROADMAP.md` is what is planned; `git log` is every step including the ones
that were reverted. Neither is a release note.

This matters more than usual here because the store and the export schema
have stability rules — RELEASING.md step 3 asks whether a schema change is
compatible or breaking — and there is nowhere for that answer to be written
down.

## Acceptance criteria

- [ ] `CHANGELOG.md`, Keep a Changelog shape, with an Unreleased section
- [ ] The work already done is summarised honestly rather than invented
- [ ] Store and schema compatibility get their own line when they change
- [ ] RELEASING.md points at it

## How it was resolved

`CHANGELOG.md`, in Keep a Changelog shape. Nothing has been released, so
there is one `Unreleased` section and no invented history.

It opens with the two promises that outlive the process — that a newer
poptop reads an older one's store and log, and that export fields are added
rather than renamed — and says that a change to either gets a line here.
RELEASING.md step 3 now points at it, and at the **Stability** rules in
`docs/design/recording-and-output.md`, which is where they moved when the
README was split.

The "Fixed" list is taken from the commits, not written from memory: twelve
defects that produced a wrong answer rather than a missing one.
