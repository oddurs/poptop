//! Every `/proc` parser, on the same bytes: `stat`, `meminfo`, `vmstat`, PSI,
//! `diskstats`, `mounts`, `net/dev`, `snmp`, `io`, the NUMA files, `auxv`,
//! `statfs` and `cmdline`. The kernel writes most of these; an unprivileged
//! user writes part of `stat` (`comm`), of `mounts` (the paths) and all of
//! `cmdline`. Linux only — elsewhere this target is empty.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|bytes: &[u8]| {
    #[cfg(target_os = "linux")]
    poptop::collect::fuzz_proc(bytes);
    #[cfg(not(target_os = "linux"))]
    let _ = bytes;
});
