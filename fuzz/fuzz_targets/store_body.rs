//! The store with a valid header already in place, so every input reaches the
//! schema block and the merge path — the parts 0059 found three ways to crash
//! by reading, and the parts a fuzzer starting from raw bytes spends most of
//! its time failing to get past the magic to reach.
#![no_main]

use libfuzzer_sys::fuzz_target;
use poptop::store::{MAGIC, VERSION};

fuzz_target!(|body: &[u8]| {
    let mut file = Vec::with_capacity(MAGIC.len() + 4 + body.len());
    file.extend_from_slice(MAGIC);
    file.extend_from_slice(&VERSION.to_le_bytes());
    file.extend_from_slice(body);
    let _ = poptop::store::decode_reporting(&file);
});
