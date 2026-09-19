---
id: 101
title: sysinfo hands sysctlbyname a name with no NUL at its end
type: bug
status: backlog
milestone: later
labels:
- review
created: 2026-09-18
updated: 2026-09-18
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
