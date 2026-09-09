//! poptop — a system monitor you can rewind.
//!
//! Copyright (C) 2026  poptop contributors
//!
//! This program is free software: you can redistribute it and/or modify it
//! under the terms of the GNU General Public License as published by the Free
//! Software Foundation, either version 3 of the License, or (at your option)
//! any later version. See the LICENSE file for details.

mod app;
mod check;
mod collect;
mod config;
mod cvd;
mod export;
mod glyphs;
mod history;
mod log;
mod persist;
mod query;
mod report;
mod sample;
mod store;
mod theme;
mod tree;
mod ui;

#[cfg(test)]
mod ui_tests;

use app::App;
use collect::{Collector, Needs, Platform, Source};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use std::io;
use std::time::{Duration, Instant};

const USAGE: &str = "\
poptop — a system monitor you can rewind

USAGE:
    poptop            interactive mode
    poptop --once     print one plain-text sample and exit
    poptop --read DATE
                    open a recorded day (YYYY-MM-DD) instead of live
    poptop --days     list the recorded days and their sizes
    poptop --export=json|line [DATE]
                    every metric, by name, for a script. With a date, the whole
                    of that recorded day rather than the machine now.
    poptop --schema   what --export reports: every record, field, type and unit
    poptop --report [DATE]
                    summarise a recorded day: peak and sustained, and what was
                    responsible for each. Today unless a date is given.
    poptop --bench    time 20 collection passes (development)
    poptop --check-theme NAME
                    measure a theme and say whether it is legible

    --glyphs=SET    timeline drawing: braille (default), block, or ascii.
                    Falls back to ascii automatically on a Linux console.
    --color=TIER    auto (default), mono, 16, 256, or true. Honours NO_COLOR.
    --interval=SPAN time between samples: 500ms, 2s, 10m (default 1s)
    --window=SPAN   history retained, as time not samples (default 10m)
    --store=on|off  keep history across restarts (default off). Written on a
                    clean exit to $XDG_STATE_HOME/poptop/history and read at
                    startup. poptop needs nothing running beforehand either way.
    --log=on|off    write a daily log that outlives the process (default off).
                    One file a day in $XDG_STATE_HOME/poptop/log, opened with
                    --read. poptop logs if it is left running and works if it
                    was not: nothing it draws depends on the log existing.
    --log-interval=SPAN
                    how often a sample reaches the log (default 10m). Not the
                    sample interval — the buffer stays at --interval.
    --log-days=N    days of log kept (default 7)
    --log-bytes=SIZE
                    bytes of log kept across every day (default 512M). The
                    bound that holds: a sample carries a whole process table.
    --warn=PCT      where 'getting busy' begins (default 50)
    --critical=PCT  where 'in trouble' begins (default 80). Must exceed --warn.
    --theme=NAME    a built-in (safe, classic, auto) or a file in
                    ~/.config/poptop/themes/NAME.theme. 'safe' replaces green with
                    cyan: green/yellow separates by only dE 3.7 under simulated
                    protanopia, against a target of 8, and red-green deficiency
                    affects roughly 8% of men. 'classic' restores green/yellow/red.

CONFIG:
    ~/.config/poptop/poptop.conf, honouring $XDG_CONFIG_HOME. Every setting above
    is a `key = value` line without the leading dashes:

        theme    = classic
        glyphs   = block    # comments run to the end of the line
        color    = 256
        warn     = 65       # a build box is busy at 50% and fine
        critical = 90
        interval = 500ms    # every sample keeps a whole process table,
        window   = 30m      # so these two together decide the memory
        store    = off      # keep history across restarts

    Lowest precedence first: built-in default, config file, NO_COLOR, flag —
    so a wrapper script can override a user's file without editing it.

    An unknown key warns, naming the key and the line, and poptop starts anyway.
    One typo should not cost you the tool.

THEMES:
    ~/.config/poptop/themes/NAME.theme, one line per colour. Every line is
    optional — a theme inherits `safe` for anything it does not name:

        ok         = #8fbcbb    # hex,
        series_cpu = 67         # a 256-colour index,
        chrome     = darkgray   # or an ANSI name

    Tokens: ok, warn, critical, series_cpu, series_mem, chrome, text,
    text_dim, selection_bg, live. The built-ins ship as files too, so the way
    to learn the format is to copy one.

    `poptop --check-theme NAME` measures one: the separation between every pair
    of meaning-bearing hues under simulated colour vision deficiency, and the
    contrast of everything drawn against the backgrounds it sits on. Only a
    PASS exits zero — a colour written as an ANSI name is a slot your terminal
    defines, so it cannot be measured, and that is INCOMPLETE rather than
    success. A failing theme still loads, with one line saying why: it is your
    terminal and your choice.

OPTIONS:
    -h, --help      show this help
    -V, --version   show version

HEADER:
    CLK             how much of the processor's nominal clock the kernel is
                    currently allowing. Shown only when it is below nominal,
                    because a machine at full speed has nothing to say — and
                    because a figure present on every frame is one nobody reads.

                    Not a temperature. A reading of `84°C` makes you infer, and
                    on hardware whose nominal is 85°C it makes you infer
                    wrongly; the machine knows whether it is allowed to run at
                    full speed and says so. `CPU 100%` beside `CLK 62%` is a
                    processor flat out and getting two thirds of the work done,
                    which nothing else on the header can distinguish from a
                    healthy busy machine — STALL, WAIT and disk saturation all
                    read normal, because nothing is waiting.

                    The policy ceiling, not the current frequency: an idle core
                    clocks down and that is a healthy machine doing nothing.
                    Catches whatever the driver reports by lowering its policy
                    maximum — thermal, power, or a limit set by hand — and not
                    hardware capping that reports through counters instead.

