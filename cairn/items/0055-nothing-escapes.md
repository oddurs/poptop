---
id: 55
key: v2.0
title: Nothing escapes
type: milestone
status: backlog
created: 2026-09-08
updated: 2026-09-08
priority: p2
due: 2027-04-01
---

The foundation for atop parity: a model that can carry two hundred metrics
without a rewrite per metric, a store that survives an upgrade instead of
discarding history, and the completeness guarantee that makes atop trustworthy
— every process that was alive during an interval appears in it, including the
ones that exited.

Ordered first on purpose. atop's institutional strength is not its field count;
it is that you can point it at yesterday and believe what it says. Everything
after this milestone is cheap once these three hold, and miserable if they do
not.
