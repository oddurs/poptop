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

### Added

- **Temperatures and fans.** The hottest sensor in each group — CPU, GPU,
  storage, memory, battery, board — from `hwmon` on Linux and the HID sensor
  services on macOS, where forty sensors labelled like `PMU tdie6` become
  three figures a person can read. `TEMP` in the header, beside `CPU` once it
  is within fifteen degrees of critical; a `TEMP` row on the timeline when
  there is room, drawn from 20°C rather than zero; `FAN` while a fan is
  turning. Recorded, exported as `temps` and `fans` in `celsius` and `rpm`.
  Schema change, compatible: two optional fields and two records (0231).
- **Disks on macOS.** Throughput, operations, mean service time and mean queue
  depth for each whole disk, from the storage drivers' own counters in the IO
  registry. In the header as what the disk is moving each way, and on the
  timeline as a byte rate for one named disk. macOS does not count
  utilisation, so `DiskStat.util` — and `queue`, for symmetry — are optional
  now. Schema change, compatible: a field that becomes optional reads every
  older recording's value as present, which the store now does by rule rather
  than skipping it (0232).

### Fixed

- **Keys waited for the sample.** Collection ran on the thread that reads
  keys, so for as long as a sample took the screen froze and keys queued. It
  runs on a thread of its own now, with the log writes, and the interface draws
  a finished sample within a few milliseconds of it being ready (0229).
- **Samples fell wherever poptop happened to start.** They land on wall-clock
  multiples of the interval now — at one second, on the second — so two
  poptops, a feed and the monitor beside it, and one day and the next all
  sample the same instants. A clock that steps or slews costs one interval
  slightly short or long, never a stall or a burst. The feed kept its own
  schedule and restarted it from each wake-up, which drifted; it shares this
  one (0230).

## [0.2.0] - 2026-09-20

Activity Monitor's shape, in a terminal — over a timeline Activity Monitor
does not have.

**Store format 15, unchanged.** A 0.1.0 store and a 0.1.0 day log are read by
this version. Nothing this release adds is recorded; it is all screen.

**Export schema: compatible.** `--schema` reports the package version, so it
now reads `0.2.0`, and that one line is the whole diff against the 0.1.0
golden — every record, field, type and unit is the same. A parser written
against 0.1.0 reads this without a change, unless it pinned the version
string itself.

### The screen

- **A menu bar.** `F10`, or `Alt` and a title's underlined letter: File, Edit,
  View, Go, Process, Help. poptop has around thirty single-key bindings, which
  is fine once you know them and impenetrable before, and the footer fits six.
  Every item names the key that also runs it, and a test walks every one and
  checks that hint against what the key actually does — so the menu cannot
  drift into advertising a key that has come to mean something else.
- **Tabs, on the table they describe.** CPU, memory and disk are named sets of
  columns with the sort that belongs to each, on a strip directly above the
  table: `Tab`, `1`–`3`, `v`, or a click. The disk figures used to vanish on a
  narrow terminal with nothing on screen to bring them back; the disk tab is
  what brings them back, and `i` is gone along with the question it answered.
- **A scope line that cannot disappear.** What the list is and what has
  narrowed it — `nginx · 3 of 748 · root`. This was a clause in the panel
  title, which is a ladder that drops clauses from the least important end, so
  on exactly the terminals where the table is hardest to read the sentence
  saying *which* processes these were went first. A table that does not say it
  has been filtered lies about the machine, and does it silently.
- **The filter is a field, not a mode.** `/` types into the scope line, where
  the result of it is stated, rather than into a box at the other end of the
  screen. `Esc` puts back what was there before.
- **The graphs moved under the table.** The table is the present and the
  timeline is the past, so the screen reads down: this machine, this table, how
  it got here. The timeline's caption — how much time is on screen, which
  sample the cursor is on — now lands beside the keys that scrub it.
- **An inspector.** `⏎` on a process: what it is, what it has been doing, and
  peaks taken from the retained buffer rather than from this instant.
- **An action bar on the selection**, which states a refusal before the attempt
  rather than after it.
- **A summary strip** over the rows on screen, saying what *they* add up to —
  which is not what the machine adds up to, once anything has narrowed them.
- **The sorted column wears a caret in its own header**, and a click on a
  header sorts by it. The sort was named in the panel title and shown nowhere
  near the sorting.
- **Smoothing.** `--smooth=5s`, or the View menu. Two problems with two
  answers, because they are not the same problem. The *figure* is a weighted
  average ending at the moment on screen — the newest sample about a third of
  it, the oldest about a tenth — so a spike moves the number on the second it
  starts and fades over the seconds after, agreeing with the graph under it.
  The *order* is the same average taken on a beat, so between beats the table
  cannot change its mind about what goes above what. A row whose number
  twitches is mildly annoying; a row that swaps places with its neighbour while
  you are reading it is what makes a table unreadable, and only the second one
  is worth holding still.
- **Density.** `--density=compact|comfortable|spacious` — the margin either
  side of the content, the air in the header, and whether the table and the
  graphs are separated by a blank row. Comfort is still the first thing a small
  terminal surrenders: the margin goes where the command column needs it, and
  below that width the blank row is what tells the three apart.
