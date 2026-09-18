---
id: 79
key: r2
title: Every unsafe line accounted for
type: milestone
status: backlog
labels:
- review
created: 2026-09-18
updated: 2026-09-18
priority: p2
---

Code review sprint. Read the code for what the tests cannot show: the reasoning behind each `unsafe` block, the moments between a check and the action it guards, the panics a user can reach, and the 3,400 lines of `main.rs` and `app.rs` that have no unit tests of their own. Every finding is either fixed with a test that fails before the fix, or filed as its own item with the reasoning.