KEYS:
    q               quit
    Left/Right      scrub through history (Shift for 10 at a time)
    b               jump to a moment, as atop's -b does. Takes a distance or
                    a time: `-2h`,
                    `03:00`, `2026-09-08 03:00`. Local time, and a relative
                    jump is measured from the end of what is retained — in a
                    day opened with --read that is not today.

                    Landing where nothing was recorded says so, with how far
                    away the nearest sample is, rather than showing it as
                    though it were the moment asked for.
    + / -           zoom the timeline in and out
    Space           pause on the current sample, or resume live
    Home/End        jump to oldest / live
    Up/Down         select a process
    s               cycle sort column
    S               sort by whichever resource is stopping work, when the panel
                    names one. Never applied on its own — a table that reorders
                    itself under the reader is worse than one that does not.
    t               toggle the process tree
    d               show the selected process's own history in place of the
                    machine's: CPU, memory, threads and disk over the window on
                    screen, at full width, with the moments it was not running
                    marked rather than interpolated. `+`/`-` widen that window
                    the same way they do for the machine, and the cursor is the
                    same one, so scrubbing moves both.

                    Select a process with the arrow keys first — with nothing
                    selected there is no history to show, and the panel says so.
    g               fold processes sharing a name into one row, with the count
                    in the PID column. CPU, memory and threads are summed;
                    state, user, history and the command line are not — a group
                    has no single one of those, and says so with an em dash.
                    The summed memory is an upper bound: forked workers share
                    an interpreter heap copy-on-write and it is counted once
                    per member. The PSS column in the memory view (v) is the
                    measurement, and sums correctly. Not available with the
                    tree. Press again to fold by user, then by container.
    K               show kernel threads. Hidden by default on Linux: kworker,
                    ksoftirqd, irq and the rest outnumber the real processes
                    several times over on a many-core box, and none of them is
                    what anyone opened a monitor to find. The number hidden is
                    in the panel title. Does nothing on macOS, which has none.
    i               show or hide the per-process disk IO columns. Shown by
                    default where they can be read: `/proc/<pid>/io` needs
                    CAP_SYS_PTRACE for other users' processes, so on a box
                    running its services as root they would be a wall of
                    dashes, and poptop withdraws them after one sample.
    /               filter. A bare word is a substring match on the name, the
                    command line, the user or the pid, as before. It is also a
                    small query language:

                        state = D              stuck in uninterruptible sleep
                        write > 1mb            causing the disk saturation
                        threads > 100          leaking threads
                        cpu > 5 and user = root

                    Fields: cpu, mem (rss), threads (thr), state, pid, read,
                    write, user, name (command). Operators: > >= < <= = !=,
                    joined by `and`. Sizes take k/m/g/t and are binary, like
                    the column: 1mb is 1048576.

                    A figure the platform could not read matches nothing — not
                    `> 0`, and not `< 1mb` either. A malformed query filters
                    nothing away and says what is wrong.

                    Evaluated at the cursor while scrubbing, so it answers what
                    was in D-state at the moment of the spike.

ON MACOS:
    Some figures are Linux-only and simply do not appear:

    clock ceiling      macOS publishes none reachable without shelling out,
                       and `pmset -g therm` reports nothing at all on Apple
                       Silicon. CLK is absent here rather than reading 100%,
                       which would claim the machine is at full speed on the
                       strength of not being able to look.

    state = D          macOS reports no uninterruptible-sleep state, so that
                       query finds nothing here even on a machine stuck on IO.
                       The states it does report are R, S, I, T and Z.

    per-device disk    reading them costs 12ms a sample against a whole sample
                       of about four, so poptop does not read them rather than
                       pay it or show a stale number
    stall pressure     /proc/pressure has no equivalent here
    network drops      sysinfo counts errors without separating drops, and
                       exposes no TCP retransmit counters

    Everything else is read the same way on both.
";

/// Print a line, stopping the program quietly if the reader has gone away.
///
/// Rust ignores SIGPIPE and turns the resulting write error into a panic, so
/// `poptop --once | head -1` died with a backtrace. `--once` exists to be
/// scriptable, and `| head`, `| grep -m1` and `| less` are how a scriptable
/// thing gets used — a monitor that panics when you page its output is not one.
///
/// Handled in the writer rather than by restoring the signal disposition,
/// which would need `libc` for two lines of behaviour.
macro_rules! outln {
    ($($arg:tt)*) => {{
        use std::io::Write as _;
        if writeln!(std::io::stdout(), $($arg)*).is_err() {
            return Ok(());
        }
    }};
}

/// The same, without the newline, for output that carries its own.
///
/// `--export` and `--schema` used bare `print!` and so panicked on a broken
/// pipe — `| head`, `| grep -m1`, `| jq … | head` — which is precisely what
/// `outln!` exists to prevent, on the formats most likely to be piped.
macro_rules! out {
    ($($arg:tt)*) => {{
        use std::io::Write as _;
        if write!(std::io::stdout(), $($arg)*).is_err() {
            return Ok(());
        }
    }};
}

