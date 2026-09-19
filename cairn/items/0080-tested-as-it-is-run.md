---
id: 80
key: r3
title: Tested as it is run
type: milestone
status: backlog
labels:
- testing
created: 2026-09-18
updated: 2026-09-18
priority: p2
---

Testing sprint. 593 unit tests exercise poptop from the inside. Nothing runs the binary the way a person or a script does. CI's live checks are `--once` and a closed pipe. This sprint tests from the outside: the command line, the export format against its own schema, the Linux backend against kernels other than the CI runner's, the TUI in a real pseudo-terminal, and signals sent to real processes.
