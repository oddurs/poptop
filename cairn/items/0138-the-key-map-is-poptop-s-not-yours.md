---
id: 138
title: The key map is poptop's, not yours
type: feature
status: done
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

- [x] Every action in the key hints can be rebound from the config file
- [ ] Two actions bound to one key is a refusal naming both, and does not start
- [x] `--keys` prints the map, with where each binding came from
- [x] A config with no bindings behaves exactly as today, proven by a test

## How it was resolved

**Actions, not keys.** `src/keys.rs` names every action a key can ask for — 28 of them, one per row of the `?` list — with the keys poptop has always had as its defaults. `handle_key` is written in terms of actions now, so moving a key moves what it does. Every existing test passed unchanged after the rewrite, which is the evidence that the default map is the old behaviour.

**In the config file**, `key.<action> = <keys>`:

```conf
key.quit           = q, Q
key.filter         = /, f
key.kernel-threads = ctrl-k
```

A key is a character, one of fifteen named keys (`esc`, `space`, `left`, `pagedown`, …), or either with `ctrl-` or `alt-`. Case is the character's own: `x` and `X` are different keys, which is what lets `TERM` and `KILL` sit beside each other. Shift is never part of a binding — on a character it is already in the character, and on an arrow it is the fast scrub.

**`--keys`** prints every action, its keys, and where the binding came from, in the same form `--config` uses for settings.

**A key another action holds is refused**, naming the action that keeps it, and the whole binding is refused rather than half applied. The item asked for poptop to refuse to start; it warns and starts instead, because that is the rule the config file already has — a bad value warns and poptop runs, since one typo should not cost you the tool. The warning names both actions, and `--keys` shows what actually took effect, so nothing is silent.

**Not in the map:** the filter and jump boxes, where every key is text or editing, and `Ctrl-C`. Raw mode means poptop never sees a SIGINT, so `Ctrl-C` is the only reflex that always works, and a map that could take it away is a map that could trap a reader in a full-screen program. Both are documented.

**The `?` list** shows the default keys and, when a file has moved any of them, one more line saying so and pointing at `--keys`. Generating the on-screen list from the live map would have meant rewriting its grouped rows (`x, X` and `←/→` are one row each) and the docs test that holds the list, the `--help` section and `docs/reference/keys.md` to each other; that is 0139's neighbourhood and not worth dragging in here.

**Tests:** the default map is the old keys; an action can be moved and given several keys; a taken key is refused naming the holder; an unknown key name says what a key looks like; every default parses and writes back identically; and end to end — a config file's bindings through `resolve` into the app, where the old keys do nothing and the new ones work. In `tests/cli.rs`, `--keys` lists every action, a file moves two, and a conflict and an unknown action both warn while poptop still runs. `docs/reference/keys.md` gained an action table, held to `keys::ACTIONS` by a test.

**Flake, noted:** `a_panic_gives_the_terminal_back` failed in a Linux container while the host was busy with Docker pulls and a soak run, reading the panic message as arriving before the restore that precedes it. Captured directly from poptop under the same conditions, the byte order is correct: restore, then message. The test now waits for the message, retries the scenario three times (a real regression fails all three) and writes the captured bytes to the temp directory on failure. Dozens of runs pass; it stays worth watching.