/// Print what could not be used.
///
/// Held rather than printed where it was found, because config is read before
/// the alternate screen opens and the screen erases everything written before
/// it. A warning nobody can see is not a warning, so the interactive path
/// waits until the terminal is its own again.
fn flush(warnings: &[config::Warning]) {
    for w in warnings {
        eprintln!("poptop: {w}");
    }
}

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut collector = Platform::new()?;

    let mut warnings = Vec::new();
    // Whatever the backend had to assume about this machine. Said once, with
    // the config warnings, rather than folded into every figure that rests on
    // it — an assumption nobody is told about is the same shape as a wrong
    // number.
    warnings.extend(collector.take_notes().into_iter().map(config::Warning));
    let file = config::read(&mut warnings);
    let no_color = std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty());
    let (settings, positional, file_warnings) = config::resolve(
        config::Settings::detect(),
        config::Sources {
            file: file.as_ref().map(|(o, t)| (o.as_str(), t.as_str())),
            no_color,
            themes: &config::read_theme,
        },
        &args,
    )
    .unwrap_or_else(|bad| {
        // Before exiting, not after: a config file that could not be *read* is
        // reported here too, and dropping that warning because a flag was also
        // wrong would send the user off to fix the flag and rerun into the
        // same silently-ignored config.
        flush(&warnings);
        eprintln!("poptop: {}", bad.as_flag());
        std::process::exit(2);
    });
    warnings.extend(file_warnings);

    let args = positional;

    // Built here rather than beside the App, so that a problem with the user's
    // theme is reported on every path — a colour scheme nobody can read is a
    // fact about their config, and `--once` and `--version` report every other
    // config problem too.
    let (theme, skipped) = theme::Theme::new(settings.palette, settings.tier)
        .with_thresholds(settings.warn, settings.critical)
        .with_overrides(&settings.overrides);

    match args.first().map(String::as_str) {
        // A report is not interactive, so it is a subcommand rather than a
        // mode: `poptop --report` from cron is how atop is used
        // non-interactively, and a report that needed a terminal could not be.
        Some(a) if a == "--report" || a.starts_with("--report=") => {
            let inline = a.strip_prefix("--report=").filter(|s| !s.is_empty());
            let text = inline.or_else(|| args.get(1).map(String::as_str));
            flush(&warnings);
            let Some(dir) = log::dir() else {
                eprintln!("poptop: no state directory — set HOME or XDG_STATE_HOME");
                std::process::exit(2);
            };
            // Today unless a day is named. The cron case is a nightly summary
            // of the day that has just happened, and making it spell the date
            // out would make it a date-arithmetic problem in a crontab.
            let date = match text {
                Some(t) => match log::Date::parse(t) {
                    Some(d) => d,
                    None => {
                        eprintln!("poptop: `{t}` is not a date. Write it as YYYY-MM-DD");
                        std::process::exit(2);
                    }
                },
                None => match log::date_of(std::time::SystemTime::now()) {
                    Some(d) => d,
                    None => {
                        eprintln!("poptop: this machine's clock is before the epoch");
                        std::process::exit(2);
                    }
                },
            };
            let (samples, said) = match log::open_day(&dir, date) {
                Ok(pair) => pair,
                Err(why) => {
                    eprintln!("poptop: {why}");
                    std::process::exit(1);
                }
            };
            for note in said {
                eprintln!("poptop: {note}");
            }
            outln!("poptop report for {date}");
            out!(
                "{}",
                report::render(
                    &samples,
                    settings.warn,
                    REPORT_WINDOW,
                    settings.log_interval
                )
            );
            return Ok(());
        }
        Some("--schema") => {
            flush(&warnings);
            out!("{}", export::schema_json());
            return Ok(());
        }
        // Both spellings. `--export=json` is what the help shows and what
        // every other flag here looks like; `--export json` is what a hand
        // reaches for. Neither is worth an error message.
        Some(a) if a == "--export" || a.starts_with("--export=") => {
            let inline = a.strip_prefix("--export=").filter(|s| !s.is_empty());
            let how = inline.or_else(|| args.get(1).map(String::as_str));
            let Some(how @ ("json" | "line")) = how else {
                flush(&warnings);
                eprintln!("poptop: --export takes `json` or `line`");
                std::process::exit(2);
            };
            // A day, if one was named; otherwise the machine now. Reading
            // history is not a separate feature — it is the same output over a
            // different buffer, which is the whole reason the store carries a
            // schema.
            let day = args.get(if inline.is_some() { 1 } else { 2 });
            flush(&warnings);
            let samples = match day {
                Some(text) => {
                    let Some(date) = log::Date::parse(text) else {
                        eprintln!("poptop: `{text}` is not a date. Write it as YYYY-MM-DD");
                        std::process::exit(2);
                    };
                    let Some(dir) = log::dir() else {
                        eprintln!("poptop: no state directory — set HOME or XDG_STATE_HOME");
                        std::process::exit(2);
                    };
                    match log::open_day(&dir, date) {
                        Ok((s, said)) => {
                            for note in said {
                                eprintln!("poptop: {note}");
                            }
                            s
                        }
                        Err(why) => {
                            eprintln!("poptop: {why}");
                            std::process::exit(1);
                        }
                    }
                }
                None => {
                    // Two samples, as `--once` takes: every rate here is a
                    // difference, and one reading has nothing to difference
                    // against.
                    //
                    // Every optional source, unlike the interactive path where
                    // each is gated on a view being open. A script asking for
                    // "every metric by name" means it, and `null` because
                    // poptop chose not to ask is indistinguishable from `null`
                    // because the kernel does not publish it — which is the one
                    // distinction this whole format exists to keep.
                    let needs = Source::ALL.into_iter().fold(Needs::NONE, |n, s| n.with(s));
                    collector.sample(needs)?;
                    std::thread::sleep(settings.interval);
                    vec![collector.sample(needs)?]
                }
            };
            // One object per sample for JSON, newline-delimited, so a day is
            // streamable and `head` on it is not a parse error. For the line
            // format one stream, so the header block is written once for the
            // whole day rather than once a sample.
            match how {
                "json" => {
                    for s in &samples {
                        out!("{}", export::sample_json(s));
                    }
                }
                _ => out!("{}", export::lines_of(&samples)),
            }
            return Ok(());
        }
        Some("--days") => {
            flush(&warnings);
            let Some(dir) = log::dir() else {
                eprintln!("poptop: no state directory — set HOME or XDG_STATE_HOME");
                std::process::exit(2);
            };
            let days = log::days(&dir);
            if days.is_empty() {
                outln!("no logs in {}", dir.display());
            }
            for d in days {
                let size = std::fs::metadata(dir.join(log::file_name(d)))
                    .map(|m| m.len())
                    .unwrap_or(0);
                outln!("{d}  {}", ui::fmt_bytes(size));
            }
            return Ok(());
        }
        Some("--once") => {
            // `--once` logs too, where the user asked for a log. A monitor
            // that can only record while somebody is watching it is not much
            // of a recorder, and `poptop --once --log=on` from cron is a
            // legitimate way to fill a day without leaving a terminal open.
            let logging = settings.log.then(log::dir).flatten();
            if settings.log && logging.is_none() {
                // Said here as it is said on the interactive path. A cron job
                // with no `HOME` would otherwise exit cleanly and write to an
                // empty log forever.
                warnings.push(config::Warning(
                    "no state directory to log into — set HOME or XDG_STATE_HOME".into(),
                ));
            }
            let r = once(
                &mut collector,
                settings.interval,
                logging.as_deref().map(|dir| Logging {
                    dir: dir.to_path_buf(),
                    every: settings.log_interval,
                    days: settings.log_days,
                    bytes: settings.log_bytes,
                }),
                &mut warnings,
            );
            flush(&warnings);
            return r;
        }
        Some("--bench") => {
            flush(&warnings);
            // Measure with extended collection both off and on, so the cost
            // of gating a column is a number rather than a claim.
            let n = 20;
            for needs in [
                Needs::NONE,
                Needs::NONE.with(Source::Io),
                Needs::NONE.with(Source::Io).with(Source::Threads),
                Needs::NONE
                    .with(Source::Io)
                    .with(Source::Threads)
                    .with(Source::Exited),
                Needs::NONE
                    .with(Source::Io)
                    .with(Source::Threads)
                    .with(Source::Exited)
                    .with(Source::Cgroups),
                Needs::NONE.with(Source::Pss),
            ] {
                collector.sample(needs)?;
                let t0 = std::time::Instant::now();
                let mut count = 0;
                let mut tasks = 0;
                let mut exited = 0;
                let mut groups = 0;
                for _ in 0..n {
                    let s = collector.sample(needs)?;
                    count = s.procs.len();
                    tasks = s.tasks.as_ref().map_or(0, Vec::len);
                    exited += s.exited.as_ref().map_or(0, Vec::len);
                    groups = s.cgroups.as_ref().map_or(0, Vec::len);
                }
                let label = match (
                    needs.asked(Source::Io),
                    needs.asked(Source::Threads),
                    needs.asked(Source::Exited),
                    needs.asked(Source::Cgroups),
                ) {
                    (false, ..) if needs.asked(Source::Pss) => {
                        "pss only (one extra read a process)      "
                    }
                    (false, ..) => "io off, threads off, exits off, cgroups off",
                    (true, false, ..) => "io on,  threads off, exits off, cgroups off",
                    (true, true, false, _) => "io on,  threads on,  exits off, cgroups off",
                    (true, true, true, false) => "io on,  threads on,  exits on,  cgroups off",
                    (true, true, true, true) => "io on,  threads on,  exits on,  cgroups on ",
                };
                outln!(
                    "{label}: {count} procs, {tasks} threads, {exited} exits, \
                     {groups} cgroups, {:?}/sample",
                    t0.elapsed() / n
                );
            }
            return Ok(());
        }
        Some("--help" | "-h") => {
            flush(&warnings);
            outln!("{USAGE}");
            return Ok(());
        }
        Some("--check-theme") => {
            flush(&warnings);
            let Some(name) = args.get(1) else {
                eprintln!("poptop: --check-theme needs a theme name");
                std::process::exit(2);
            };
            return check_theme(name);
        }
        // Handled later, once the buffer can be sized for the day it opens —
        // named here so it is not rejected as unrecognised on the way past.
        Some("--read") => {}
        Some("--version" | "-V") => {
            flush(&warnings);
            outln!("poptop {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        Some(other) => {
            flush(&warnings);
            eprintln!("poptop: unrecognised option '{other}'\n\n{USAGE}");
            std::process::exit(2);
        }
        None => {}
    }

    // Reported here rather than beside the other config warnings, because
    // these are about what you will *see*. Emitting them before the argument
    // paths branch put a note about the configured theme on top of
    // `--check-theme`'s report of a different one, and on top of its usage
    // errors — a warning about a theme the user is not asking about.
    // A failing theme still loads, with one line saying so. It is the user's
    // terminal and their choice; poptop's job is to have the number and say it,
    // not to refuse — the same principle as rendering `—` rather than a
    // fabricated zero. Only for user themes: a built-in's shortfall is a
    // decision already made and documented, not news.
    if !settings.overrides.is_empty()
        && let Some(warning) = check::Report::of(&settings.theme, &theme).warning()
    {
        warnings.push(config::Warning(warning));
    }
    // Said once rather than per colour: a 256-colour terminal reading a
    // true-colour theme would otherwise print ten near-identical lines, and
    // the useful fact is which terminal you are on, not which token was first.
    if let Some(note) = config::theme_note(
        settings.tier,
        &settings.theme,
        &theme,
        &settings.overrides,
        &skipped,
    ) {
        warnings.push(config::Warning(note));
    }

    // A recorded day, if one was asked for. Read before the buffer is sized,
    // because a day holds as many samples as it holds and a buffer sized for
    // the live window would throw away the morning to make room for the
    // evening.
    let opened = match args.first().map(String::as_str) {
        Some("--read") => {
            let Some(text) = args.get(1) else {
                flush(&warnings);
                eprintln!("poptop: --read needs a date, as YYYY-MM-DD. `poptop --days` lists them");
                std::process::exit(2);
            };
            let Some(date) = log::Date::parse(text) else {
                flush(&warnings);
                eprintln!("poptop: `{text}` is not a date. Write it as YYYY-MM-DD");
                std::process::exit(2);
            };
            let Some(dir) = log::dir() else {
                flush(&warnings);
                eprintln!("poptop: no state directory — set HOME or XDG_STATE_HOME");
                std::process::exit(2);
            };
            let (samples, said) = match log::open_day(&dir, date) {
                Ok(pair) => pair,
                Err(why) => {
                    flush(&warnings);
                    eprintln!("poptop: {why}");
                    std::process::exit(1);
                }
            };
            warnings.extend(said.into_iter().map(config::Warning));
            Some(samples)
        }
        _ => None,
    };

    let capacity = opened
        .as_ref()
        .map_or(settings.history_len(), |s| s.len().max(1));
    let mut app = App::new(capacity);
    app.interval = settings.interval;
    app.theme = theme;
    app.glyphs = settings.glyphs;

    // Collect once before drawing so the first frame has real numbers. CPU
    // still reads zero — there is no previous counter to diff against yet.
    let first = collector.sample(app.needs())?;
    app.probe_io(&first);

    // Restored before the first live sample, so the new run's history lands
    // after the old one rather than being buried by it. Whatever gap sits
    // between the two, the timeline already draws its seam there and the
    // caption already reads real time.
    //
    // Samples from a previous boot are dropped, and that is not tidiness. A
    // process is identified by `(pid, started)`, and on Linux `started` counts
    // ticks since *boot* — so across a reboot a live pid 1 matches a restored
    // pid 1, and its history column would render the previous boot's CPU as
    // this process's own. macOS counts from the epoch and so does not have that
    // collision, but the check stays for both: a token whose meaning is
    // platform-specific is not one to make cross-boot promises about. Said out
    // loud rather than done quietly, because losing history is exactly what a
    // user should hear about.
    if settings.store {
        let boot = store::boot_time(&first);
        // Whatever the reader had to say about the file: a format it could
        // not read, or a field this build has no home for. Said out loud for
        // the same reason as the boot mismatch below — a user should hear
        // about history they are not getting.
        let mut said = Vec::new();
        let restored = store::load(&mut said).unwrap_or_default();
        warnings.extend(said.into_iter().map(config::Warning));
        let total = restored.len();
        let usable: Vec<sample::Sample> = restored
            .into_iter()
            .filter(|s| store::same_boot(store::boot_time(s), boot))
            .collect();
        if usable.len() < total {
            warnings.push(config::Warning(format!(
                "{} stored samples predate this boot and were discarded: a process \
                 is identified by pid and start time, and start time only means \
                 anything within one boot",
                total - usable.len()
            )));
        }
        let skip = usable.len().saturating_sub(app.history.capacity());
        for s in usable.into_iter().skip(skip) {
            app.history.push(s);
        }
    }
    let replaying = opened.is_some();
    app.replaying = replaying;
    if let Some(samples) = opened {
        let n = samples.len();
        // The interval the day was *recorded* at, not the live one. Almost
        // everything downstream is scaled by it — the timeline's seam
        // threshold, the growth column's refusal to divide by an unknown span,
        // the panel title's "of 2m23s buffered" — and a ten-minute log read at
        // one second is drawn as nothing but seams and labelled as two minutes.
        if let Some(every) = log::spacing(&samples) {
            app.interval = every;
        }
        for s in samples {
            app.history.push(s);
        }
        // The cursor lands on the oldest recorded sample rather than on the
        // live one. Somebody who opened a day meant to look at the day.
        app.history.goto_oldest();
        warnings.push(config::Warning(format!("opened {n} recorded samples")));
    } else {
        app.push(first);
    }

    // Nothing is written unless the user asked, once — the flag or the config
    // key. A missing state directory is a reason to say so rather than to
    // quietly not log: somebody who turned this on should hear that it is off.
    let logging = settings.log.then(log::dir).flatten().map(|dir| Logging {
        dir,
        every: settings.log_interval,
        days: settings.log_days,
        bytes: settings.log_bytes,
    });
    if settings.log && logging.is_none() {
        warnings.push(config::Warning(
            "no state directory to log into — set HOME or XDG_STATE_HOME".into(),
        ));
    }

    let mut terminal = ratatui::init();
    let mut said = Vec::new();
    let result = run(
        &mut terminal,
        &mut app,
        &mut collector,
        logging.as_ref(),
        replaying,
        &mut said,
    );
    ratatui::restore();
    warnings.extend(said.into_iter().map(config::Warning));
    // A source that is only opened when a view is — an exit listener, a cgroup
    // walk — finds out it is unavailable the first time somebody asks, which is
    // long after the startup warnings were printed. Drained here so the reason
    // reaches the reader instead of a channel nobody is listening to.
    warnings.extend(collector.take_notes().into_iter().map(config::Warning));
    // After the screen is restored, so a write error is a line the user can
    // actually read. Written on a clean exit only: a periodic flush is what
    // turns a live tool into a recorder, which is the thing this deliberately
    // is not.
    // Never after `--read`: the buffer holds a recorded day, and saving it
    // would replace the user's real restart history with whatever day they
    // opened — which they would then get back on the next ordinary launch.
    if settings.store
        && !replaying
        && result.is_ok()
        && let Err(e) = store::save(&app.history.iter().collect::<Vec<_>>())
    {
        warnings.push(config::Warning(format!("could not save history: {e}")));
    }
    flush(&warnings);
    result
}

