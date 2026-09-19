//! `/proc/self/mountstats` and the RPC counters. One line a mount, and the
//! mount point is whatever whoever mounted it chose. Linux only.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|bytes: &[u8]| {
    #[cfg(target_os = "linux")]
    {
        let text = String::from_utf8_lossy(bytes);
        let _ = poptop::collect::nfs::parse_mountstats(&text);
        let _ = poptop::collect::nfs::parse_rpc_nfs(&text);
        let _ = poptop::collect::nfs::parse_rpc_nfsd(&text);
    }
    #[cfg(not(target_os = "linux"))]
    let _ = bytes;
});
