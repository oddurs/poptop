---
id: 51
title: Scrubbing loses the process you were watching
type: bug
status: backlog
milestone: v1.0
created: 2026-09-07
updated: 2026-09-07
priority: p2
area: ui
---

## The lesson, and why it lands harder here

htop has `F` — follow a process, keeping it selected as the table reorders
around it. btop calls the same thing "follow specific processes during detailed
inspection".

In a live-only monitor that solves one problem: the row you are reading moving
under you when the sort reshuffles. In poptop it would solve a second one that
no other tool has, because no other tool has the buffer.

Scrubbing back through a spike, the table is re-sorted at every sample. The
process you are trying to watch moves from row 3 to row 11 to off-screen, and
the selection follows the *position*, not the process. So the one gesture the
tool exists for — find the moment it went wrong, then watch what that process
was doing around it — is the gesture that loses your place.

## What it should do

Selection keyed on the process rather than the row: hold `(pid, started)`, the
identity `ProcSample::key` already provides for exactly this reason, and resolve
it to a row on each frame.

That also answers what to do when the followed process is not in the sample
under the cursor: it is not "select something else", it is "this process did not
exist yet", which the panel can say. A process appearing partway through the
buffer is information — often the information.

## What needs deciding

- Whether following is a mode (`F`) or simply what selection means. The second
  is less to explain and is probably right: a selection that tracks a row number
  is not a selection of anything.
- What the row does when the process is absent at the cursor — a held place, or
  a stated absence.
- Whether it survives a sort change, a filter change, and leaving tree mode.

## Acceptance criteria

- [ ] Selection is of a process, not a row index
- [ ] Scrubbing keeps the same process selected while it exists
- [ ] A process absent at the cursor is stated, not silently swapped
- [ ] Sorting and filtering do not move the selection to a different process