/// Measure a theme and say whether it is legible, for scripts and reviewers.
///
/// The side effect worth having: a contributed theme arrives with a
/// measurement rather than a screenshot.
///
/// Measured at the top tier, not the detected one. The detected tier is read
/// from the environment, and in a CI job — where `TERM` is often unset, which
/// detects as monochrome — nothing would be measurable and every theme would
/// be certified. That is exactly where this command is meant to be run.
///
/// The question is whether the *theme* is legible, which is a property of the
/// colours it names rather than of the terminal that happens to be running the
/// check.
fn check_theme(name: &str) -> io::Result<()> {
    let mut warnings = Vec::new();
    let (palette, overrides) =
        match config::resolve_named_theme(name, &config::read_theme, &mut warnings) {
            Ok(pair) => pair,
            Err(why) => {
                eprintln!("poptop: {why}");
                std::process::exit(2);
            }
        };
    flush(&warnings);
    let (built, _) = theme::Theme::new(palette, theme::Tier::TrueColor).with_overrides(&overrides);
    // A user theme inherits `safe`, which has no caveat; a built-in speaks
    // for itself.
    let report = check::Report::of(name, &built)
        .with_caveat(theme::Palette::parse(name).and_then(theme::Palette::caveat));
    outln!("{report}");
    let code = report.verdict().exit_code();
    if code != 0 {
        std::process::exit(code);
    }
    Ok(())
}

