---
id: 23
title: RSS is wrong by an integer factor on any kernel without 4K pages
type: bug
status: done
milestone: v0.2
created: 2026-09-06
updated: 2026-09-06
priority: p0
effort: s
area: collect
---

## What happens

`ProcFs::new` hardcodes `page_size: 4096`, and every process's memory figure is
`rss_pages * page_size` (`collect/linux.rs`). `/proc/<pid>/stat` field 24 is a
count of pages, so on a kernel whose pages are not 4 KiB every RSS in the table
— and the memory bar derived from it — is out by that ratio.

- 16 KiB pages: every figure is **4x too small**. Asahi Linux and several ARM
  distributions ship this.
- 64 KiB pages: **16x too small**. It has been the RHEL aarch64 default.

Nothing about the output looks wrong. A process using 4 GB is reported as
using 256 MB, in a figure people size machines from.

## What should happen

Read the real page size once at startup.

`/proc/self/smaps` states it exactly, on its first mapping:

    KernelPageSize:        4 kB

Deriving it instead from `/proc/self/statm` pages against `/proc/self/status`
`VmRSS` does not work — the two files are read microseconds apart and RSS moves
in between, which gave 4084 rather than 4096 when measured. `smaps` is a larger
file, but this is read once at startup rather than per sample.

The same file answers it on any kernel, so no dependency is needed. That
matters: the point of this backend is to need nothing.

## Acceptance criteria

- [ ] Page size read from the kernel rather than assumed
- [ ] A kernel that will not say degrades to 4096 and says so, rather than silently
- [ ] Read once at startup, not per sample — measured with `--bench`
- [ ] A test asserts the parsed size against `/proc/self/smaps` on the live machine
