---
id: 137
title: A setting that did not take effect is invisible
type: feature
status: done
milestone: v1.1
created: 2026-09-20
updated: 2026-09-20
priority: p2
effort: m
area: config
---

## Problem

`config::resolve` merges a built-in default, the file, `NO_COLOR` and the flags, and then nothing can show the result. A user who writes `interval = 500ms` and sees one-second samples has no way to learn whether the line was read, overridden by a flag, or rejected. A wrong key warns once, at startup, above a full-screen monitor that erases it.

There is also nothing to start from: the config file has to be written by hand against the README.

## Proposal

`--config` prints every setting, its value and where it came from: the built-in default, the file and its line, `NO_COLOR`, or the flag. `--write-config` writes a commented file of the current values to the config path, refusing to overwrite one that exists.

## Acceptance criteria

- [x] `--config` lists every setting with its value and origin, and exits 0
- [x] The origin names the file and line number where a file set it
- [x] `--write-config` writes a file poptop reads back to the same settings, and refuses to overwrite
- [x] Both are in `--help`, the README, and `tests/cli.rs`

## How it was resolved

**`--config`** prints every setting, its value and where that value came from: `the default`, the file and its line, `NO_COLOR`, or the flag as it was written. The origin is recorded where the value is applied — `apply_file` knows the line, the flag loop knows the flag — and kept on `Settings::origins`. A setting nothing set has no entry, and reads back as the default.

The four settings the pair check reverts (`warn`, `critical`, `interval`, `window`) say `the default, after the pair above was refused`, because after a revert the file's line is no longer where any of them comes from.

**`--write-config`** writes a commented file of the current settings to the config path, one comment per setting, and refuses to overwrite one that is already there: it is a starting point, not a reset. `color` is written as `auto` unless something asked for a tier — writing the tier of the terminal it happened to run in would pin it for every terminal that later read the file. A container without a terminal proved that one: the round-trip test failed there until `color` was set explicitly, because `auto` re-detects.

**How the table grew.** `KEYS` was a list of `(name, apply)` pairs. Each entry is now a `Setting` with `apply`, a `show` that writes the value back in a spelling `apply` takes, and a one-line `note` for the file. The round trip is the test that holds `show` and `apply` together: settings written and read back are equal, field by field, including `1500ms`, `30m`, `2G` and `62.5`.

`Settings` now implements `PartialEq` by hand, over values only. Where a value came from is not what it is, and a test comparing a file against flags that ask for the same thing is asking about the values.

**Tests:** origins from each of the four sources; the pair revert; the round trip; `auto` left to the terminal; and in `tests/cli.rs`, `--config` against a real file with a flag and `NO_COLOR`, and `--write-config` written, read back and refused a second time.

**Fixed on the way past:** `a_panic_gives_the_terminal_back` (0094) asserted that the *last* restore came before the panic message. Unwinding drops the terminal after the hook has printed, so on Linux there is a second restore after the message, and the test failed there while passing on macOS. It now asks for the first restore after the last time the screen was entered. The harness also waits for the pty output to settle rather than for a fixed 100ms, which was enough on an idle machine and not on a loaded one.