/// Print one sample as plain text and exit.
///
/// Two samples are taken, one interval apart: CPU figures are deltas between
/// reads, so a single sample could only ever report zero.
/// The scripted machine-is-capped line, or nothing.
///
/// A script reading only `cpu` sees 100% on a capped machine and on a healthy
/// one — the same failure the `stall` line was added to prevent, and here there
/// is nothing else in the output that could give it away.
///
/// Split from `once` so it can be tested: that function writes to stdout, and a
/// figure this easy to forget wants an assertion rather than an eyeball.
fn clock_line(s: &sample::Sample) -> Option<String> {
    let clock = s.clock_ceiling.filter(|c| *c < ui::CLOCK_NOMINAL)?;
    Some(format!(
        "clock   {clock:.1}%  of nominal — the machine is capped"
    ))
}

fn once(
    collector: &mut impl Collector,
    interval: Duration,
    logging: Option<Logging>,
    warnings: &mut Vec<config::Warning>,
) -> io::Result<()> {
    let needs = Needs::NONE
        .with(Source::Io)
        .with(Source::Exited)
        .with(Source::Cgroups);
    collector.sample(needs)?;
    std::thread::sleep(interval);
    let s = collector.sample(needs)?;
    // Nothing poptop prints depends on the log. A full disk or a read-only
    // state directory used to propagate out of here with `?`, so
    // `poptop --once --log=on` exited non-zero having printed nothing — the
    // exact inversion of the rule this feature is built on.
    if let Some(cfg) = logging {
        match log::append(&cfg.dir, s.at, &[&s], cfg.bytes) {
            Ok(true) => {}
            Ok(false) => warnings.push(config::Warning(
                "today's log is at its size limit and was not written to".into(),
            )),
            Err(e) => warnings.push(config::Warning(format!("could not write the log: {e}"))),
        }
        // Retention applies here too. A machine logged only from cron would
        // otherwise accumulate one file a day forever, with `log-days` and
        // `log-bytes` never reached by any code path.
        if let Some(today) = log::date_of(s.at) {
            warnings.extend(
                log::prune(&cfg.dir, cfg.days, cfg.bytes, today)
                    .into_iter()
                    .map(config::Warning),
            );
        }
    }

    outln!(
        "cpu     {:.1}%  ({} cores)",
        s.cpu_total,
        s.cpu_per_core.len()
    );
    // The saturation figures belong here as much as in the header, and
    // arguably more: `--once` exists to be scripted, and a script that reads
    // only cpu and mem will read a stalled machine as an idle one. Omitted
    // rather than zeroed where the platform cannot see them.
    if let Some(iowait) = s.iowait {
        outln!("wait    {iowait:.1}%  of wall clock, idle with IO outstanding");
    }
    if let Some(running) = s.running {
        outln!(
            "run     {running}  runnable, on {} cores",
            s.cpu_per_core.len()
        );
    }
    if let Some(blocked) = s.blocked {
        outln!("blocked {blocked}  in uninterruptible sleep");
    }
    if let Some(p) = s.pressure {
        let (what, pct) = p.worst();
        outln!("stall   {pct:.1}%  of the last 10s with every task stopped, on {what}");
    }
    if let Some(d) = s.busiest_disk() {
        match d.await_ms {
            Some(a) => outln!(
                "disk    {:.1}%  {} utilised, {a:.1}ms per operation",
                d.util,
                d.name
            ),
            None => outln!("disk    {:.1}%  {} utilised", d.util, d.name),
        }
    }
    // The counter that says the network is unhealthy, which no other line here
    // would show: a script reading cpu and mem sees a machine with a dead path
    // to its peers as perfectly idle.
    if let Some((what, n)) = s.net.as_ref().and_then(sample::NetStat::trouble) {
        outln!("net     {n}  {what} in the last interval");
    }
    // NFS, where the machine mounts or serves it. On a box whose storage is
    // remote the `disk` line above describes a local disk doing nothing while
    // the machine waits on the network, and nothing else here would say so.
    // Per second, like every other rate here, so a script comparing two
    // machines does not have to know what interval each was run at.
    if let Some(nfs) = s.nfs.as_ref().filter(|n| n.in_use()) {
        for m in &nfs.mounts {
            // A mean over the interval, not a rate: a mount is not faster
            // because it was watched for longer. `—` where nothing completed.
            let rtt = m.rtt_ms.map_or("—".to_string(), |v| format!("{v:.1}ms"));
            outln!(
                "nfs     {}  {}, {}/s calls, {}/s resent, {rtt} mean, {}/s read, {}/s written",
                m.mount,
                m.server,
                m.ops,
                m.retrans,
                human(m.read),
                human(m.write)
            );
        }
        outln!(
            "nfsc    {}/s calls, {}/s resent  client, across every mount",
            nfs.client_calls,
            nfs.client_retrans
        );
        if let Some(calls) = nfs.server_calls {
            let part = |v: Option<u64>| v.map_or("—".to_string(), |n| n.to_string());
            let bytes = |v: Option<u64>| v.map_or("—".to_string(), human);
            outln!(
                "nfsd    {calls}/s calls, {}/s read, {}/s written, {} hits, {} misses, {} refused",
                bytes(nfs.server_read),
                bytes(nfs.server_write),
                part(nfs.server_hits),
                part(nfs.server_misses),
                part(nfs.server_badauth)
            );
        }
    }
    // The only line here that describes a hard failure rather than a slowdown,
    // and the one a script most wants: a machine out of disk does not get
    // slower, it stops. Printed whatever the fullness, because a script has no
    // header to compare it against and no threshold of its own.
    if let Some(f) = s.fullest() {
        outln!(
            "fs      {:.1}%  {} full, {} of {} available",
            f.used_pct(),
            f.mount,
            human(f.avail),
            human(f.total)
        );
    }
    outln!(
        "mem     {:.1}%  {} / {} used, {} available",
        s.mem.used_pct(),
        human(s.mem.used),
        human(s.mem.total),
        human(s.mem.available)
    );
    if s.mem.swap_total > 0 {
        outln!(
            "swap    {:.1}%  {} / {}",
            s.mem.swap_pct(),
            human(s.mem.swap_used),
            human(s.mem.swap_total)
        );
    }
    outln!("load    {:.2} {:.2} {:.2}", s.load[0], s.load[1], s.load[2]);
    // The CPU line's other classes, each absent rather than zero where the
    // platform does not publish it. `steal` first: on a cloud instance it is
    // the difference between a busy box and a box that is not being given one.
    for (label, v) in [
        ("steal", s.steal),
        ("guest", s.guest),
        ("irq", s.irq),
        ("softirq", s.softirq),
    ] {
        match v {
            Some(v) => outln!("{label:<7} {v:.1}%  of wall clock"),
            None => outln!("{label:<7} —  not published here"),
        }
    }
    // Each says its own absence. Collapsing the pair into one line claimed the
    // platform published neither when it had only withheld one.
    let rate_of = |v: Option<u64>| match v {
        Some(n) => format!("{n}/s"),
        None => "—".to_string(),
    };
    outln!(
        "switch  {} context, {} interrupts",
        rate_of(s.ctxt),
        rate_of(s.intr)
    );
    // What the machine's memory is actually holding. `used` above is
    // `total - available`, which includes every one of these — so a leak in
    // kernel memory shows there as used memory belonging to no process, and
    // this is where it becomes visible.
    // A loop rather than a closure: `outln!` returns from the *function* when
    // stdout is gone, which a closure cannot do for it.
    // Rates, because the swap *level* cannot tell a thrashing box from one
    // sitting on idle swap — and an OOM count, because a killed process is gone
    // from the next sample with nothing else saying why.
    for (label, v) in [
        ("pagein", s.pgin),
        ("pageout", s.pgout),
        ("swapin", s.swin),
        ("swapout", s.swout),
    ] {
        match v {
            Some(b) => outln!("{label:<7} {}/s", ui::fmt_bytes(b)),
            None => outln!("{label:<7} —  not published here"),
        }
    }
    match s.oom_kills {
        Some(n) => outln!("oomkill {n}  in the last interval"),
        None => outln!("oomkill —  not published here"),
    }
    for (label, v) in [
        ("dirty", s.mem.dirty),
        ("slab", s.mem.slab),
        ("slabrec", s.mem.slab_reclaimable),
        ("shmem", s.mem.shmem),
        ("pgtab", s.mem.page_tables),
        ("hugetot", s.mem.huge_total),
        ("hugeuse", s.mem.huge_used),
    ] {
        match v {
            Some(b) => outln!("{label:<7} {}", ui::fmt_bytes(b)),
            None => outln!("{label:<7} —  not published here"),
        }
    }
    // Per node, and only where there is more than one. A box with a single
    // node says nothing: its per-node figures are the figures above.
    match &s.nodes {
        Some(nodes) => {
            for n in nodes {
                let cpu = n.cpu.map_or("—".to_string(), |v| format!("{v:.1}%"));
                // The parts, not just the total: a node whose free memory is
                // mostly page cache and a node that is genuinely empty are the
                // same number here otherwise, and only one of them is fine.
                let part = |v: Option<u64>| v.map_or("—".to_string(), ui::fmt_bytes);
                outln!(
                    "node{:<3} {} of {} free, cpu {cpu}, file {}, dirty {}, shmem {}",
                    n.id,
                    ui::fmt_bytes(n.free),
                    ui::fmt_bytes(n.total),
                    part(n.file),
                    part(n.dirty),
                    part(n.shmem)
                );
            }
        }
        None => outln!("nodes   —  one node, or nothing readable here"),
    }
    outln!("procs   {}", s.procs.len());
    // The processes that lived and died inside the interval — the ones a
    // sample of `/proc` at an instant cannot see at all. An em dash where the
    // kernel would not let poptop listen, never a zero: "none exited" and "I
    // was not allowed to look" are opposite answers.
    match &s.cgroups {
        Some(g) => {
            outln!("cgroups {}  walked", g.len());
            for c in g.iter().take(3) {
                let psi = c.pressure.map_or("—".to_string(), |p| {
                    format!("{:.1}", p.io.some.max(p.cpu.some))
                });
                let cpu = c.cpu.map_or("—".to_string(), |v| format!("{v:.1}%"));
                outln!("        {} cpu {cpu} psi {psi}", c.path);
            }
        }
        None => outln!("cgroups —  not collected"),
    }
    match &s.exited {
        Some(e) => {
            outln!("exited  {}  in the last interval", e.len());
            for p in e.iter().take(3) {
                outln!("        {} pid {} {:.1}%", p.name, p.pid, p.cpu);
            }
        }
        None => outln!("exited  —  not collected"),
    }
    if s.io_denied > 0 {
        outln!(
            "io     {}/{} processes unreadable — run as root to see them",
            s.io_denied,
            s.procs.len()
        );
    }

    if let Some(line) = clock_line(&s) {
        outln!("{line}");
    }

    let mut top = s.procs.clone();
    top.sort_by(|a, b| b.cpu.total_cmp(&a.cpu));
    outln!(
        "\n{:>7}  {:>6}  {:>9}  {:>10}  {:>10}  COMMAND",
        "PID",
        "CPU%",
        "RSS",
        "DISK R/s",
        "DISK W/s"
    );
    for p in top.iter().take(10) {
        // A dash, never a zero: this process could not be read, which is not
        // the same as it doing no IO.
        let (r, w) = match p.io {
            Some(io) => (human(io.read), human(io.write)),
            None => ("—".into(), "—".into()),
        };
        outln!(
            "{:>7}  {:>6.1}  {:>9}  {r:>10}  {w:>10}  {}",
            p.pid,
            p.cpu,
            human(p.rss),
            // Cut to a width, unlike the TUI which elides to its column. A
            // Chrome renderer's arguments run past any terminal, and wrapping
            // one row over four lines makes the other nine unreadable.
            ui::elide_middle(p.command(), 56)
        );
    }
    Ok(())
}

