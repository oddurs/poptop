---
id: 93
key: v4.0
title: It recognises
type: milestone
status: backlog
created: 2026-09-13
updated: 2026-09-13
priority: p1
due: 2028-10-01
---

Every monitor treats each moment as new. A machine is not new — it has a rhythm.
The backup runs at 03:17, the deploy at 14:00 on weekdays, the log rotation on
Sunday, and the same four services hold the same share of memory for months.

An operator learns that rhythm over a year and then leaves. poptop has it written
down in the log after a week and cannot see it.

The point of this milestone is that **"is this unusual" is a different question
from "is this high", and only the first one is worth waking up for.** netdata
answers it with a model per metric trained at the edge. poptop can answer it from
the retained samples themselves — which is cheaper, explainable, and arguable,
and an answer you can argue with is worth more at 3am than a score you cannot.

Nothing in here needs new collection. It is all reading what is already on disk.
