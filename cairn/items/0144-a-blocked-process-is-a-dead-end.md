---
id: 144
title: A blocked process is a dead end
type: feature
status: backlog
milestone: v5.1
created: 2026-09-13
updated: 2026-09-13
priority: p1
area: collect
---

## Problem

poptop reports a process in `D` state — uninterruptible sleep — and colours it,
because there is no healthy amount of stuck-in-the-kernel. Then it stops.

`D` is the state a reader most wants to follow, and it is the one poptop says
least about. "Blocked" is not an answer; "blocked reading from
`/var/lib/pg/base/16384/2601` on an NFS mount whose server stopped answering" is
an answer, and every part of it is in `/proc`.

## What is actually reachable, without eBPF

- `/proc/<pid>/wchan` — the kernel symbol the task is sleeping in.
  `folio_wait_bit`, `rpc_wait_bit_killable`, `io_schedule`. atop reads this with
  `-W` and shows the raw symbol.
- `/proc/<pid>/syscall` — the syscall number and its arguments, including the fd
  for read and write.
- `/proc/<pid>/fd/<n>` — a symlink to the file, socket or pipe.
- `/proc/<pid>/stack` — the kernel stack, needs `CAP_SYS_ADMIN`.

Nobody joins them. atop shows the wchan symbol and leaves the reader to know
what `rpc_wait_bit_killable` means. The join — **symbol → syscall → fd → path**
— is the whole value, and it is three small reads of files poptop is already
walking.

## Why this is poptop's to do

It composes with everything that makes poptop different. A `D` state in the live
table is one thing; a `D` state in a sample from four minutes ago, with the file
it was blocked on, attached to the spike in the graph above it, is a different
tool. And it needs nothing installed.

## What needs deciding

- **Cost.** Three extra reads per blocked process, not per process. Blocked
  processes are usually few and always interesting, which is the right shape for
  the cost model — but it wants measuring on a box where a hundred tasks are
  stuck behind one hung mount, because that is the case it exists for.
- **Symbol to English.** `rpc_wait_bit_killable` means "waiting on an NFS
  server". A small table of the symbols that matter, with the raw symbol kept
  beside the translation — never replacing it, because the table will be
  incomplete and an unrecognised symbol must still be shown.
- **Retention.** Storing a path per blocked process per sample is cheap because
  there are few of them, but it is another `Option` in the record and needs the
  same absent-versus-none discipline.
- **Permission.** `wchan` and `syscall` are readable for your own processes and
  need `CAP_SYS_PTRACE` for others — the same boundary the IO columns already
  handle, and it should be handled the same way rather than a second time.

## Acceptance criteria

- [ ] A blocked process names what it is blocked in, not only that it is blocked
- [ ] Where the syscall carries an fd, the file or socket behind it is named
- [ ] A kernel symbol with no translation is shown raw rather than hidden
- [ ] The extra reads are only made for processes that are blocked
- [ ] Cost measured with a hundred tasks stuck on one mount
- [ ] Unreadable is `—`, and distinguishable from "not blocked"