fn human(b: u64) -> String {
    const U: [&str; 5] = ["B", "K", "M", "G", "T"];
    let (mut v, mut i) = (b as f64, 0);
    while v >= 1024.0 && i < U.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    format!("{v:.1}{}", U[i])
}

/// How long a stretch has to be before a report calls it sustained.
///
/// Five minutes, which is the shortest run of trouble anybody investigates. A
/// window that is too short reports every spike as a run, and one too long
/// misses the incident that resolved itself before anyone was paged.
const REPORT_WINDOW: Duration = Duration::from_secs(300);

/// Where and how often the log is written, when it is written at all.
///
/// `None` is the default and the whole point: poptop logs if it is left running
/// and works if it was not, so the ordinary run writes nothing.
pub struct Logging {
    pub dir: std::path::PathBuf,
    pub every: Duration,
    pub days: u32,
    pub bytes: u64,
}

fn run(
    terminal: &mut ratatui::DefaultTerminal,
    app: &mut App,
    collector: &mut impl Collector,
    logging: Option<&Logging>,
    // Whether the buffer holds a recorded day rather than live history.
    //
    // Sampling continues either way — the log keeps being written, and the
    // collector's counters stay warm — but a live sample must not be pushed
    // into a replayed buffer. The buffer is sized to the day exactly, so each
    // push evicts the oldest recorded sample and shifts the pinned cursor onto
    // a different moment: a day left open for its own length would become
    // entirely live samples, silently.
    replaying: bool,
    notes: &mut Vec<String>,
) -> io::Result<()> {
    let interval = app.interval;
    // The first sample reaches the log immediately rather than one logging
    // interval in. A poptop left running for nine minutes and killed would
    // otherwise have recorded nothing at all, which is the case somebody who
    // asked for a log is least willing to forgive.
    let mut next_log = Instant::now();
    let mut last_pruned: Option<log::Date> = None;
    // A fixed cadence, not "one interval after the last sample finished".
    //
    // Restarting the clock after collection adds the collect and draw time to
    // every period, so the timestamps drift steadily away from the rate they
    // claim — on a box with thousands of processes, far enough that the gap
    // detector would see a missed tick on every single cell and paint the
    // whole graph as seams. The interval is a schedule, so schedule against it.
    let mut next_sample = Instant::now() + interval;

    loop {
        terminal.draw(|f| ui::draw(f, app))?;

        // Poll with whatever is left of the sample interval: input stays
        // responsive without spinning, and sampling stays on schedule.
        let timeout = next_sample.saturating_duration_since(Instant::now());
        if event::poll(timeout)?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            handle_key(app, key.code, key.modifiers);
        }

        if Instant::now() >= next_sample {
            // Sampling continues while paused — that is the whole point. The
            // cursor stays put, the buffer keeps filling behind it.
            // Timed, so collection that has grown past its share of the
            // interval gives something up rather than quietly becoming part of
            // the load it is measuring. See `App::spent`.
            let t0 = Instant::now();
            let s = collector.sample(app.needs())?;
            app.spent(t0.elapsed(), interval);
            // Logged before the buffer takes it, so what is written is one
            // sample rather than however many the buffer happens to hold.
            if let Some(cfg) = logging.filter(|_| Instant::now() >= next_log) {
                let at = s.at;
                // Onto the panel while it is true, as well as into the lines
                // printed at exit. A disk that filled at 10:00 is something
                // the reader needs at 10:00; a message they see when they quit
                // is one they see after it stopped mattering.
                let said = match log::append(&cfg.dir, at, &[&s], cfg.bytes) {
                    Ok(true) => None,
                    Ok(false) => Some(
                        "the log is at its size limit and is no longer being written to"
                            .to_string(),
                    ),
                    Err(e) => Some(format!("could not write the log: {e}")),
                };
                app.log_note = said.clone();
                // Once in the exit lines. A disk that filled would otherwise
                // add one every logging interval until the tool is closed, and
                // the first already said it.
                if let Some(said) = said.filter(|s| !notes.contains(s)) {
                    notes.push(said);
                }
                next_log = Instant::now() + cfg.every;
                // Retention is applied when the date changes, not on a timer:
                // the rule is about days, and a poptop left running over
                // midnight is exactly the one that needs it applied.
                let today = log::date_of(at);
                if today.is_some() && today != last_pruned {
                    last_pruned = today;
                    if let Some(d) = today {
                        notes.extend(log::prune(&cfg.dir, cfg.days, cfg.bytes, d));
                    }
                }
            }
            if !replaying {
                app.push(s);
            }
            next_sample += interval;
            // Falling a whole interval behind means the host cannot sustain
            // the rate. Resync rather than catch up: catching up would sample
            // flat out until the backlog cleared, which is the worst thing to
            // do to the loaded box that caused the backlog. The samples really
            // are further apart than the nominal rate, and the timeline says
            // so — that is what the seam is for.
            let now = Instant::now();
            if next_sample <= now {
                next_sample = now + interval;
            }
        }

        if app.should_quit {
            return Ok(());
        }
    }
}

