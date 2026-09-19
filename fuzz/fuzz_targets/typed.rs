//! Everything a person types: the filter, the jump box, a colour, a config
//! file and a theme file. The filter is parsed on every keystroke, so a panic
//! in it is one keypress from leaving the terminal in raw mode.
#![no_main]

use libfuzzer_sys::fuzz_target;
use std::time::{Duration, UNIX_EPOCH};

fuzz_target!(|bytes: &[u8]| {
    let text = String::from_utf8_lossy(bytes);
    let _ = poptop::query::parse(&text);
    let _ = poptop::log::parse_when_noting(&text, UNIX_EPOCH + Duration::from_secs(1_800_000_000));
    // A colour that reads must write back as itself: that is what lets a
    // theme file round-trip.
    if let Some(c) = poptop::theme::parse_color(&text) {
        let again = poptop::theme::parse_color(&poptop::theme::write_color(c));
        assert_eq!(again, Some(c), "{text:?}");
    }
    let mut settings = poptop::config::Settings::detect();
    let mut warnings = Vec::new();
    poptop::config::apply_file(&mut settings, &text, "fuzz", &mut warnings);
    let _ = poptop::config::parse_theme(&text, "fuzz", &mut warnings);
});
