---
id: 101
title: sysinfo hands sysctlbyname a name with no NUL at its end
type: bug
status: done
milestone: later
assignee: Oddur Sigurdsson
labels:
- review
created: 2026-09-18
updated: 2026-09-19
priority: p2
effort: s
area: darwin
---

## What happens

`sysinfo` 0.38.4, which the macOS backend uses, calls `sysctlbyname("hw.cpufrequency".as_ptr(), …)` in `get_cpu_frequency` (`src/unix/apple/cpu.rs:208`). A Rust string literal has no terminating NUL, so the C function reads past the end of the literal until it finds a zero byte somewhere in the binary's read-only data. It is undefined behaviour, reached on every `System::new_all`, which is every macOS startup of poptop. In practice the next bytes are usually another literal and the lookup fails harmlessly, so the fallback is taken. Nothing guarantees that.

Found by running the test suite under AddressSanitizer for 0086: `global-buffer-overflow … in sysctlbyname`, in `SysinfoCollector::new`. It is the only report ASan makes for the whole suite, on either platform. The latest release, 0.39.6, has the same line.

## What should happen

`c"hw.cpufrequency".as_ptr()`, as the same file already does for `c"pmgr"`.

## Reproduction

1. `RUSTFLAGS="-Zsanitizer=address" cargo +nightly test -Zbuild-std --target aarch64-apple-darwin --bin poptop`
2. The first test that builds a `SysinfoCollector` aborts with the report above.

Not poptop's code to fix. It needs a report or a one-line pull request upstream (GuillaumeGomez/sysinfo), and then a version bump here.

## How it was resolved

Fixed on poptop's side by never taking the path. sysinfo calls `get_cpu_frequency` only when asked to refresh the CPU frequency, and poptop never reads the frequency. It was asking only because `System::new_all` and `refresh_cpu_all` ask for everything. The collector now starts with `RefreshKind::everything()` minus the frequency, and refreshes with `refresh_cpu_usage`. Nothing else it reads changes.

Reproduced first: `RUSTFLAGS=-Zsanitizer=address cargo +nightly test -Zbuild-std --target aarch64-apple-darwin --bin poptop` aborted with `global-buffer-overflow … in sysctlbyname` from `SysinfoCollector::new` (`darwin.rs:92`). After the change, all 633 tests pass under ASan with no report.

Kept that way two ways. CI's `asan` job now runs on macOS as well as Linux; the only report ASan ever made was on the platform it didn't run on. `sysinfo_is_never_asked_for_the_cpu_frequency` fails if the request comes back.

The literal is still unterminated in sysinfo 0.38.4 and 0.39.6, and a one-line fix upstream (`c"hw.cpufrequency"`, as the same file already does for `c"pmgr"`) would help its other users. poptop no longer needs it.
