---
id: 159
title: There is no one-sentence answer
type: feature
status: backlog
milestone: v6.1
depends_on:
- 140
- 154
- 157
created: 2026-09-13
updated: 2026-09-13
priority: p0
area: ui
---

## Problem

The question every one of these tools is opened to answer is four words long:
**what is wrong here**. poptop's answer is a screen, and assembling the sentence
from it takes a person who knows the machine.

By the end of v4 poptop holds every piece of that sentence: which figure is
constrained, which process accounts for it, whether this is normal for the hour,
whether it recurs, where it is heading, and what stopping it would return. They
are in six different panels.

## What it does

```sh
poptop --why
```

```text
This box is CPU-bound.

  14 × cc1plus under `make -j16` (pid 8841) are using 71% of 14 cores,
  started 03:02:11, 4m29s ago. 87% of the machine's CPU is accounted for.

  Unusual: peak CPU for 03:00 on a weekday is 22% over the last 7 days.
  Not a known recurrence.

  Stopping them returns ~10 of 14 cores. `make` would remain and respawn.

Nothing else is constrained: memory 41%, disk 4%, no stalls.
```

And on a machine with nothing wrong, the sentence that makes the feature
trustworthy:

```text
Nothing is constrained. CPU 6%, memory 41%, disk 2%, no stalls, no waiting.
This is ordinary for 09:00 on a weekday.
```

A tool that always finds something wrong is a tool nobody believes twice.

## Why this is the keystone and not a gimmick

It is the only item in the roadmap that is **pure composition**: it collects
nothing, computes nothing new, and adds no state. Every clause is another item's
output, and the value is entirely in putting them in one order in one place.

That also makes it the test of whether the rest was built honestly. If the
attribution cannot state its remainder, the sentence cannot be written. If the
baseline cannot refuse, the sentence claims "unusual" on three days of data. If
the projection cannot decline, the sentence predicts a catastrophe from noise.
**`--why` is where every piece of hedging in this roadmap either holds or is
revealed as decoration.**

It is also the shape the tool gets used in: in a terminal, in a runbook, piped
into an incident channel, and — because it is one paragraph of plain text with
the evidence attached — as the thing somebody pastes to a colleague who is not
looking at the screen.

## What needs deciding

- **Which constraint wins** when several are live. `S` already names the
  constraint for sorting and the same ranking should serve here, or the two will
  disagree and one of them will be wrong.
- **Length.** One paragraph. The moment it grows sections it is `--report`, which
  already exists and is the right place for detail.
- **Whether it is live-only.** It should work at the cursor too — `--why` for a
  moment in a recorded day is the support workflow, and it is the same code.
- **How it fails.** With no log there is no baseline and no recurrence, so two
  clauses vanish. The sentence must degrade to what is known rather than omitting
  silently: "no history on this machine, so I cannot say whether this is normal".

## Acceptance criteria

- [ ] One command answers "what is wrong here" in a paragraph
- [ ] A healthy machine gets a sentence saying so, with the figures
- [ ] Every clause is traceable to a panel a reader can open and check
- [ ] Clauses that need history say so when there is none, rather than vanishing
- [ ] It works at a cursor position in a recorded day, not only live
- [ ] Nothing in it is computed here that is not computed for a panel as well
