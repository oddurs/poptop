---
id: 227
title: The table is a spreadsheet, and poptop's one advantage barely reaches it
type: feature
status: backlog
milestone: r8
labels:
- ui
created: 2026-09-20
updated: 2026-09-20
priority: p2
---

## Problem

The concept item the rest of this milestone keeps running into.

Every row gets the same thirteen columns, and which thirteen is chosen by the
tab. That is htop's model, and it is a good one for a tool whose data is the
present instant. poptop's data is not: it keeps every sample it has taken,
*including the whole process table*, and that is the entire claim on the front
of the README.

In the process table that claim shows up as one ten-cell sparkline, on a shared
scale that flattens it (0222), and it is the first column dropped when the
width ladder starts (`shape.spark` is step 7 of 9). The one thing no other
monitor can draw is the most easily discarded thing on the screen, and the rest
of the row is the same columns htop would have shown.

## Proposal

Not a redesign. A direction for the items above to point at, so they are not
thirteen separate width arguments:

**A row should spend its width on what is unusual about that row.** A process
that is swapping shows its fault rate; one stuck in `D` shows what on; one that
has grown 400 MB in a minute shows the slope. The columns every row shares
shrink to the ones every row is compared on — CPU, memory, identity — and the
rest of the width goes to the answer for *that* process.

The buffer already holds what this needs. `App::growth` is the shape of it: an
arithmetic over two samples that no column set had to be widened for.

The three questions worth settling before anything is built:

- what makes a row "unusual" without it flickering as the figure crosses a
  threshold — the same problem the sort's calm has, with the same answer
- whether the unusual column is *instead of* a shared one or as well as
- whether a reader can turn it off, and whether the tabs survive it at all

## Acceptance criteria

- [ ] A decision recorded in `docs/design/answering-questions.md`, with the
      alternatives that were rejected and why
- [ ] If it is built: a row on a quiet machine looks exactly as it does today
