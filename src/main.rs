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
mod glyphs;
mod history;
mod persist;
mod query;
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
                    per member. Not available with the tree.
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
        Some("--once") => {
            flush(&warnings);
            return once(&mut collector, settings.interval);
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

    let mut app = App::new(settings.history_len());
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
    app.push(first);

    let mut terminal = ratatui::init();
    let result = run(&mut terminal, &mut app, &mut collector);
    ratatui::restore();
    // A source that is only opened when a view is — an exit listener, a cgroup
    // walk — finds out it is unavailable the first time somebody asks, which is
    // long after the startup warnings were printed. Drained here so the reason
    // reaches the reader instead of a channel nobody is listening to.
    warnings.extend(collector.take_notes().into_iter().map(config::Warning));
    // After the screen is restored, so a write error is a line the user can
    // actually read. Written on a clean exit only: a periodic flush is what
    // turns a live tool into a recorder, which is the thing this deliberately
    // is not.
    if settings.store
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

fn once(collector: &mut impl Collector, interval: Duration) -> io::Result<()> {
    let needs = Needs::NONE
        .with(Source::Io)
        .with(Source::Exited)
        .with(Source::Cgroups);
    collector.sample(needs)?;
    std::thread::sleep(interval);
    let s = collector.sample(needs)?;

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

fn run(
    terminal: &mut ratatui::DefaultTerminal,
    app: &mut App,
    collector: &mut impl Collector,
) -> io::Result<()> {
    let interval = app.interval;
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
            app.push(s);
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

fn handle_key(app: &mut App, code: KeyCode, mods: KeyModifiers) {
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
            // A cycle: off, by name, by container. Folding by container is the
            // same machinery with a different key, so it is a choice rather
            // than a fifth exclusive layout.
            app.group = app.group.next();
            if app.group != crate::app::Grouping::Off {
                app.tree = false;
            }
        }
        KeyCode::Char('/') => {
            app.editing_filter = true;
            app.filter.clear();
        }
        _ => {}
    }
}