- **The mouse.** Click a tab, a column header or a row; drag the timeline to
  scrub; the wheel moves time over the graph and the selection over the table.
  One layout serves both the drawing and the hit-testing, so a click cannot
  land on something other than what is under the pointer. `--mouse=off` gives
  the terminal its own selection back.
- **Line graphs, and an axis that can fit its data.** `--graph=line` traces the
  samples instead of filling under them; `--scale=fit` crops the axis to the
  data, which shows small movement and costs the comparison between one graph
  and the next.
- **Surfaces.** The interface paints its own grounds, in four measured layers,
  built from the terminal's own background where the terminal will say what
  that is. `--surface=off` leaves every ground alone.

### Settings

- **A config file, and `--config` to see it.** Every setting poptop has, what
  it is set to, and which line of which file set it — so a value that is not
  what you expected says where it came from rather than leaving you to guess
  between a flag, a file and a default. `--write-config` writes the current
  state back out as a file you can edit, with a line of prose per setting.
- **Every key is an action, and a config file can move it.** `key.quit = q, Q`.
  The keys poptop has always had are the defaults; the boxes are not bindable,
  because a map that could rebind Backspace inside the filter is one that could
  stop you typing. Bindings carry what the action carries, so `tab-memory` is a
  key for the memory tab and `page-up` is "select ten up" rather than a special
  case in the handler.
- **Start as you left off.** `view`, `sort`, `zoom`, `tree`, `group`,
  `kernel-threads` — each taking the values its key cycles through. Two
  combinations that cannot both hold (a tree with grouping, a sort the tab
  cannot show) warn and fall back rather than refusing to start.
- **`hide-columns`** drops columns before the width ladder runs, so the room
  goes to the command rather than to whatever the ladder would have dropped.
- **A theme you can edit while it runs.** `R` reads the theme file again, for
  trying a colour without restarting, and three more tokens — the selection's
  foreground, panel borders, and the seam where sampling stopped.

### Fixed

- **A narrow table dropped digits instead of columns.** Every column is a fixed
  width, and ratatui squeezes a set that does not fit rather than dropping any
  — so a right-aligned figure lost its *leading* digits and `100.9` rendered as
  `.9`. A wrong number is the one thing this table must never show. The ladder
  now drops columns, `RSS`, the state letter and the pid included, until what
  is left fits (0105).
- **The tree opened on a page of question marks on macOS**, where `launchd` is
  a root the kernel will not describe and is the parent of everything. A branch
  is ordered by what the whole branch adds up to now, so the root holding the
  work comes first (0107).
- **The header followed a different network interface every second**, and
  counted loopback. It follows one real interface chosen over a window, and
  labels which way the bytes are going (0108).
- **The history column was the same flat picture in every row** on a quiet
  machine, spending ten columns to repeat the `CPU%` beside it while the
  command was elided for want of room. It is drawn when some row's history
  moves, and says `history flat` when that is why it is missing (0110).
- **The selected row was a dark background and bold**, faint on most themes
  among rows that reorder every second. It has a ground of its own, which
  reverses where there is no colour to paint with (0114).
- **A poptop whose terminal went away span at 100% forever** (0115).
- **The sparkline and the timeline were drawn over different spans**, side by
  side, with nothing saying so. One clock for both pictures.
- **The graphs did not scroll together.** A braille cell was one bar at the
  peak of its two samples, and a sample met a new neighbour every second, so
  every cell was worked out again on every push: memory, which barely moves,
  scrolled cleanly while CPU and the network seemed to lag and catch up. Each
  dot column is now its own sample. Zoomed out, slots are cut on fixed
  positions in the recording rather than counted back from the newest sample,
  so the newest column fills and the rest only ever scroll.
- **`sysinfo` was raised past the stated compiler floor** by a dependency bump
  that edited the root manifest while updating only the fuzz lockfile. Held at
  the version `rust-version = "1.88"` can build, and `./check` now builds
  `--locked` first, which is the shape of that bug.

### Known at release

Open, filed, and disclosed here rather than found by you:

- **A process on its first sample reports `0.0%` CPU** rather than "not yet
  known" (0134). CPU is a delta and a first sample has nothing to subtract
  from; the disk columns say `—` in exactly that situation and CPU prints a
  number. During a fork storm the processes doing the work sort to the bottom.
  The fix is a type change reaching both backends, the sort, the filter and the
  export schema, so it waits.
- **An empty process table gives no reason** (0124). Grouping by container on a
  machine with none, or a filter that matches nothing, draws a header and blank
  lines.
- **`Esc` quits out of a filtered table** instead of clearing the filter
  (0125). An applied filter is not on the back-out ladder, though `Esc` does
  undo one while it is being typed. `q` and `Ctrl-C` quit from anywhere, as
  documented.
- **The actions group bump (#140) is unmerged**, needing a token scope this
  release was not cut with.

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

[Unreleased]: https://github.com/oddurs/poptop/compare/v0.2.0...master
[0.2.0]: https://github.com/oddurs/poptop/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/oddurs/poptop/releases/tag/v0.1.0
