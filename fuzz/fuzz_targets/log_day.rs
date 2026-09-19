//! A day's log: length-framed store blocks, as a crash, a full disk or two
//! poptops writing at once may have left them.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|bytes: &[u8]| {
    let _ = poptop::log::read_blocks(bytes, "poptop-fuzz");
});
