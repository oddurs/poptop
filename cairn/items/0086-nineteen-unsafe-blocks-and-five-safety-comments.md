---
id: 86
title: Nineteen unsafe blocks and five SAFETY comments
type: chore
status: done
milestone: r2
labels:
- review
created: 2026-09-18
updated: 2026-09-18
priority: p1
effort: m
area: safety
---

## Problem

There are `unsafe` blocks in `signal.rs` (3), `log.rs` (3), `collect/taskstats.rs` (5), `collect/procinfo.rs` (7) and `collect/linux.rs` (1). The codebase has five `// SAFETY:` comments. The FFI declarations (`kill`, `localtime_r`, `mktime`, and the netlink and `proc_pidinfo` calls) are written out by hand rather than taken from `libc`, so a wrong signature or struct layout would compile and misbehave.

## Proposal

Review each block. Write a `SAFETY:` comment stating the invariant and why it holds, or remove the block. Check every hand-declared extern signature and `#[repr(C)]` struct against the platform headers for both targets, including the arm64 and x86_64 layouts. Turn on `clippy::undocumented_unsafe_blocks`.

## Acceptance criteria

- [x] Every `unsafe` block carries a `SAFETY:` comment that a reviewer has checked
- [x] Every hand-written extern and `repr(C)` struct is checked against headers, with a size and alignment `const` assertion where it can be written
- [x] Buffer lengths from netlink and `proc_pidinfo` replies are validated before any read that depends on them
- [x] `clippy::undocumented_unsafe_blocks` is denied in CI
- [x] The tests run under Miri for the modules Miri can execute, or the reason they cannot is written down

## How it was resolved

**Found and fixed.**

- **On an Intel Mac every filesystem figure was read from the wrong structure.** `getfsstat` was declared by hand under its bare name. On x86_64 that symbol is the pre-10.6 call, which fills the old `struct statfs` with 32-bit inode numbers, a different size with different offsets. The SDK header renames the call to `getfsstat$INODE64` for any C program, and a hand declaration has to do the same. The root check then failed and the Filesystems panel was empty. `the_live_filesystems_include_a_sized_root` already caught it, but only on x86_64, where nothing had ever run it. The declaration now carries the `link_name`, and CI and `./check` run the whole suite as x86_64 under Rosetta. Without the fix that test fails there, and with it all 621 pass.
- **The netlink control replies were read without their own lengths.** `family` sliced the first read from byte 20 and searched it for attributes without checking the message type. An `NLMSG_ERROR` there was searched as if it were a reply. `register` indexed a fixed 20 bytes of whatever arrived. Both now go through `messages`, which bounds each message by its own `nlmsg_len`, checked against what was read, before anything indexes into it. `family_in` and `registration_in` are split from the socket so they can be tested.
- **An errno of `i32::MIN` panicked the registration reader.** Negating it overflows. It was found by the new never-panics test on its first run, and is now `checked_neg`.
- **`recv`'s count is clamped to the buffer** before it is used to slice. Without `MSG_TRUNC` the kernel never returns more, but every slice depends on that count, so the bound is now enforced instead of assumed.
- **`Listener::open` was one `unsafe` block over thirty lines of safe code.** It is now one block per call, each with its own reason. `bind` and `setsockopt` take their lengths from the values they point at, not a literal `12` and `4`.

**Checked against the headers.** A C program printed `sizeof`, `offsetof` and every constant, built against the macOS SDK for arm64 and x86_64, and against glibc in `gcc` containers for linux/arm64 and linux/amd64. Everything poptop declares matches on all four: `struct tm` (56 bytes, `tm_gmtoff` 40, `tm_zone` 48, 64-bit `time_t`), `struct statfs` on Linux (120; type 0, bsize 8, blocks 16, bavail 32) and on macOS (2168; the seven offsets in `OFF`), `kinfo_proc` (648; start time 0, `tv_usec` a 4-byte int at 8, pid 40), `proc_taskinfo` (96; threads at 84), `sockaddr_nl` (12), `socklen_t` (4), and the netlink, taskstats, socket, `sysctl` and mount constants. The one mismatch was a symbol name, not a layout: `getfsstat`, above.

**Written down where it can be.** `Tm` has a `const` assertion on its size and alignment, so a 32-bit target fails to compile instead of letting `localtime_r` write past a stack local. Its `tm_gmtoff` and `tm_zone` now use `c_long` and `c_char`. The byte-offset readers assert that their offsets fall inside the record sizes they accept.

**SAFETY comments.** All twenty-two blocks have one (nineteen, plus three from splitting `open`): `signal.rs` (3), `log.rs` (3), `taskstats.rs` (8), `procinfo.rs` (7) and `linux.rs` (1). `clippy::undocumented_unsafe_blocks` is denied in `Cargo.toml`, so CI's clippy enforces it on both platforms. Removing one comment fails the build.

**Miri cannot run these tests, so AddressSanitizer does.** Every `unsafe` block in poptop is a call to a C function, and Miri has models for almost none of them: it stops the whole run at the first call to `kill` or `mktime` ("can't call foreign function"), and the rest of the crate has no `unsafe` for it to check. AddressSanitizer intercepts the same calls and checks the buffers passed to them. The full suite is clean under ASan on Linux (arm64, with leak detection) and on macOS, apart from 0101, which is in `sysinfo`. A privileged container ran the new `the_live_socket_hears_a_real_exit_or_says_why_not` under ASan through `socket`, `bind`, `setsockopt`, `send`, the family lookup and the registration. Docker Desktop's containers are not in the initial PID namespace, so the kernel answered `EINVAL`, and a real exit record has not been received under ASan. On a machine where poptop runs as root, the test also proves that a process which lived a moment is heard. CI runs the suite under ASan on Linux in a job of its own.

