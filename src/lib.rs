//! The program's modules as a library, for the fuzzer and nothing else.
//!
//! poptop is a binary, and `cargo fuzz` can only link a library. Rather than
//! turn the program into a library with a thin `main` — which would make every
//! `pub` item public API, and silence the dead-code lint that has already
//! caught a field nothing read — the same files are compiled a second time
//! here, only when the `fuzzing` feature asks for them. Without it this crate
//! is empty and the program is exactly what it was.
//!
//! The modules are the program's own files, so `crate::` paths inside them
//! resolve here just as they do in `main.rs`. What they do not reach is
//! whatever `main.rs` defines itself, which only their tests do — so a test
//! build leaves the library empty. The tests run once, in the program.

// Library lints on the program's `pub` items, which are `pub` to each other
// rather than to the world: `len` without `is_empty` is advice for an API.
#![cfg_attr(
    feature = "fuzzing",
    allow(dead_code, unused_imports, clippy::len_without_is_empty)
)]

#[cfg(all(feature = "fuzzing", not(test)))]
pub mod app;
#[cfg(all(feature = "fuzzing", not(test)))]
pub mod check;
#[cfg(all(feature = "fuzzing", not(test)))]
pub mod collect;
#[cfg(all(feature = "fuzzing", not(test)))]
pub mod config;
#[cfg(all(feature = "fuzzing", not(test)))]
pub mod cvd;
#[cfg(all(feature = "fuzzing", not(test)))]
pub mod export;
#[cfg(all(feature = "fuzzing", not(test)))]
pub mod glyphs;
#[cfg(all(feature = "fuzzing", not(test)))]
pub mod history;
#[cfg(all(feature = "fuzzing", not(test)))]
pub mod keys;
#[cfg(all(feature = "fuzzing", not(test)))]
pub mod log;
#[cfg(all(feature = "fuzzing", not(test)))]
pub mod persist;
#[cfg(all(feature = "fuzzing", not(test)))]
pub mod query;
#[cfg(all(feature = "fuzzing", not(test)))]
pub mod report;
#[cfg(all(feature = "fuzzing", not(test)))]
pub mod sample;
#[cfg(all(feature = "fuzzing", not(test)))]
pub mod signal;
#[cfg(all(feature = "fuzzing", not(test)))]
pub mod store;
#[cfg(all(feature = "fuzzing", not(test)))]
pub mod theme;
#[cfg(all(feature = "fuzzing", not(test)))]
pub mod tree;
#[cfg(all(feature = "fuzzing", not(test)))]
pub mod ui;
