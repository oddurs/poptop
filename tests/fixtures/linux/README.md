# Linux fixtures

The Linux collector reads `/proc` and `/sys` from whatever kernel runs it.
CI is one kernel. These directories are others: the files poptop read on each
machine, which the collector's tests read instead of the machine running
them (`collect::linux::fixtures`, through `collect::at`).

Each has `expected`, the facts the test checks, worked out from the files by
`record` with awk rather than by poptop. A fixture cannot agree with a wrong
parser by being generated from it.

## Recorded

Run on the machine, with `record`, and nothing changed afterwards.

| fixture | machine | what it covers |
| --- | --- | --- |
| `ubuntu-x86_64` | GitHub `ubuntu-latest`, 6.17, x86_64 | a whole host: PSI, a cgroup v2 tree, ~160 processes |
| `ubuntu-22.04-x86_64` | GitHub `ubuntu-22.04`, 6.8, x86_64 | the older LTS kernel CI still runs |
| `ubuntu-arm64` | GitHub `ubuntu-24.04-arm`, 6.17, arm64 | arm64 |
| `container-arm64` | Docker (OrbStack), 7.0, arm64 | a restricted `/proc`: a pid namespace of two processes, a namespaced cgroup, and a kernel built without PSI |

`uname` in each says exactly which kernel.

## Derived

For the machines nobody here has, a recorded fixture changed by `derive` in
exactly the ways such a kernel differs, and nothing else. `derived` in each
says from what and how. A recorded one would be better, and replaces it.

| fixture | from | what changes |
| --- | --- | --- |
| `kernel-3.10` | `ubuntu-22.04-x86_64` | no `/proc/pressure` (before 4.20), no `MemAvailable` (before 3.14; poptop estimates it as `free` did), no `smaps_rollup` (before 4.14), cgroup v1 |
| `cgroup-v1` | `ubuntu-x86_64` | the v1 hierarchy, and v1 lines in `/proc/<pid>/cgroup` |
| `arm64-64k-pages` | `ubuntu-arm64` | a 64 KiB page in the auxiliary vector, as a `CONFIG_ARM64_64K_PAGES` kernel reports it. 0023 was the 16 KiB case of this |
| `nfs-client` | `container-arm64` | one NFSv4 mount in `mounts`, `mountstats` and `rpc/nfs` |

## Recording another

On the machine, with strace installed:

```sh
cargo build --release
tests/fixtures/linux/record target/release/poptop tests/fixtures/linux/NAME
uname -a > tests/fixtures/linux/NAME/uname
```

Or push a branch named `record-fixtures` and download the artifacts of the
`record-fixtures` workflow, which records on each of GitHub's runners.

**Read what it recorded before committing it.** Process command lines and
`/etc/passwd` are the machine's own. Environments are never read.

`record --expected DIR` works the facts out again from files already there.
The test then holds the collector to them, and to one more rule: a subsystem
a fixture does not have is absent in the sample, never zero.
