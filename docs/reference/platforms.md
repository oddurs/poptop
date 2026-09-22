# Platforms

poptop reads what the kernel publishes, and says `—` for the rest rather
than a zero. What you get therefore depends on the platform, and on what
you are allowed to read.

Linux goes through `/proc` and `/sys` directly
(`src/collect/linux.rs`). macOS and other Unixes go through `sysinfo`, plus
`proc_pidinfo`, `getfsstat` and `kinfo_proc` for what sysinfo does not
publish (`src/collect/darwin.rs`).

## What each platform gives you

| | Linux | macOS |
|---|---|---|
| CPU total and per core | yes | yes |
| iowait, steal, guest, irq, softirq | yes | — |
| context switches, interrupts, forks | yes | — |
| Memory, swap | yes, with composition | total, used, available, swap |
| Paging, swapping, OOM kills | yes | — |
| Stall pressure (PSI) | kernel 4.20+ | — |
| Load average | yes | yes |
| Per-process CPU, memory, state, threads | yes | yes |
| Per-process disk IO | `CAP_SYS_PTRACE` for other users' | your own processes |
| Per-process faults | yes | yes, for your own processes |
| PSS (shared memory divided fairly) | yes, in the memory view | — |
| Command lines | yes | yes |
| Processes that lived and died between samples | `CAP_NET_ADMIN`, host namespaces | — |
| Kernel threads | yes, hidden until `K` | none exist |
| Process tree | yes | yes, but the parent of a process you cannot read is unknown |
| Containers (cgroup v2) | yes | — |
| Per-cgroup CPU and pressure (`C`) | cgroup v2 | — |
| NUMA nodes | yes | — |
| Disks: throughput, IOPS, service time, queue depth | yes | yes |
| Disk saturation (utilisation) | yes | — |
| Filesystem capacity | yes | yes |
| Network throughput | yes | yes |
| Network errors, drops, retransmits | yes | errors only |
| NFS client and server | yes | — |
| CPU throttling | `cpufreq` | — |
| Temperatures, hottest in each group | `hwmon` | the HID sensor services |
| Fan speeds | `hwmon` | — |
| Battery: charge, direction, draw | `power_supply` | the smart battery, in the IO registry |
| GPU load | amdgpu, in `drm` | the accelerator's statistics, in the IO registry |

## What you can read without root

**Linux.** Almost everything above except `/proc/<pid>/io` for other users'
processes, which needs `CAP_SYS_PTRACE`, and exited processes, which need
`CAP_NET_ADMIN`. If most of the table is unreadable poptop withdraws the IO
columns entirely rather than draw a wall of dashes, and says so in the
panel rule.

**macOS.** About a third of a typical Mac's process table belongs to other
users or to the system, and the kernel will not describe those processes to
you: their CPU, memory, threads and faults come back as `—`, their state as
`?`, and their parent as unknown, so they appear as roots in the tree.
Running as root shows them.

## Where a figure comes from

Every figure poptop draws has one source, and the one it uses is chosen so
the number means what the column says. The arguments — why `read_bytes` and
not `rchar`, why `MemAvailable` and not free, why the busiest disk and not
the sum — are in [docs/design/](../design/).
