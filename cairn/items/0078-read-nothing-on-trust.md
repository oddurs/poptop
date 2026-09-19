---
id: 78
key: r1
title: Read nothing on trust
type: milestone
status: done
labels:
- validation
created: 2026-09-18
updated: 2026-09-18
priority: p2
---

Validation sprint. Every byte poptop reads that it did not write this second — the store, the daily logs, /proc, netlink replies, config, themes, the query and jump prompts — is fed by a fuzzer or a property test, not only by the inputs a test author thought of. 0059 found three ways to hang or crash the reader by reasoning; this sprint is so the next three are found by a machine.
