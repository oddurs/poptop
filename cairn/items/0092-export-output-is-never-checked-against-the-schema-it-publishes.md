---
id: 92
title: Export output is never checked against the schema it publishes
type: chore
status: backlog
milestone: r3
labels:
- testing
created: 2026-09-18
updated: 2026-09-18
priority: p1
effort: s
area: export
---

## Problem

0074 added JSON and line-format export "with its schema". Consumers will write parsers against that schema. Nothing checks that what poptop emits conforms to what it says it emits, or that the schema does not change without the change being noticed.

## Proposal

Validate every exported JSON record from a live sample and a recorded store against the published schema in a test. Keep a golden copy of the schema so any change to it appears as a diff in review. Decide and document what counts as a breaking change.

## Acceptance criteria

- [ ] A test validates export output from a real sample against the published schema
- [ ] The schema is snapshot-tested; changing it requires updating the golden file
- [ ] The line format has a stated grammar and a test that parses its output back
- [ ] The compatibility promise for the schema is written in the docs
