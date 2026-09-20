# Validation

The tests show poptop agrees with itself. `validate/run` checks that it agrees
with the machine. It starts loads whose size is known (two busy loops, 256 MiB
held by one process, a direct-I/O disk writer on Linux, loopback traffic), then
reads poptop's `--export=line` and the platform's own tools over the same
window. It prints every metric with its reference, its tolerance and the
reason for that tolerance, and exits with the number that disagree.

```sh
cargo build --release
validate/run                  # target/release/poptop by default
VALIDATE_TRACE=1 validate/run # every command, timestamped
```

Or run the `validate` workflow, which runs it on GitHub's Ubuntu and macOS
runners and keeps the output. It is step 4 of `RELEASING.md`.

`results/` holds each run worth keeping. The first, on 2026-09-19, agreed on
every metric on four machines: an M-series Mac, a Linux arm64 container,
GitHub's Ubuntu x86_64 and GitHub's macOS arm64.

What each row compares against:

| metric | Linux | macOS |
| --- | --- | --- |
| cores | `getconf` | `getconf` |
| memory total | `free` | `sysctl hw.memsize` |
| memory available | `free` | `memory_pressure`'s free percentage |
| swap used | `free` | `sysctl vm.swapusage` |
| uptime | `/proc/uptime` | `kern.boottime` |
| processes | `ps -A` | `ps -A` |
| a busy loop's CPU | 100%, when a core is spare; `top`, same window | the same |
| whole-machine CPU | `vmstat` user + system | `iostat` user + system |
| a 256 MiB process | `ps` rss; virtual ≥ 256 MiB | the same |
| direct writes | the file's growth | — |
| loopback received | `/proc/net/dev` | `netstat -ib` |
