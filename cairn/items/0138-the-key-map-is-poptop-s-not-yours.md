---
id: 138
title: The key map is poptop's, not yours
type: feature
status: backlog
milestone: v1.1
created: 2026-09-20
updated: 2026-09-20
priority: p2
effort: l
area: ui
---

## Problem

Every key is hard-coded in `handle_key`. A reader who wants `k` to mean kill rather than up, or who does not want `Esc` to quit at all (0102 made it back out first, which was as far as a fix could go without a key map), has no way to say so. Terminals differ too: a keyboard where `/` needs a modifier makes filtering awkward, and nothing can be moved.

## Proposal

A `[keys]` section in the config file naming an action per binding: `quit = q`, `filter = /`, `signal-term = x`. Several keys may share an action. Unknown action names warn like any other bad line, and a binding that would shadow another is refused with both named. `--keys` prints the resolved map, and the in-app hints and `--help` read from it rather than from a literal.

## Acceptance criteria

- [ ] Every action in the key hints can be rebound from the config file
- [ ] Two actions bound to one key is a refusal naming both, and does not start
- [ ] `--keys` prints the map, and the footer hints follow it
- [ ] A config with no `[keys]` behaves exactly as today, proven by a test
