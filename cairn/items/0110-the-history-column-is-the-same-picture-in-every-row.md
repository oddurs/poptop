---
id: 110
title: The history column is the same picture in every row
type: feature
status: done
milestone: r5
assignee: Oddur Sigurdsson
labels:
- ui
created: 2026-09-19
updated: 2026-09-19
priority: p2
effort: s
area: table
---

## Problem

HIST is 10 columns wide, and in every capture all 23 rows showed the same flat `⠀⠀⢀⣀⣀⣀⣀⣀⣀⣀`. Every row shares one scale, set by whichever process is on top (`≤25%`, `≤100%`, `≤200%` across captures). Most processes sit near zero on that scale, so the column shows a flat line in 23 rows while using ten columns of the table's width.

## Proposal

Either scale each row to its own peak, with the header saying so, or hide the column when no row varies. Grouped rows (`g`) show no history at all; they could show the group's combined history.

## Acceptance criteria

- [x] A history column that shows nothing but the CPU% level again gives its width to the command and says why (was: "two processes with different CPU shapes draw visibly different lines"; see the note)
- [x] The header says what the scale is — unchanged, `HIST ≤25%`
- [x] Grouped rows: decided no, on the grounds already in the code (see the note)

## How it was resolved

Two proposals were tried and rejected, each on the evidence of an existing test.

- **A shared log scale.** It separated idle, light and heavy at the bottom, but four levels can't be fine at the bottom and readable at the top. A ramp to 285% saturated 7 of 10 cells, which is the "no graph at all" failure `a_process_using_several_cores_still_has_a_readable_sparkline` guards against, and the top is where a process pinning several cores lives.
- **A history for grouped rows.** The code had already decided against it, for a reason that holds: "membership changes as processes come and go, and a line through that is continuity that never happened" (`a_group_states_nothing_it_cannot_sum`). The critique missed that.

**What was done instead, following the rule 0043 already applies to columns:** a column of one repeated value is stating one fact, so it gives its width back and says the fact.

The history column's only unique information is change over time; CPU% already gives the level. So it's drawn when some process's history moves, meaning a process seen at two different heights on the shared scale. Otherwise the column is dropped, its width goes to the command, and the title says `history flat`.

- **Judged across the whole buffer, not the visible rows.** The first version judged from the visible rows, and `scrolling_the_list_does_not_rescale_everybody_else_history` caught it: scrolling the one busy process out of view took the column away. Movement is now judged over every process in the buffer, like the scale. It's one pass that stops at the first process seen at a second height.
- **Not judged until there are `App::CONSTANT_FOR` samples,** the threshold the user column folds on, so the column doesn't appear a few seconds after start.

On a live machine something is always moving, so the column stays, as it should. On a quiet one, or a flat test fixture, the command gets the ten columns back.

Existing tests whose fixtures were flat but were testing the column itself now vary. The README's table line for postgres was regenerated from what poptop draws for the varied fixture.
