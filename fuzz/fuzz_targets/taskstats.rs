//! A datagram from the taskstats socket: netlink messages holding nested
//! attributes holding a `struct taskstats`, every length in it one the reader
//! indexes with. Linux only.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|bytes: &[u8]| {
    #[cfg(target_os = "linux")]
    {
        use poptop::collect::taskstats::{Boot, UserCache, exits_in};
        let boot = Boot {
            epoch_secs: 1_788_800_000,
            ticks_per_sec: 100.0,
        };
        let before = std::collections::HashMap::from([(1, u64::MAX)]);
        let _ = exits_in(bytes, 1.0, boot, &before, &mut UserCache::new());
    }
    #[cfg(not(target_os = "linux"))]
    let _ = bytes;
});
