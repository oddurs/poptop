---
id: 96
title: No number poptop shows has been checked against another tool at the same moment
type: chore
status: done
milestone: r4
labels:
- validation
created: 2026-09-18
updated: 2026-09-18
priority: p1
effort: l
area: collect
---

## Problem

The tests check parsing and arithmetic against expected values written by the same author, from the same understanding of the counters. 0023, 0024 and 0027 were all cases where poptop was consistent with itself and wrong about the machine. The only protection against the next one is a comparison with an independent reader.

## Proposal

Write a validation harness, a script rather than a unit test, that runs a known load (CPU burn, memory allocation, disk writes, network transfer) and samples poptop alongside `ps`, `vmstat`, `iostat`, `/usr/bin/time` and `atop` on Linux, and `ps`, `vm_stat`, `iostat` and `top -l` on macOS. It then reports every metric that falls outside a stated tolerance. Run it before each release and record the results.

## Acceptance criteria

- [x] A harness in the repo, runnable with one command on each platform
- [x] Tolerances stated per metric, with the reason for each
- [x] A first run on Linux and macOS, with every disagreement fixed or filed
- [x] The release checklist includes running it

## How it was resolved

**The harness**, `validate/run`, runs with one command on both platforms. It needs only the tools it compares against, plus perl for the loads, and nothing needs root. The known loads: two busy loops; 256 MiB of random bytes held by one process (random, because macOS compresses a page of one repeated byte to nothing); a direct-I/O disk writer on Linux; and about 64 MB/s of loopback traffic between two perl sockets. It takes poptop's line export over a 3-second window with `vmstat` or `iostat` and `top` running over the same window, and checks eleven or twelve metrics against independent readers: `free`, `sysctl`, `memory_pressure`, `getconf`, `ps`, `top`, `vmstat`, `iostat`, `/proc/net/dev`, `netstat` and the loads' own known sizes. Each row prints its tolerance and the reason for it (`validate/README.md` has the table). It exits with the number that disagree. The `validate` workflow runs it on GitHub's Ubuntu and macOS runners. atop was left out: none of the machines had it, and it reads the same counters as `vmstat` and `ps`.

**First runs, 2026-09-19**, on an M-series Mac, a Linux arm64 container, GitHub's Ubuntu x86_64 (6.17) and GitHub's macOS arm64. After the fixes below, every metric agrees on all four. The outputs are in `validate/results/`.

**Disagreements, and what each was:**

- **poptop: macOS reported no virtual size.** The README said sysinfo publishes none. poptop was already reading `proc_taskinfo` for thread counts, and `pti_virtual_size` is at offset 0 of the same buffer (checked against the SDK header). Fixed, with a test that holds our own process to `ps -o vsz`. **Filed as 0103:** the fault counts are in that buffer too (`pti_faults`, `pti_pageins`), and the same README note wrongly excludes them.
- **Harness: macOS available memory "disagreed by half"** against vm_stat's free + inactive + speculative + purgeable pages. That is a different definition. The kernel's own answer, `memory_pressure`'s free percentage, matched poptop to within a percent, because sysinfo uses XNU's documented available-memory formula. The reference is now `memory_pressure`.
- **Harness: whole-machine CPU on Linux** was compared against `100 − idle`, which counts iowait as busy. poptop reports iowait separately, and the disk writer produces a lot of it. The reference is now `us + sy`.
- **Harness: seven minutes lost on a runner.** `wc -c` read the disk writer's whole 92 GB file to count its bytes, and the other readings went stale while it did. It now uses `stat`, and the writer is capped at 4 GiB.
- **Harness: a busy loop is not 100% on a saturated machine.** On the 3-core macOS runner, two loops plus traffic left one loop 84% of a core, and `top` agreed with poptop. The "known 100%" row is held only when the machine had a core to spare, and says "skipped" otherwise.
- **Harness:** a same-byte string compressed to nothing, a fixed loopback transfer finished before the window opened, `nc` behaved differently on each platform, `iostat` printed disk columns first, and `kern.boottime` was misparsed. All fixed in the script.

**The release checklist**, `RELEASING.md`, now requires a clean run on both platforms (item 4).

