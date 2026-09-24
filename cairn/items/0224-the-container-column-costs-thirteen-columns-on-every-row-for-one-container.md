---
id: 224
title: The container column costs thirteen columns on every row for one container
type: feature
status: backlog
milestone: r8
labels:
- ui
created: 2026-09-20
updated: 2026-09-20
priority: p3
---

## Problem

`CID` is drawn when *any* process is in a container. One container on a box of
761 processes means twelve columns of whitespace and a separator on all the
rows that are not in one.

`USER` folds when every value is the same. `CID` does not fold when almost
every value is absent, which is the same waste from the other end.

## Proposal

Draw it when enough rows can fill it to be worth the width — the same shape of
test as `App::one_user`, measured against the rows on screen rather than the
whole sample. Below that, the container is a fact about a few rows and belongs
where the other per-row facts that do not deserve a column go: the inspector.