/// The key handler, for tests that need to press a key rather than set a flag.
///
/// Exposed because the modal boxes are state machines: what `Ctrl-C` does while
/// the jump box is open, and what an arrow key does to the last jump's answer,
/// are properties of the handler and cannot be checked by poking the `App`.
#[cfg(test)]
pub fn handle_key_for_test(app: &mut App, code: KeyCode) {
    handle_key(app, code, KeyModifiers::NONE);
}

#[cfg(test)]
pub fn handle_key_with_mods_for_test(app: &mut App, code: KeyCode, mods: KeyModifiers) {
    handle_key(app, code, mods);
}

fn handle_key(app: &mut App, code: KeyCode, mods: KeyModifiers) {
    // Before the modal boxes, not after. `Ctrl-C` is the only quit-on-interrupt
    // path there is — poptop installs no SIGINT handler, and raw mode means the
    // terminal will not deliver one — so a box that swallowed it as a literal
    // `c` left the reflexive escape from a full-screen program doing nothing.
    if code == KeyCode::Char('c') && mods.contains(KeyModifiers::CONTROL) {
        app.should_quit = true;
        return;
    }
    if app.editing_filter {
        match code {
            KeyCode::Enter | KeyCode::Esc => app.editing_filter = false,
            KeyCode::Backspace => {
                app.filter.pop();
            }
            KeyCode::Char(c) => {
                app.filter.push(c);
            }
            _ => {}
        }
        return;
    }
    if app.editing_jump {
        match code {
            // Enter acts, Esc abandons. Unlike the filter, which applies as it
            // is typed: a filter narrows a table you are already looking at,
            // and a jump moves the cursor — doing that on every keystroke
            // would walk the reader through `0`, `03`, `03:0` before arriving.
            KeyCode::Enter => {
                app.editing_jump = false;
                app.jump_to(std::time::SystemTime::now());
            }
            KeyCode::Esc => {
                app.editing_jump = false;
                app.jump_note = None;
            }
            KeyCode::Backspace => {
                app.jump.pop();
            }
            KeyCode::Char(c) => {
                app.jump.push(c);
            }
            _ => {}
        }
        return;
    }

    // The answer to the last jump belongs to the last jump. Left standing it
    // described nothing on screen as soon as the reader scrubbed away, and it
    // kept the key hints hidden for the rest of the run.
    if code != KeyCode::Char('b') {
        app.jump_note = None;
    }
    match code {
        KeyCode::Char('q') | KeyCode::Esc => app.should_quit = true,
        KeyCode::Char('c') if mods.contains(KeyModifiers::CONTROL) => app.should_quit = true,

        // Scrubbing. Shift jumps ten samples at a time for crossing a long
        // buffer without holding the key down.
        KeyCode::Left | KeyCode::Char('h') => {
            let step = if mods.contains(KeyModifiers::SHIFT) {
                10
            } else {
                1
            };
            app.history.scrub(-step);
        }
        KeyCode::Right | KeyCode::Char('l') => {
            let step = if mods.contains(KeyModifiers::SHIFT) {
                10
            } else {
                1
            };
            app.history.scrub(step);
        }
        KeyCode::Char(' ') => {
            // Space toggles: pause pins the cursor where it is, resume returns
            // to the live edge.
            if app.history.is_live() {
                app.history.scrub(-1);
            } else {
                app.history.goto_live();
            }
        }
        KeyCode::Home => {
            app.history.goto_oldest();
        }
        KeyCode::End => {
            app.history.goto_live();
        }

        KeyCode::Up | KeyCode::Char('k') => app.select_delta(-1),
        KeyCode::Down | KeyCode::Char('j') => app.select_delta(1),
        KeyCode::PageUp => app.select_delta(-10),
        KeyCode::PageDown => app.select_delta(10),

        // '=' so zooming out does not require Shift on most layouts.
        KeyCode::Char('+' | '=') => app.zoom_in(),
        KeyCode::Char('-' | '_') => app.zoom_out(),

        // The selection is of a process, so re-sorting moves the row under it
        // and keeps it selected. Resetting to the top here was the same bug as
        // the one scrubbing had.
        KeyCode::Char('s') => app.sort = app.sort.next(app.io_collected(), app.view),
        // Column sets, over the same rows and the same renderer. `v` because
        // atop spends seven keys on this and poptop has three views and few
        // free letters.
        KeyCode::Char('v') => {
            app.view = app.view.next();
            // Asking for the view again is asking for its columns again, if the
            // budget had taken them away.
            app.insist_for_view();
            // A sort the new view cannot show would be an ordering with no
            // visible reason for it, so switching views brings the sort with
            // it when it has to.
            if !app.view.sorts().contains(&app.sort) {
                app.sort = app.view.default_sort_for(app.io_collected());
            }
        }
        // Accept the suggestion. Never applied on its own: a table that
        // reorders itself under the reader is worse than one that does not, so
        // the constraint is named and this is the one key that acts on it.
        KeyCode::Char('S') => {
            if let Some(c) = app.constraint() {
                app.sort = c.sort();
                // Sorting by a column that is not on screen answers the
                // question invisibly: the rows move and nothing says why. The
                // reader asked for this by pressing the key, so the columns
                // come with it.
                if c.sort() == app::Sort::Disk {
                    app.show_io = true;
                }
            }
        }
        KeyCode::Char('i') => app.toggle_io(),
        // atop's key for the same thing.
        KeyCode::Char('y') => app.toggle_threads(),
        // atop shows cgroups on G. C here, because g is already grouping and
        // G is not free either.
        KeyCode::Char('C') => app.toggle_cgroups(),
        KeyCode::Char('K') => app.show_kernel = !app.show_kernel,
        KeyCode::Char('t') => {
            app.tree = !app.tree;
            // Grouping destroys parentage by construction, so a grouped tree
            // would be a tree of things that are not processes. bottom makes
            // the same two exclusive.
            if app.tree {
                app.group = crate::app::Grouping::Off;
            }
        }
        KeyCode::Char('d') => app.detail = !app.detail,
        KeyCode::Char('g') => {
            // A cycle: off, by name, by user, by container — atop's `p`, `u`
            // and `j` on one key. Each is the same machinery with a different
            // key, so they are a choice rather than three exclusive layouts.
            app.group = app.group.next();
            if app.group != crate::app::Grouping::Off {
                app.tree = false;
            }
        }
        KeyCode::Char('/') => {
            app.editing_filter = true;
            app.filter.clear();
        }
        // `b` for the beginning of a moment, which is atop's `-b`. Not `j`:
        // that is already "select the next process", the vim binding beside
        // `k`, and a key that quietly stopped moving the selection would be a
        // worse trade than an unfamiliar letter.
        KeyCode::Char('b') => {
            app.editing_jump = true;
            app.jump.clear();
            app.jump_note = None;
        }
        _ => {}
    }
}
