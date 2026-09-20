# Changelog

Notable changes, in the shape [Keep a
Changelog](https://keepachangelog.com/en/1.1.0/) describes, and versioned by
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Two things are promised beyond the version number, because they outlive the
process that wrote them:

- **The store and the daily log.** A newer poptop reads an older one's files.
  A change that breaks that is a breaking change and is named here as one.
- **The export schema.** `--export` fields are added, not renamed or
  repurposed; `--schema` is the record. The rules are under **Stability** in
  [docs/design/recording-and-output.md](docs/design/recording-and-output.md),
  and every schema change gets a line here saying whether it is compatible.

## [Unreleased]

### Fixed

- **The install line in the README did not work.** `cargo install --git
  <repo>` searches the whole repository, finds `fuzz/` as a second package
  with binaries, and refuses to guess; the package name is required. Nothing
  tested it, because nothing here installed poptop the way a reader does.
  The release workflow now runs the documented line against the tag before
  publishing, so a broken one stops the release instead of shipping in it.

## [0.1.0] - 2026-09-19

The first release. Written from the work, not from a plan.

**Store format 15, export schema `0.1.0`.** These are the baseline every
later version promises to read; `tests/corpus` holds files written by each
version that changed what a sample holds, and a test reads all of them.

### The program

- **A timeline you can scrub.** Every sample poptop has taken, including the
  whole process table, is kept and addressable: `←`/`→` to move, `Space` to
  hold, `b` to jump to a moment, `+`/`-` to zoom. Sampling continues while
  you look. Landing where nothing was recorded says so rather than showing
  the nearest sample as though it were the one asked for.
- **The process table as the thing being scrubbed.** The table under the
  cursor is the real one from that moment. Sorting, filtering, grouping,
  trees and the per-process detail panel all answer about that moment.
- **A filter with a grammar.** `/` takes a word or a query —
  `cpu > 5 and user = root`.
- **Groups, trees and threads.** `g` folds by name, then user, then
  container; `t` nests by parent; `y` expands the selected process only.
- **Views.** `v` cycles CPU, memory (`PSS`, `VSZ`, `MAJF/s`, `GROW`) and
  disk. `C` shows cgroups with pressure; `K` shows kernel threads.
- **History that outlives the process.** `--store=on` keeps the buffer across
  restarts; `--log=on` writes a day a file, opened by date with `--read`,
  listed with `--days`, summarised with `--report`.
- **Output for scripts.** `--once`, `--export=json|line`, and `--schema`,
  which is generated from the same definitions the exporter uses.
- **Signals, behind an opt-in.** `--signals=on` lets `x` and `X` send TERM
  and KILL — never while scrubbing, and never to a pid whose start time no
  longer matches, checked again at the moment of sending and through a pidfd
  on Linux 5.3 and later.
- **Colour that survives its reader.** The default palette replaces green
  with cyan; `--check-theme` measures any theme against simulated protanopia,
  deuteranopia and tritanopia and reports the separations. `NO_COLOR` and a
  monochrome tier are honoured, and user themes are files.
- **Two backends.** `/proc`, `/sys`, netlink and cgroup v2 on Linux;
  `proc_pidinfo`, `sysctl` and `getfsstat` on macOS. What each platform can
  and cannot publish is written down in
  [docs/reference/platforms.md](docs/reference/platforms.md).

### The rule everything else follows

A figure poptop could not read is `—`. A figure it read and which is zero is
`·`. Neither is ever written as the other, and nothing on screen is a
fabricated zero. `N/M need root` is a reason a column is empty, not a
warning.

### Fixed

Found and fixed before this release. The ones worth naming, because each was
a wrong answer rather than a missing one:

- A process whose `comm` is not UTF-8 was invisible on Linux.
- One unusual mount path blanked every filesystem and NFS figure.
- A torn write lost the rest of a day's log; entries now carry a checksum
  older builds read straight past.
- A day file linked to `/dev/zero` was read until the machine ran out.
- Kernels older than 3.14 reported memory as 100% used.
- A malformed cpu list could ask for a 16 GB allocation.
- A clock that skipped or repeated an hour refused to land anywhere.
- Every process read `R` on macOS, whatever it was doing.
- `hw.cpufrequency` through sysinfo read past a string literal.
- A terminal that went away left poptop spinning at 100% of a core forever.
- A narrow terminal truncated the digits of numbers rather than dropping
  whole columns.
- The network header flapped between interfaces and could settle on
  loopback.

### Known at release

Open, filed, and disclosed here rather than found by you:

- **A process on its first sample reports `0.0%` CPU** rather than "not yet
  known" (0134). CPU is a delta and a first sample has nothing to subtract
  from; the disk columns say `—` in exactly that situation and CPU prints a
  number. During a fork storm the processes doing the work sort to the
  bottom. Visible in the screenshot in the README, which is how it was
  found. The fix is a type change reaching both backends, the sort, the
  filter and the export schema, so it waits for 0.2.
- **An empty process table gives no reason** (0124). Grouping by container
  on a machine with none, or a filter that matches nothing, draws a header
  and blank lines.
- **`Esc` quits out of a filtered table** instead of clearing the filter
  (0125). An applied filter is not on the back-out ladder. `q` and `Ctrl-C`
  quit from anywhere, as documented.

[Unreleased]: https://github.com/oddurs/poptop/compare/v0.1.0...master
[0.1.0]: https://github.com/oddurs/poptop/releases/tag/v0.1.0
