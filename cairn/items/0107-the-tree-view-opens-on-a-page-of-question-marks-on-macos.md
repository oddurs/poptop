---
id: 107
title: The tree view opens on a page of question marks on macOS
type: bug
status: done
milestone: r5
assignee: Oddur Sigurdsson
labels:
- ui
created: 2026-09-19
updated: 2026-09-19
priority: p1
effort: s
area: ui
---

## What happens

Pressing `t` as a normal user on macOS fills every visible row with root-owned daemons that poptop can't read: `cfprefsd`, `trustd`, `backupd`, `automountd`… each showing `0.0  0B  ?  —  —  ?`. No tree lines are visible, and none of the user's own processes appear on the first screen. The view looks broken the moment it opens.

The row also contradicts itself. Memory reads `0B` and CPU `0.0`, while threads and disk read `—`. A process poptop can't read isn't one using zero bytes.

## What should happen

- A value poptop can't read shows `—` in every column, memory and CPU included.
- The tree is ordered so the first screen shows something: branches that contain the user's own processes, or the most active branches, first. Unreadable leaves go last, or fold into a single "N processes not readable without root" row.
- The tree lines are visible from the first row.

## Reproduction

1. Run poptop on macOS as a normal user.
2. Press `t`.

## How it was resolved

The cause: sysinfo can't read another user's processes on macOS and returns each one with parent 0. Pid 0 isn't in the list, so all 189 of those daemons became roots. `launchd` was one of them too: unreadable, 0% CPU, and the parent of every process the user owns. Roots were ordered by their own figures, so `launchd` tied with 189 zeros, and the user's work sat somewhere below them.

Three changes:

- **A root sorts by its whole branch.** `tree::branch_peaks` finds the member of each root's branch that sorts first under the current sort, and roots are ordered by that. It's iterative and visits each process once, so a deep chain can't overflow the stack and a cycle can't loop. On this Mac the tree now opens on `launchd` with the user's processes directly beneath it.
- **Unreadable is not idle.** `ProcSample::unmeasured()` means no resident memory and no thread count together. That pair only occurs when the kernel withheld the figures: a kernel thread has zero memory but still has a thread count. Such a row now shows `—` for CPU and memory, where it showed `0.0` and `0B` beside a `—` for threads. The CPU and memory sorts put it after processes that are really idle.
- **No stray spines at root level.** Roots are drawn without connectors, but their children drew a `│` in the first column under every root but the last, which joined nothing to nothing. The root's level is now always blank.

Tests: `the_busy_branch_comes_first_whatever_its_root_shows` models the macOS shape (unreadable launchd holding the work, unreadable daemons as roots) and fails on the old code. `a_process_the_kernel_would_not_describe_shows_dashes_not_zeros` renders the row and checks its position.
