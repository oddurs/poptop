//! The restart store, from the first byte: magic, version, schema, strings,
//! samples. Anything may come back; nothing may panic, hang or allocate
//! without bound.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|bytes: &[u8]| {
    let _ = poptop::store::decode_reporting(bytes);
});
