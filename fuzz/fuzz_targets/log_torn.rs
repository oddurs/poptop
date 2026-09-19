//! A day of entries, some written whole and some cut short by a crash, in any
//! order — and the reader must find exactly the whole ones.
//!
//! The other targets ask only that nothing panics. This one asks for the right
//! answer, because the failure it guards against is silent: a torn entry whose
//! length swallows the entries appended after it loses the rest of the day and
//! raises no error at all.
//!
//! What is guaranteed is every whole entry, in order. A torn entry can come
//! back too, in one case: when the bytes written after it are themselves torn
//! and happen to fill exactly the length it claimed, so that the entry after
//! them begins where it said it would end. Nothing in the file tells the two apart without a
//! checksum, which the format does not have yet — so the assertion allows it,
//! and only for the entries that really were torn.
#![no_main]

use libfuzzer_sys::fuzz_target;
use poptop::sample::Sample;
use std::time::{Duration, UNIX_EPOCH};

fuzz_target!(|plan: &[u8]| {
    // Two bytes an entry: whether it is torn, and where. Its CPU figure is
    // its position, so what comes back says which entries survived.
    let mut day = Vec::new();
    let mut whole = Vec::new();
    let mut torn = Vec::new();
    for (i, spec) in plan.chunks_exact(2).take(8).enumerate() {
        let cpu = i as f32;
        let sample = Sample {
            at: UNIX_EPOCH + Duration::from_secs(1_800_000_000 + i as u64),
            cpu_total: cpu,
            ..Sample::unknown()
        };
        let block = poptop::store::encode(&[&sample]);
        let mut framed = (block.len() as u32).to_le_bytes().to_vec();
        framed.extend(block);
        if spec[0] & 1 == 0 {
            day.extend(&framed);
            whole.push(cpu);
        } else {
            // A cut anywhere from the first byte to the last-but-one: a cut at
            // zero is an entry never begun, and at the end one that finished.
            let cut = 1 + usize::from(spec[1]) * (framed.len() - 2) / 255;
            day.extend(&framed[..cut]);
            torn.push(cpu);
        }
    }
    let (back, _) = poptop::log::read_blocks(&day, "poptop-fuzz");
    let got: Vec<f32> = back.iter().map(|s| s.cpu_total).collect();
    let mut rest = got.iter().filter(|c| !torn.contains(c));
    assert!(
        whole.iter().all(|w| rest.next() == Some(w)) && rest.next().is_none(),
        "plan {plan:?}: wrote {whole:?} whole and got {got:?}"
    );
});
