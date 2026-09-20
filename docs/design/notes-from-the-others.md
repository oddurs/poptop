# Notes from reading the others

Lessons taken from htop, btop, and bottom, with the measurements that backed
them:

- **Cache what doesn't change.** htop reads a process's cmdline and start time
  once per process *lifetime*, not once per sample, and gets the uid from a
  single `fstat` on the `/proc/<pid>` directory rather than parsing
  `/proc/<pid>/status`. poptop originally parsed that file for every process every
  second: at 400 processes that was 58% of total collection time. Switching to
  `fstat` took a sample from 3.19ms to 1.41ms.
- **Clamp CPU percentages.** htop does `MINIMUM(percent_cpu, activeCPUs * 100)`.
  Without it, a pid reused between two samples diffs the new process against the
  old one's counter and reports thousands of percent. poptop clamps the same
  way, but in the model rather than in a backend: the ceiling is a claim about
  what the hardware can deliver, not a fact about `/proc`, and it lived in the
  Linux collector alone for long enough that nothing said whether macOS did not
  need it or had merely forgotten it. It is now applied to every backend's
  output by `Collector::sample`, so a third backend gets it by existing.
- **Guard the sample interval.** htop carries the comment "period might be 0
  after system sleep" — a real bug someone hit on a laptop, worth knowing about
  before it happens to you.
- **Only collect what is displayed.** htop gates expensive reads behind
  `PROCESS_FLAG_*` bits derived from the visible columns, so turning off a
  column stops the syscalls behind it. poptop collects everything unconditionally;
  this is the right shape to adopt before adding per-process IO and network.
- **Spread expensive work across samples.** For costly `/proc/<pid>/maps`
  parsing htop re-checks each process on a randomised interval rather than doing
  every process on the same tick, which avoids a periodic stall.
- **Have a fallback glyph set.** btop keeps a `tty_mode` symbol table for
  terminals that cannot render braille. Any Unicode-dependent drawing needs a
  plain-ASCII path.

All three are implemented: braille rendering and timeline zoom
(`src/glyphs.rs`, `History::peak_slots`), the process tree (`src/tree.rs`), and
tiered collection (`collect::Needs`).

**Tiered collection.** Core figures — cpu, memory, rss, name, user, state,
threads — are never gated, because the timeline and the default table depend on
them and their history has to be complete. Per-process disk IO is gated on the
column being visible, and it is not cheap: at 400 processes a sample costs
1.55ms without it and 2.28ms with, since it adds a file read per process.

**Gated collection conflicts with rewindable history** in a way htop never has
to face — enable IO at t=300 and the first 300 samples have nothing to show when
you scrub back into them. poptop resolves this by making collection a *ratchet*:
showing the columns starts collection, hiding them does not stop it. Toggling
would otherwise punch holes wherever the column happened to be off, and one
clean boundary is far easier to reason about while scrubbing than several.

Three states are rendered, and **none of them is a zero** — a fabricated zero is
indistinguishable from a genuinely idle process:

| Shown | Meaning |
| --- | --- |
| `·` | history recorded before the column was switched on |
| `—` | unreadable, or the process is too new to have a second reading yet |
| `1.2M/s` | a real rate |

`/proc/<pid>/io` is mode `0400` and owned by the process owner, so reading other
users' processes needs `CAP_SYS_PTRACE`. Only genuinely unreadable processes
prompt for root; a process merely awaiting its second reading also shows a dash
but resolves on its own, and conflating the two produces advice that does not
help.

The tree is a view over `ppid`, which every retained sample already carries, so
the tree you see while scrubbed back is the real hierarchy from that moment.
Every process appears exactly once even when `ppid` is corrupt or cyclic, and a
process orphaned between samples becomes a root rather than disappearing.

**macOS caveat:** `sysinfo` reports no parent for processes the user does not
own, so about a third of pids arrive with `ppid = 0` and appear as roots. The
`/proc` backend has real parentage for everything.
