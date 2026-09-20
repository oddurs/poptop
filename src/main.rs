//! poptop — a system monitor you can rewind.
//!
//! Copyright (C) 2026  poptop contributors
//!
//! This program is free software: you can redistribute it and/or modify it
//! under the terms of the GNU General Public License as published by the Free
//! Software Foundation, either version 3 of the License, or (at your option)
//! any later version. See the LICENSE file for details.

mod app;
#[cfg(test)]
mod budget;
mod check;
mod collect;
mod command;
mod config;
mod cvd;
mod export;
mod glyphs;
mod history;
mod keys;
mod log;
#[cfg(test)]
mod mangle;
mod menu;
mod persist;
mod query;
mod report;
mod sample;
mod signal;
mod store;
mod term;
mod theme;
mod tree;
mod ui;

#[cfg(test)]
mod docs_tests;
#[cfg(test)]
mod ui_tests;

use app::App;
use collect::{Collector, Needs, Platform, Source};
use command::Action;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

const USAGE: &str = "\
poptop — a system monitor you can rewind

USAGE:
    poptop            interactive mode
    poptop --once     print one plain-text sample and exit
    poptop --read DATE
                    open a recorded day (YYYY-MM-DD) instead of live
    poptop --days     list the recorded days and their sizes
    poptop --export=json|line [DATE] [--follow [--for SPAN]]
                    every metric, by name, for a script. With a date, the whole
                    of that recorded day rather than the machine now. --follow
                    keeps going: without a date a record an interval, flushed
                    as it is taken; with one, the day's entries as they are
                    appended, which is how to subscribe to a log something else
                    is writing. Either ends when it is stopped, when the reader
                    goes away, or after --for SPAN. Under --follow the line
                    format writes its header block once, at the top of the
                    stream.
    poptop --schema   what --export reports: every record, field, type and unit
    poptop --report [DATE]
                    summarise a recorded day: peak and sustained, and what was
                    responsible for each. Today unless a date is given.
    poptop --keys     every action and the keys bound to it
    poptop --config   every setting, its value, and where that value came from
    poptop --write-config
                    write a commented config file of the current settings,
                    at the path above, refusing to overwrite one
    poptop --bench    time 20 collection passes (development)
    poptop --check-theme NAME
                    measure a theme and say whether it is legible

    --graph=SET     how the timeline is drawn. `braille` (default) draws bars
                    at four levels a cell, and two samples a cell in the table's
                    sparkline; `block` resolves more — eight levels — with the
                    eighths ramp; `line` draws the outline instead of filling
                    under it; `ascii` needs no Unicode. `--glyphs` is the old
                    name and still works. Falls back to ascii on a Linux
                    console.
    --scale=WHERE   where the y-axis starts: zero (default) or fit. `fit`
                    reclaims the rows a high flat series wastes — memory at
                    72-85% spends most of a 0-100 panel on ink that never
                    changes — at the cost of drawing that panel as a line
                    rather than bars, since a bar on a truncated axis
                    misstates its own magnitude.
    --density=HOW   compact, comfortable (default) or spacious. How much air
                    the layout is given — the table's indent, the gap between
                    header figures, and a blank row between the graph and the
                    table. Every one of them is given up on a terminal too
                    small to afford it. Also in the View menu.
    --surface=WHERE auto (default) or off. poptop asks the terminal for its
                    background colour and steps its panels a few per cent away
                    from it, so the layers match whatever scheme you already
                    have. `off` paints nothing and skips the question.
    --mouse=on|off  take the mouse (default on). Click the menu, click a row
                    to select it, click or drag the timeline to scrub, wheel to
                    move whichever of the two is under the pointer. While
                    poptop has the mouse, dragging no longer selects text for
                    copying; most terminals restore that if you hold Shift.
    --color=TIER    auto (default), mono, 16, 256, or true. Honours NO_COLOR.
    --interval=SPAN time between samples: 500ms, 2s, 10m (default 1s)
    --smooth=SPAN   how long the table's figures are averaged over (default 5s,
                    or `off`). A process table at one sample a second is mostly
                    noise, and the rows swap places while you are reading them.
                    The timeline is not averaged: it is where a spike has to be
                    found, and the table is what you read once you have found
                    one.
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
    --signals=on|off
                    allow x and X to send TERM and KILL to the selected process
                    (default off). A monitor that cannot change the machine is
                    a monitor that cannot break it, so poptop reads files and
                    nothing else until you say otherwise. Never while scrubbing,
                    and never to a pid that has been recycled since you selected
                    it — see the KEYS section.
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
        graph    = block    # comments run to the end of the line
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

EXIT STATUS:
    0               done as asked
    1               could not: a recorded day that cannot be read, a failure
                    reading the machine, or a --check-theme verdict other
                    than PASS
    2               would not: a command line or setting that cannot be run
                    as written, no state directory for a command that needs
                    one, or the interactive monitor without a terminal

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
    q, Esc          quit. Esc backs out one level first: it leaves the
                    filter or jump box, cancels a signal, or lets go of the
                    selected process, and quits only when there is none.
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
    v               the next tab: memory (what each process's memory
                    costs, what it has reserved, whether it is being paged
                    in), then disk (the throughput columns, whatever the
                    width), then back. The strip above the table names them,
                    so this is the keyboard's way of doing what a click does.
    y               expand the selected process into its threads — the
                    selected one only, so the table does not grow ninefold.
    C               show cgroups in place of processes: what each is using and
                    how stalled it is. Linux, cgroup v2.
    ?               list every key.
    F10             open the menu bar: File, Edit, View, Go, Process, Help.
                    Alt and a title's underlined letter opens that one
                    directly; arrows move, Enter chooses, Esc closes. Every
                    item names the key that also runs it, so the menu teaches
                    itself out of use.
    Tab             the next tab, and Shift-Tab the previous one. 1-9 open one
                    by number. A tab is a set of columns and the sort that goes
                    with them: CPU, memory, disk.
    Enter           open the inspector on the selected process.
    x, X            send TERM (x) or KILL (X) to the selected process, after a
                    confirmation that names it — the pid is the part that gets
                    misread, and poptop knows the command line. Off unless
                    --signals=on, and the key says so if it is not.

                    Two rules poptop can offer and other monitors cannot,
                    because their process tables are always the present and
                    poptop's may be four minutes old:

                      · nothing is sent while scrubbing. That table is history.
                      · the (pid, start time) pair is rechecked against the
                        newest sample, and then against the kernel as the
                        signal is sent — through a pidfd on Linux 5.3 and
                        later. A pid the kernel has since handed to something
                        else is refused by name.

    R               read the theme file again, so a colour can be tried
                    without restarting. A theme file that changes on disk is
                    picked up on the next sample anyway.
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
    // Written rather than `eprintln!`ed: that panics when stderr cannot be
    // written — a terminal that has gone away, `2>&-` — and a warning that
    // cannot be delivered is not a reason to crash on the way out.
    use std::io::Write as _;
    let mut err = io::stderr().lock();
    for w in warnings {
        let _ = writeln!(err, "poptop: {w}");
    }
}

/// What the command line asked for, once the settings flags are taken out.
///
/// Settings (`--theme=…`, `--interval=…`) are `config::resolve`'s. This is
/// what is left: at most one command word and its arguments.
#[derive(Debug, PartialEq)]
enum Command {
    /// The monitor: live, or a recorded day with `--read`.
    Tui {
        day: Option<log::Date>,
    },
    Once,
    /// A day's summary; today when none is named.
    Report(Option<log::Date>),
    Schema,
    /// Every setting, its value, and where it came from.
    Config,
    /// Every action and the keys bound to it.
    Keys,
    /// A config file of the current settings, at the config path.
    WriteConfig,
    /// `json` or `line`, of a recorded day or of the machine now.
    ///
    /// `follow` keeps sampling and writing one record an interval, until it is
    /// stopped or `until` has gone by.
    Export {
        json: bool,
        day: Option<log::Date>,
        follow: bool,
        until: Option<Duration>,
    },
    Days,
    Bench,
    Help,
    Version,
    CheckTheme(String),
}

/// A command line that cannot be run, and what to say about it. Always exit 2.
#[derive(Debug, PartialEq)]
struct Usage(String);

const NO_STATE_DIR: &str = "no state directory — set HOME or XDG_STATE_HOME";
const NO_CONFIG_DIR: &str = "no config directory — set HOME or XDG_CONFIG_HOME";

/// Decide what the command line asks for, without doing any of it.
///
/// Pure, so every way of getting it wrong is a row in a table rather than a
/// process to spawn. Each command word is accepted as `--word VALUE` and
/// `--word=VALUE` alike, and anything it does not take is refused: an argument
/// dropped silently is an instruction ignored, and `poptop --read DATE --once`
/// opening the day and ignoring `--once` was exactly that.
fn command(args: &[String]) -> Result<Command, Usage> {
    let Some(first) = args.first() else {
        return Ok(Command::Tui { day: None });
    };
    let (word, inline) = match first.split_once('=') {
        Some((w, v)) if w.starts_with("--") => (w, Some(v)),
        _ => (first.as_str(), None),
    };
    // The command word's arguments, the inline one first. An empty inline value
    // (`--report=`) is no value, as it always was.
    let mut rest = inline
        .filter(|v| !v.is_empty())
        .into_iter()
        .chain(args[1..].iter().map(String::as_str));
    let date = |t: &str| {
        log::Date::parse(t)
            .ok_or_else(|| Usage(format!("`{t}` is not a date. Write it as YYYY-MM-DD")))
    };
    let command = match word {
        "--report" => Command::Report(rest.next().map(date).transpose()?),
        "--export" => {
            let json = match rest.next() {
                Some("json") => true,
                Some("line") => false,
                _ => return Err(Usage("--export takes `json` or `line`".into())),
            };
            // The date, then the modifiers. Taken here rather than by
            // `config::resolve` because they say what this command does and
            // not what poptop is: `--follow` means nothing to the TUI.
            let (mut day, mut follow, mut until) = (None, false, None);
            while let Some(word) = rest.next() {
                let (word, inline) = match word.split_once('=') {
                    Some((w, v)) if w.starts_with("--") => (w, Some(v)),
                    _ => (word, None),
                };
                match word {
                    "--follow" => follow = true,
                    "--for" => {
                        let Some(span) = inline.filter(|v| !v.is_empty()).or_else(|| rest.next())
                        else {
                            return Err(Usage("--for needs a span, as `30s`, `5m` or `2h`".into()));
                        };
                        until =
                            Some(config::duration(span).map_err(|want| {
                                Usage(format!("--for `{span}`: expected {want}"))
                            })?);
                    }
                    _ if day.is_none() && !word.starts_with("--") => day = Some(date(word)?),
                    _ => {
                        return Err(Usage(format!(
                            "--export does not take `{word}` — a date, --follow, or --for SPAN"
                        )));
                    }
                }
            }
            // Refused rather than ignored, as everything else here is. A
            // `--for` that quietly did nothing would read as a feed that
            // stopped on time and printed one sample.
            if until.is_some() && !follow {
                return Err(Usage(
                    "--for is --follow's; there is no feed to stop".into(),
                ));
            }
            Command::Export {
                json,
                day,
                follow,
                until,
            }
        }
        "--read" => {
            let Some(text) = rest.next() else {
                return Err(Usage(
                    "--read needs a date, as YYYY-MM-DD. `poptop --days` lists them".into(),
                ));
            };
            Command::Tui {
                day: Some(date(text)?),
            }
        }
        "--check-theme" => match rest.next() {
            Some(name) => Command::CheckTheme(name.to_string()),
            None => return Err(Usage("--check-theme needs a theme name".into())),
        },
        "--schema" => Command::Schema,
        "--config" => Command::Config,
        "--keys" => Command::Keys,
        "--write-config" => Command::WriteConfig,
        "--days" => Command::Days,
        "--once" => Command::Once,
        "--bench" => Command::Bench,
        "--help" | "-h" => Command::Help,
        "--version" | "-V" => Command::Version,
        _ => return Err(Usage(format!("unrecognised option '{first}'\n\n{USAGE}"))),
    };
    match rest.next() {
        None => Ok(command),
        Some(extra) => Err(Usage(format!(
            "{word} does not take `{extra}` — one command at a time"
        ))),
    }
}

/// Whether the terminal is in raw mode on the alternate screen, for [`exit`].
static TERMINAL_TAKEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Whether the terminal has gone away underneath poptop — an ssh session
/// dropped, a terminal emulator killed — rather than been handed back.
///
/// Once it has, nothing may be written to it. Restoring it fails with EIO, and
/// ratatui reports that failure with `eprintln!`, which panics when stderr is
/// the dead terminal too: the hangup that should have been a clean exit ended
/// in an abort (0115).
static TERMINAL_GONE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// `Ok(None)` if `r` failed because the terminal is gone, noting it. EIO is
/// what a read or write of a terminal whose far side has closed returns, on
/// both platforms; ENXIO is the device itself disappearing.
fn while_attached<T>(r: io::Result<T>) -> io::Result<Option<T>> {
    match r {
        Ok(v) => Ok(Some(v)),
        Err(e) if matches!(e.raw_os_error(), Some(5 | 6)) => {
            TERMINAL_GONE.store(true, std::sync::atomic::Ordering::Relaxed);
            Ok(None)
        }
        Err(e) => Err(e),
    }
}

/// Leave the process, giving the terminal back first if poptop has it.
///
/// The one way out other than returning from `main`. `process::exit` runs no
/// destructors, so a path that called it with the screen taken would leave the
/// shell in raw mode on the alternate screen. None does today; this is so none
/// can.
fn exit(code: i32) -> ! {
    if TERMINAL_TAKEN.load(std::sync::atomic::Ordering::Relaxed) {
        ratatui::restore();
    }
    std::process::exit(code)
}

/// Say why, after everything already found, and leave.
fn fail(warnings: &[config::Warning], why: impl std::fmt::Display, code: i32) -> ! {
    flush(warnings);
    eprintln!("poptop: {why}");
    exit(code)
}

/// `--export --follow`: a record an interval, written as it is taken, until
/// something stops it.
///
/// The difference from `--export` without it is not the sampling — that loop
/// is the same one `--once` runs twice — but what happens between samples.
/// Each record is written *and flushed*, because stdout is block-buffered when
/// it is a pipe, and a feed that arrives 8 KB at a time is not a feed: the
/// consumer sees nothing for a minute and then forty samples at once.
///
/// It ends on SIGTERM or SIGHUP, on `until` going by, or when the reader goes
/// away — `poptop --export=json --follow | head -3` is a normal thing to type,
/// and it exits 0, as every other output path does on a closed pipe.
/// One record per sample, written as it is taken and flushed.
///
/// The flush is not a detail: stdout is block-buffered when it is a pipe, so a
/// feed that only wrote would arrive 8 KB at a time — nothing for a minute,
/// then forty samples at once. Shared by the two feeds, the machine's and a
/// day file's, so they are one format and not two.
struct Records {
    json: bool,
    /// The line format's state, and so the header block it has written. One
    /// writer for the whole stream, which is what makes the header appear
    /// once.
    lines: export::Lines,
    out: io::Stdout,
}

impl Records {
    fn new(json: bool) -> Records {
        Records {
            json,
            lines: export::Lines::default(),
            out: io::stdout(),
        }
    }

    /// Whether the reader is still there. A reader that has gone away is not
    /// an error to report — `poptop --export=json --follow | head -3` is a
    /// normal thing to type — but any other write failure is, since a feed
    /// that swallowed a full disk would be a silent hole in somebody's
    /// recording.
    fn write(&mut self, s: &sample::Sample) -> io::Result<bool> {
        use std::io::Write as _;
        let record = if self.json {
            export::sample_json(s)
        } else {
            self.lines.add(s);
            self.lines.take()
        };
        let mut out = self.out.lock();
        match out.write_all(record.as_bytes()).and_then(|()| out.flush()) {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == io::ErrorKind::BrokenPipe => Ok(false),
            Err(e) => Err(e),
        }
    }
}

/// A request to stop: SIGTERM from a service manager or `kill`, SIGHUP from
/// the terminal going away.
fn stop_flag() -> io::Result<Arc<AtomicBool>> {
    let stop = Arc::new(AtomicBool::new(false));
    for sig in [signal_hook::consts::SIGTERM, signal_hook::consts::SIGHUP] {
        signal_hook::flag::register(sig, stop.clone())?;
    }
    Ok(stop)
}

fn feed(
    collector: &mut impl Collector,
    records: &mut Records,
    interval: Duration,
    until: Option<Duration>,
) -> io::Result<()> {
    // Every optional source, for the reason the one-shot path takes them all:
    // a script asking for every metric by name means it.
    let needs = Source::ALL.into_iter().fold(Needs::NONE, |n, s| n.with(s));
    // A priming read. Every rate is a difference, so the first record can only
    // be written once there are two readings to difference — which is why the
    // feed's first line arrives one interval in and not at once.
    collector.sample(needs)?;

    let stop = stop_flag()?;
    let start = Instant::now();
    // A fixed cadence, for the reason the interactive loop keeps one: timing
    // the next sample from the end of the last adds the cost of collecting to
    // every period, and a feed that claims a second and delivers 1.05 is one
    // whose timestamps drift away from the rate it documents.
    let mut next = start + interval;
    loop {
        // In slices, so a signal is noticed within one rather than at the end
        // of an interval that may be an hour.
        while let Some(left) = next.checked_duration_since(Instant::now()) {
            if stop.load(Ordering::Relaxed) || past(start, until, Instant::now()) {
                return Ok(());
            }
            std::thread::sleep(left.min(STOP_CHECK));
        }
        let now = Instant::now();
        if stop.load(Ordering::Relaxed) || past(start, until, now) {
            return Ok(());
        }
        // Rebased on the clock rather than advanced from the last deadline:
        // after a laptop sleeps for an hour, `next += interval` would spend
        // that hour writing eighteen thousand records as fast as it could.
        next = now + interval;

        // One stream for the line format, so the header block is written
        // once — at the top, where a reader that has been there since the
        // start sees it. A reader attaching to a feed already running gets
        // rows and no header; that is what `--export=json` is for, and the
        // guide says so.
        if !records.write(&collector.sample(needs)?)? {
            return Ok(());
        }
    }
}

/// `--export DATE --follow`: a day file read as it is written.
///
/// The other feed samples the machine; this one waits on somebody else's
/// writer, which may be another poptop on the same box or this one with
/// `--log=on`. What is already recorded is written first — the file is a day,
/// and a consumer that attached at noon wanting the morning has no other way
/// to ask for it — and then each entry as it lands.
///
/// Polled rather than watched. The entries arrive a `log-interval` apart, ten
/// minutes by default, and a poll is a `stat` and usually nothing else;
/// inotify and kqueue are two platform APIs, two failure modes and a
/// descriptor per file, to learn a quarter of a second sooner.
fn follow_day(
    dir: &std::path::Path,
    date: log::Date,
    records: &mut Records,
    until: Option<Duration>,
) -> io::Result<()> {
    /// Often enough that a consumer sees an entry as it lands, rarely enough
    /// that a feed left running all week is not a background load.
    const POLL: Duration = Duration::from_millis(250);

    let stop = stop_flag()?;
    let start = Instant::now();
    // Rolls over at midnight only when the day being followed is the live one:
    // somebody following `2026-09-08` asked for that day, and a day that is
    // over does not continue into the next.
    let live = log::date_of(std::time::SystemTime::now()) == Some(date);
    let mut following = log::Follower::open(dir, date, live);
    loop {
        let (samples, notes) = following.poll()?;
        // Straight to stderr, where the warnings from every other path go, and
        // not into the feed: a consumer parsing records must not have to parse
        // prose. Each is said once, which the follower keeps track of.
        flush(&notes.into_iter().map(config::Warning).collect::<Vec<_>>());
        for s in &samples {
            if !records.write(s)? {
                return Ok(());
            }
        }
        // Checked after writing rather than before waiting, so `--for 0s` and
        // a day that is already complete still deliver what is there.
        if stop.load(Ordering::Relaxed) || past(start, until, Instant::now()) {
            return Ok(());
        }
        std::thread::sleep(POLL);
    }
}

/// Whether `until` has gone by, counted from `start`.
fn past(start: Instant, until: Option<Duration>, now: Instant) -> bool {
    until.is_some_and(|d| now.duration_since(start) >= d)
}

/// A recorded day, or the reason there is none as the way out.
///
/// What the reader had to say about the file joins the warnings: a day read
/// with a gap is still a day, and the gap is worth a line.
fn read_day(warnings: &mut Vec<config::Warning>, date: log::Date) -> Vec<sample::Sample> {
    let dir = log::dir().unwrap_or_else(|| fail(warnings, NO_STATE_DIR, 2));
    match log::open_day(&dir, date) {
        Ok((samples, said)) => {
            warnings.extend(said.into_iter().map(config::Warning));
            samples
        }
        Err(why) => fail(warnings, why, 1),
    }
}

/// The platform's collector, and whatever it had to assume about this machine.
///
/// Said once, with the config warnings, rather than folded into every figure
/// that rests on it — an assumption nobody is told about is the same shape as a
/// wrong number. Opened only by the commands that sample.
fn open_collector(warnings: &mut Vec<config::Warning>) -> io::Result<Platform> {
    let mut collector = Platform::new()?;
    warnings.extend(collector.take_notes().into_iter().map(config::Warning));
    Ok(collector)
}

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();

    let mut warnings = Vec::new();
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
    // Before exiting, not after: a config file that could not be *read* is
    // reported here too, and dropping that warning because a flag was also
    // wrong would send the user off to fix the flag and rerun into the same
    // silently-ignored config.
    .unwrap_or_else(|bad| fail(&warnings, bad.as_flag(), 2));
    warnings.extend(file_warnings);

    // Everything but the settings, decided before anything is opened: `--help`
    // must not need a readable `/proc`, and a mistyped command must not cost a
    // collection pass before it is refused.
    let command = command(&positional).unwrap_or_else(|Usage(why)| fail(&warnings, why, 2));

    // Built here rather than beside the App, so that a problem with the user's
    // theme is reported on every path — a colour scheme nobody can read is a
    // fact about their config, and `--once` and `--version` report every other
    // config problem too.
    let (theme, skipped) = theme::Theme::new(settings.palette, settings.tier)
        .with_thresholds(settings.warn, settings.critical)
        .with_overrides(&settings.overrides);

    let day = match command {
        // A report is not interactive, so it is a subcommand rather than a
        // mode: `poptop --report` from cron is how atop is used
        // non-interactively, and a report that needed a terminal could not be.
        Command::Report(date) => {
            // Today unless a day is named. The cron case is a nightly summary
            // of the day that has just happened, and making it spell the date
            // out would make it a date-arithmetic problem in a crontab.
            let date = date
                .or_else(|| log::date_of(std::time::SystemTime::now()))
                .unwrap_or_else(|| fail(&warnings, "this machine's clock is before the epoch", 2));
            let samples = read_day(&mut warnings, date);
            flush(&warnings);
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
        Command::Schema => {
            flush(&warnings);
            out!("{}", export::schema_json());
            return Ok(());
        }
        // What a setting actually did. A config file is read once at startup,
        // above a full-screen monitor that erases whatever it said, so
        // "did my line take effect?" had no answer short of watching the
        // graphs. Each value is printed beside where it came from: the
        // default, the file and its line, `NO_COLOR`, or the flag.
        Command::Config => {
            flush(&warnings);
            let width = config::KEYS.iter().map(|k| k.name.len()).max().unwrap_or(0);
            let shown = settings.resolved();
            let value = shown.iter().map(|(_, v, _)| v.len()).max().unwrap_or(0);
            for (name, v, from) in &shown {
                outln!("{name:<width$}  {v:<value$}  {from}");
            }
            return Ok(());
        }
        // What each key does, after the config file has had its say. The `?`
        // list on screen is the same map; this is the one a reader can see
        // without starting the monitor, and the one to check a `key.` line
        // against.
        Command::Keys => {
            flush(&warnings);
            let width = keys::ACTIONS
                .iter()
                .map(|b| b.name.len())
                .max()
                .unwrap_or(0);
            let bound_keys: Vec<String> = keys::ACTIONS
                .iter()
                .map(|b| settings.keys.keys(b.action))
                .collect();
            let keys_w = bound_keys.iter().map(String::len).max().unwrap_or(0);
            for (bound, shown) in keys::ACTIONS.iter().zip(&bound_keys) {
                // The origin, as `--config` gives it: a binding that did not
                // take effect is the same question as a setting that did not.
                let from = settings.origin(&format!("key.{}", bound.name));
                outln!("{:<width$}  {shown:<keys_w$}  {from}", bound.name);
            }
            return Ok(());
        }
        Command::WriteConfig => {
            let Some(path) = config::path() else {
                fail(&warnings, NO_CONFIG_DIR, 2);
            };
            // Never over an existing one: a config file is written by hand,
            // and this is a scaffold for somebody who has none.
            if path.exists() {
                fail(
                    &warnings,
                    format!("{} already exists; nothing was written", path.display()),
                    2,
                );
            }
            if let Some(dir) = path.parent()
                && let Err(e) = std::fs::create_dir_all(dir)
            {
                fail(
                    &warnings,
                    format!("could not make {}: {e}", dir.display()),
                    1,
                );
            }
            if let Err(e) = std::fs::write(&path, settings.as_config_file()) {
                fail(
                    &warnings,
                    format!("could not write {}: {e}", path.display()),
                    1,
                );
            }
            flush(&warnings);
            outln!("wrote {}", path.display());
            return Ok(());
        }
        Command::Export {
            json,
            day,
            follow: true,
            until,
        } => {
            let mut records = Records::new(json);
            if let Some(date) = day {
                let dir = log::dir().unwrap_or_else(|| fail(&warnings, NO_STATE_DIR, 2));
                flush(&warnings);
                return follow_day(&dir, date, &mut records, until);
            }
            let mut collector = open_collector(&mut warnings)?;
            flush(&warnings);
            return feed(&mut collector, &mut records, settings.interval, until);
        }
        Command::Export {
            json,
            day,
            follow: false,
            ..
        } => {
            // A day, if one was named; otherwise the machine now. Reading
            // history is not a separate feature — it is the same output over a
            // different buffer, which is the whole reason the store carries a
            // schema.
            let samples = match day {
                Some(date) => read_day(&mut warnings, date),
                None => {
                    let mut collector = open_collector(&mut warnings)?;
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
            flush(&warnings);
            // One object per sample for JSON, newline-delimited, so a day is
            // streamable and `head` on it is not a parse error. For the line
            // format one stream, so the header block is written once for the
            // whole day rather than once a sample.
            if json {
                for s in &samples {
                    out!("{}", export::sample_json(s));
                }
            } else {
                out!("{}", export::lines_of(&samples));
            }
            return Ok(());
        }
        Command::Days => {
            flush(&warnings);
            let dir = log::dir().unwrap_or_else(|| fail(&[], NO_STATE_DIR, 2));
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
        Command::Once => {
            let mut collector = open_collector(&mut warnings)?;
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
        Command::Bench => {
            let mut collector = open_collector(&mut warnings)?;
            flush(&warnings);
            return bench(&mut collector);
        }
        Command::Help => {
            flush(&warnings);
            outln!("{USAGE}");
            return Ok(());
        }
        Command::CheckTheme(name) => {
            flush(&warnings);
            return check_theme(&name);
        }
        Command::Version => {
            flush(&warnings);
            outln!("poptop {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        Command::Tui { day } => day,
    };

    // The monitor draws on a terminal and reads keys from one. Without both
    // it used to panic inside ratatui, exit 101, and leave escape codes in
    // whatever stdout was redirected to. Checked before anything is read, so
    // a cron line that forgot `--once` costs nothing and says what it meant.
    use std::io::IsTerminal as _;
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        fail(
            &warnings,
            "the monitor needs a terminal on stdin and stdout; for a script, \
             `--once` prints a sample and `--export=json` every metric",
            2,
        );
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
    let opened = day.map(|date| read_day(&mut warnings, date));
    let mut collector = open_collector(&mut warnings)?;

    let capacity = opened
        .as_ref()
        .map_or(settings.history_len(), |s| s.len().max(1));
    let mut app = App::new(capacity);
    app.interval = settings.interval;
    app.set_smooth(settings.smooth);
    app.theme = theme;
    // Where the theme came from, if it was a file: `R` and the watcher below
    // read it again from here. A built-in cannot change under the program.
    app.theme_file = config::theme_file(&settings.theme);
    app.glyphs = settings.glyphs;
    app.keys = settings.keys.clone();
    app.axis = settings.axis;
    app.density = settings.density;
    app.signals = settings.signals;
    // What the keys would otherwise have to be pressed for on every launch.
    app.view = settings.view;
    app.sort = settings.sort;
    app.set_zoom(settings.zoom);
    app.tree = settings.tree;
    app.group = settings.group;
    app.show_kernel = settings.kernel_threads;
    app.hidden_columns = settings.hide_columns.clone();
    if !settings.io_columns {
        app.toggle_io();
    }
    // The tree and grouping are exclusive, as they are under `t` and `g`: a
    // grouped tree is a tree of things that are not processes.
    if app.tree && app.group != app::Grouping::Off {
        warnings.push(config::Warning(
            "`tree` and `group` cannot both be on; the tree is off".into(),
        ));
        app.tree = false;
    }
    // A sort the starting view cannot show would be an ordering with nothing
    // on screen to explain it.
    if !app.view.sorts().contains(&app.sort) {
        warnings.push(config::Warning(format!(
            "the {} view does not sort by {}; sorting by {} instead",
            app.view.label(),
            app.sort.label().to_lowercase(),
            app.view.default_sort_for(true).label().to_lowercase()
        )));
        app.sort = app.view.default_sort_for(app.io_collected());
    }

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

    // Before the alternate screen: the reply arrives on stdin, and the event
    // loop would eat it. Bounded hard — see `term::background` — because a
    // terminal that ignores the query is the common case, not the exception.
    //
    // Skipped entirely when the surfaces are switched off, so `surface = off`
    // costs nothing at all, not even the wait.
    if settings.surfaces {
        let base = term::background(std::time::Duration::from_millis(150));
        app.theme = app.theme.with_surfaces(base);
    }

    let mut terminal = ratatui::try_init().unwrap_or_else(|e| {
        // Half an initialisation is still a changed terminal.
        ratatui::restore();
        fail(&warnings, format!("could not take the terminal: {e}"), 1)
    });
    TERMINAL_TAKEN.store(true, std::sync::atomic::Ordering::Relaxed);
    show_cursor_on_panic();
    // Reported by every terminal poptop is likely to run in, and ignored until
    // now. A menu bar you can see and cannot click reads as a bar that is
    // broken, so the menu made this the next thing rather than a nicety.
    //
    // The cost is real and is stated in `--help`: while poptop has the mouse,
    // dragging no longer selects text for copying. Every terminal worth the
    // name restores that under Shift, and a reader who would rather have the
    // selection can say `mouse = off`.
    let mouse = settings.mouse;
    if mouse {
        let _ = crossterm::execute!(io::stdout(), crossterm::event::EnableMouseCapture);
    }
    let mut said = Vec::new();
    let result = run(
        &mut terminal,
        &mut app,
        &mut collector,
        logging.as_ref(),
        replaying,
        &mut said,
    );
    if mouse {
        // Before the screen is restored, so a terminal left in mouse-reporting
        // mode is not what somebody has to work out after poptop exits.
        //
        // Attempted whatever became of the terminal: if it has gone this fails
        // and is ignored, which is the same answer the restore below reaches
        // the long way round.
        let _ = crossterm::execute!(io::stdout(), crossterm::event::DisableMouseCapture);
    }
    // Given back if it is still there. If it has gone, restoring it fails and
    // ratatui says so with `eprintln!`, which panics on a dead stderr — and so
    // would the `Terminal`'s drop, which shows the cursor it hid. Neither is
    // attempted: the terminal is not anyone's to give back any more.
    //
    // Found out here as well as in the loop. A hangup signal and a dead
    // terminal arrive together, and when the signal wins the race the loop
    // ends normally without ever reading the EIO — so the restore is what
    // discovers it, and must not report it on the terminal it failed to reach.
    let gone = TERMINAL_GONE.load(std::sync::atomic::Ordering::Relaxed)
        || while_attached(ratatui::try_restore())
            .map(|r| r.is_none())
            .unwrap_or_else(|e| {
                use std::io::Write as _;
                let _ = writeln!(io::stderr(), "poptop: could not restore the terminal: {e}");
                false
            });
    if gone {
        std::mem::forget(terminal);
    }
    TERMINAL_TAKEN.store(false, std::sync::atomic::Ordering::Relaxed);
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

/// Make a panic leave the cursor visible, as well as the screen restored.
///
/// `ratatui::init` installs a hook that leaves raw mode and the alternate
/// screen and then prints the panic. The cursor it hid comes back only when the
/// `Terminal` is dropped, which unwinding does and an abort would not. So the
/// cursor is shown here first, in the hook, whatever happens after it; then
/// ratatui's hook restores the rest and prints the message where it can be read.
fn show_cursor_on_panic() {
    let restore = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = crossterm::execute!(io::stdout(), crossterm::cursor::Show);
        restore(info);
    }));
}

/// Time collection passes, for development.
fn bench(collector: &mut impl Collector) -> io::Result<()> {
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
            (false, ..) if needs.asked(Source::Pss) => "pss only (one extra read a process)      ",
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
    Ok(())
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
            Err(why) => fail(&warnings, why, 2),
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
        exit(code);
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

    // SIGTERM is how a service manager or `kill` asks a program to stop, and
    // SIGHUP is the terminal going away. Either used to end the process on
    // the spot, leaving the shell in raw mode on the alternate screen and the
    // history unsaved. Now each is a request to quit, answered like `q`.
    let stop = Arc::new(AtomicBool::new(false));
    for sig in [signal_hook::consts::SIGTERM, signal_hook::consts::SIGHUP] {
        signal_hook::flag::register(sig, stop.clone())?;
    }

    // How the pty tests prove a panic gives the terminal back. Read only by a
    // debug build, so no release binary has a way to be told to die.
    #[cfg(debug_assertions)]
    let mut forced = std::env::var_os("POPTOP_PANIC_AFTER_FIRST_FRAME").is_some();
    loop {
        // A terminal that has gone away is a request to quit, like the
        // hangup signal that comes with it — not an error to report on it.
        if while_attached(terminal.draw(|f| ui::draw(f, app)))?.is_none() {
            return Ok(());
        }
        #[cfg(debug_assertions)]
        if std::mem::take(&mut forced) {
            panic!("forced by POPTOP_PANIC_AFTER_FIRST_FRAME");
        }

        // Wait for a key until the next sample is due, in slices short enough
        // to notice a stop request: the signal handler only sets a flag, and
        // the poll underneath is restarted rather than interrupted by it.
        let key = loop {
            if stop.load(Ordering::Relaxed) {
                return Ok(());
            }
            let left = next_sample.saturating_duration_since(Instant::now());
            let Some(ready) = while_attached(event::poll(left.min(STOP_CHECK)))? else {
                return Ok(());
            };
            if ready {
                let Some(event) = while_attached(event::read())? else {
                    return Ok(());
                };
                break match event {
                    Event::Key(k) if k.kind == KeyEventKind::Press => Some(k),
                    // Answered here rather than carried out of the loop like a
                    // key. A click is a question about the layout the last
                    // frame drew, and the frame is still on screen — the
                    // redraw at the top of the loop is what shows the answer.
                    Event::Mouse(m) => {
                        handle_mouse(app, m, terminal.get_frame().area());
                        None
                    }
                    _ => None,
                };
            }
            if left <= STOP_CHECK {
                break None;
            }
        };
        if let Some(key) = key {
            for k in rejoin(key, next_key) {
                handle_key(app, k.code, k.modifiers);
            }
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
            // A theme file that has been written since it was read. One stat
            // a sample, against the several hundred reads a sample already
            // makes, so a reader editing colours sees them without restarting
            // or pressing anything.
            if let Some((name, was)) = app.theme_file.clone()
                && config::theme_file(&name).is_some_and(|(_, now)| now != was)
            {
                command::reload_theme(app);
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

/// How often a wait for input looks at the stop flag.
const STOP_CHECK: Duration = Duration::from_millis(100);

/// How long a lone Esc waits for the rest of an escape sequence.
///
/// Long enough for the next read over a slow link or a busy machine, short
/// enough that a real Esc, which backs out or quits, is not felt to lag. Vim's
/// `ttimeoutlen` in `defaults.vim`. 50ms, Neovim's, was not enough on a loaded
/// CI runner: the second half arrived after it and the arrow quit poptop.
const ESC_WAIT: Duration = Duration::from_millis(100);

/// The next key press, if one arrives within [`ESC_WAIT`].
fn next_key() -> Option<KeyEvent> {
    loop {
        if !event::poll(ESC_WAIT).ok()? {
            return None;
        }
        match event::read().ok()? {
            Event::Key(k) if k.kind == KeyEventKind::Press => return Some(k),
            Event::Key(_) => continue,
            _ => return None,
        }
    }
}

/// A key press, with an escape sequence that arrived split across two reads
/// put back together.
///
/// The terminal sends Down as three bytes, `ESC [ B`. When they arrive in one
/// read, crossterm sees Down. Over a slow link they can arrive as `ESC`, then
/// `[B`, and crossterm, seeing an `ESC` with nothing after it, reports Esc,
/// then `[`, then `B`. Esc backs out of whatever is open, and with nothing
/// open it quits. So a lone Esc asks for what follows, and a
/// `[` or `O` straight after it is read as the rest of a sequence. A sequence
/// this does not know is dropped whole, rather than half of it being typed.
fn rejoin(first: KeyEvent, mut next: impl FnMut() -> Option<KeyEvent>) -> Vec<KeyEvent> {
    let plain = |k: &KeyEvent| k.modifiers.difference(KeyModifiers::SHIFT).is_empty();
    if first.code != KeyCode::Esc || !first.modifiers.is_empty() {
        return vec![first];
    }
    let Some(second) = next() else {
        return vec![first];
    };
    let intro = match second.code {
        KeyCode::Char(c @ ('[' | 'O')) if plain(&second) => c,
        _ => return vec![first, second],
    };
    // Parameters, then one final byte: `A`, `5~`, `1;2C`.
    let mut params = String::new();
    let final_byte = loop {
        match next() {
            Some(KeyEvent {
                code: KeyCode::Char(c),
                ..
            }) if params.len() < 8 => {
                if matches!(c, '0'..='9' | ';') {
                    params.push(c);
                } else {
                    break c;
                }
            }
            // Cut off, or not a sequence after all: nothing of it is a key.
            _ => return Vec::new(),
        }
    };
    let modifiers = match params.split_once(';').map(|(_, m)| m) {
        Some("2") => KeyModifiers::SHIFT,
        Some("3") => KeyModifiers::ALT,
        Some("5") => KeyModifiers::CONTROL,
        _ => KeyModifiers::NONE,
    };
    let code = match (intro, params.split(';').next().unwrap_or(""), final_byte) {
        (_, _, 'A') => KeyCode::Up,
        (_, _, 'B') => KeyCode::Down,
        (_, _, 'C') => KeyCode::Right,
        (_, _, 'D') => KeyCode::Left,
        (_, _, 'H') | ('[', "1" | "7", '~') => KeyCode::Home,
        (_, _, 'F') | ('[', "4" | "8", '~') => KeyCode::End,
        ('[', "5", '~') => KeyCode::PageUp,
        ('[', "6", '~') => KeyCode::PageDown,
        _ => return Vec::new(),
    };
    vec![KeyEvent::new(code, modifiers)]
}

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
    // The key list is modal and any key puts it away — without also being
    // acted on, so `q` closes it rather than quitting behind it. Ahead of the
    // menu because it is the shallower surface: it holds no selection and
    // nothing is lost by dismissing it, so a reader who opened it and then
    // reached for `F10` gets the bar on the next press rather than nothing.
    if app.show_help {
        app.show_help = false;
        return;
    }
    // The bar, before every mode below it. F10 is the convention older than
    // any of them; Alt-letter jumps straight to a title, as it does everywhere
    // else a menu bar exists.
    if app.menu.is_open() {
        menu_key(app, code, mods);
        return;
    }
    // Not while a box is up. The filter and the jump take every printable key,
    // and the signal confirmation takes every key at all — a menu opening over
    // one of them would be two modes claiming the keyboard, and the one the
    // reader is looking at would lose.
    let modal = app.editing_filter || app.editing_jump || app.pending.is_some();
    if code == KeyCode::F(10) && !modal {
        app.menu.toggle();
        return;
    }
    if let KeyCode::Char(c) = code
        && mods.contains(KeyModifiers::ALT)
        && !modal
        && let Some(at) = menu::MenuState::title_for(c, &menu::bar())
    {
        app.menu.open = Some(at);
        app.menu.item = 0;
        return;
    }
    // A chord typed into a text box is not text. Ctrl-U or Alt-B arrive as
    // the letter with a modifier, and were appended as `u` and `b`.
    let typed =
        |c: char| (!mods.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)).then_some(c);
    if app.editing_filter {
        match code {
            // Enter keeps what was typed; Escape puts back what was there
            // before. Both used to commit, so Escape was a second Enter — and
            // a key that every other program uses to undo is the wrong one to
            // spend on "finish".
            KeyCode::Enter => app.editing_filter = false,
            KeyCode::Esc => {
                app.filter = std::mem::take(&mut app.filter_before);
                app.editing_filter = false;
            }
            KeyCode::Backspace => {
                app.filter.pop();
            }
            KeyCode::Char(c) => app.filter.extend(typed(c)),
            _ => {}
        }
        return;
    }
    // A pending signal takes every key: a confirmation that let other keys
    // through is one somebody dismisses by reflex while meaning to scroll.
    if app.pending.is_some() {
        // Bare `y`, with no modifier. Crossterm reports `Ctrl-Y` as `Char('y')`
        // with `CONTROL`, and every other key here cancels — so ignoring the
        // modifier made one accidental chord the *only* one that sends a
        // signal, which is exactly the wrong asymmetry.
        let plain = mods.difference(KeyModifiers::SHIFT).is_empty();
        app.confirm_signal(plain && code == KeyCode::Char('y'));
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
            KeyCode::Char(c) => app.jump.extend(typed(c)),
            _ => {}
        }
        return;
    }

    // The answer to the last jump belongs to the last jump. Left standing it
    // described nothing on screen as soon as the reader scrubbed away, and it
    // kept the key hints hidden for the rest of the run.
    // Which action this key asks for, if any. Everything below is written in
    // terms of actions rather than keys, so a config file that moves a key
    // moves what it does with it — and the menu bar and the mouse reach the
    // same list, so one verb cannot mean two things depending on how it was
    // asked for.
    let Some(action) = app.keys.action(code, mods) else {
        // Not bound to anything: the notes still clear, as they do for any
        // key that is not the one that put them there.
        app.jump_note = None;
        app.signal_note = None;
        return;
    };
    if action != Action::BeginJump {
        app.jump_note = None;
    }
    if !matches!(action, Action::Signal(_)) {
        app.signal_note = None;
    }
    // Shift crosses a long buffer without holding the key down. It multiplies
    // whatever the binding asked for rather than naming a step of its own, so
    // a reader who has bound an arrow to five samples gets fifty.
    let action = match (action, mods.contains(KeyModifiers::SHIFT)) {
        (Action::Scrub(n), true) => Action::Scrub(n * 10),
        (Action::Select(n), true) => Action::Select(n * 10),
        (a, _) => a,
    };
    action.apply(app);
}

/// What a click, drag or wheel turn means.
///
/// Pure: it takes where the pointer is and returns an action, so the mapping
/// can be tested without a terminal. `handle_mouse` is the two lines that apply
/// the result and the menu bookkeeping that is not an action.
pub fn handle_mouse(app: &mut App, ev: event::MouseEvent, area: ratatui::layout::Rect) {
    use crossterm::event::{MouseButton, MouseEventKind};

    let p = ui::panels(app, area);
    let (x, y) = (ev.column, ev.row);
    let inside =
        |r: ratatui::layout::Rect| x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height;

    match ev.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            // An open dropdown takes the click before anything under it, which
            // is what "over" means. Without this a menu item and the table row
            // it is drawn on top of both answer, and the table wins because it
            // is checked first.
            if app.menu.is_open() {
                if let Some(rect) = ui::dropdown_rect(app, area)
                    && inside(rect)
                {
                    // The frame is not an item: the top and bottom rows are the
                    // border, and clicking a border should do nothing rather
                    // than run whatever is nearest.
                    // Plus whatever the list is scrolled by, from the same
                    // function the drawing asks — an offset worked out twice
                    // would put the click on a different item from the one
                    // under the pointer.
                    let row =
                        y.saturating_sub(rect.y + 1) as usize + ui::dropdown_offset(app, area);
                    let titles = menu::bar();
                    if y > rect.y
                        && y + 1 < rect.y + rect.height
                        && let Some(t) = app.menu.open.and_then(|i| titles.get(i))
                        && let Some(action) = t.items.get(row).and_then(|i| i.action())
                    {
                        app.menu.close();
                        action.apply(app);
                    }
                    return;
                }
                // On the bar: switch titles. Anywhere else: dismiss. A menu
                // that stayed open when you clicked past it is one you have to
                // close twice.
                match title_at(x) {
                    Some(at) if app.menu.open == Some(at) => app.menu.close(),
                    Some(at) => {
                        app.menu.open = Some(at);
                        app.menu.item = 0;
                    }
                    None => app.menu.close(),
                }
                return;
            }
            if let Some(at) = title_at(x).filter(|_| inside(p.menu)) {
                app.menu.open = Some(at);
                app.menu.item = 0;
                return;
            }
            if inside(p.tabs)
                && let Some(v) = tab_at(x)
            {
                Action::SetView(v).apply(app);
                return;
            }
            if inside(p.timeline) {
                if let Some(a) = scrub_to(app, p.timeline, x) {
                    a.apply(app);
                }
                return;
            }
            if inside(p.table) {
                // The column headers sort; the rows below select. Same row the
                // caret is drawn on, which is the point of putting it there.
                if y == ui::table_header_y(p.table) {
                    if let Some(s) = ui::sort_at(app, p.table, x) {
                        app.sort = s;
                    }
                } else if let Some(a) = row_at(p.table, y) {
                    a.apply(app);
                }
            }
        }
        // Dragging the timeline scrubs continuously, which is the one gesture
        // here that a keyboard cannot express at all.
        MouseEventKind::Drag(MouseButton::Left) if inside(p.timeline) => {
            if let Some(a) = scrub_to(app, p.timeline, x) {
                a.apply(app);
            }
        }
        MouseEventKind::ScrollDown | MouseEventKind::ScrollUp => {
            let down = ev.kind == MouseEventKind::ScrollDown;
            // Over the graph the wheel moves time; over the table it moves the
            // selection. Both are the thing under the pointer.
            let action = if inside(p.timeline) {
                Action::Scrub(if down { 1 } else { -1 })
            } else {
                Action::Select(if down { 1 } else { -1 })
            };
            action.apply(app);
        }
        _ => {}
    }
}

/// Which tab the column `x` falls in, if any.
fn tab_at(x: u16) -> Option<crate::app::View> {
    crate::app::View::ALL
        .into_iter()
        .enumerate()
        .find_map(|(i, v)| {
            let at = ui::tab_column(i) as u16;
            (x >= at && x < at + ui::tab_width(v) as u16).then_some(v)
        })
}

/// Which menu title the column `x` falls in, if any.
fn title_at(x: u16) -> Option<usize> {
    let titles = menu::bar();
    let mut at = 1u16;
    for (i, t) in titles.iter().enumerate() {
        let w = t.name.chars().count() as u16 + 2;
        if x >= at && x < at + w {
            return Some(i);
        }
        at += w;
    }
    None
}

/// The table row under `y`, as an action.
fn row_at(table: ratatui::layout::Rect, y: u16) -> Option<Action> {
    // One past the column headers, wherever they turned out to be — which
    // depends on whether the summary strip took a row.
    let first = ui::table_header_y(table) + 1;
    (y >= first).then(|| Action::SelectRow((y - first) as usize))
}

/// The sample under column `x` of the timeline.
fn scrub_to(app: &App, timeline: ratatui::layout::Rect, x: u16) -> Option<Action> {
    let (start, shown, zoom) = ui::shown_window(app, timeline);
    if shown == 0 {
        return None;
    }
    let gutter = ui::GUTTER_W as u16;
    let cell = x.checked_sub(timeline.x + gutter)? as usize;
    let spc = app.glyphs.samples_per_cell();
    // The same two packings the drawing uses, run backwards: a cell is `spc`
    // slots and a slot is `zoom` samples.
    let at = start + (cell * spc * zoom).min(shown - 1);
    Some(Action::ScrubTo(at))
}

/// Keys while a dropdown is open.
///
/// The menu owns every key here. A bar that let unrelated keys through would be
/// one you dismiss by reflex while meaning to scroll — the same reasoning as
/// the signal confirmation.
fn menu_key(app: &mut App, code: KeyCode, mods: KeyModifiers) {
    let titles = menu::bar();
    match code {
        KeyCode::Esc | KeyCode::F(10) => app.menu.close(),
        KeyCode::Left => app.menu.move_title(-1, titles.len()),
        KeyCode::Right => app.menu.move_title(1, titles.len()),
        KeyCode::Up | KeyCode::Down => {
            let delta = if code == KeyCode::Up { -1 } else { 1 };
            if let Some(t) = app.menu.open.and_then(|i| titles.get(i)) {
                app.menu.move_item(delta, &t.items);
            }
        }
        KeyCode::Home => app.menu.item = 0,
        KeyCode::Enter => {
            // Closed before the action runs, not after. Several of these open a
            // box of their own — the filter, the jump, the signal confirmation
            // — and a dropdown still on screen over one of them is two modes
            // claiming the keyboard.
            let chosen = menu::chosen(app.menu, &titles);
            app.menu.close();
            if let Some(action) = chosen {
                action.apply(app);
            }
        }
        KeyCode::Char(c) if mods.contains(KeyModifiers::ALT) => {
            if let Some(at) = menu::MenuState::title_for(c, &titles) {
                app.menu.open = Some(at);
                app.menu.item = 0;
            }
        }
        _ => {}
    }
}

/// The action a key asks for, or `None` if it asks for nothing.
///
/// Separated from `handle_key` so the mapping is a table rather than a pile of
/// statements: every one of these is also a menu item, and the two have to be
/// the same command or they will drift. See `command.rs`.
pub fn action_for(code: KeyCode, mods: KeyModifiers) -> Option<Action> {
    // Shift crosses a long buffer without holding the key down.
    let step = if mods.contains(KeyModifiers::SHIFT) {
        10
    } else {
        1
    };
    Some(match code {
        KeyCode::Char('q') => Action::Quit,
        // Back out one level, as Esc does from the filter and the jump box: a
        // selection first, then the program. It was a second `q`, and a key
        // every other program uses to undo is the wrong one to spend on
        // "quit" while there is something to let go of.
        KeyCode::Esc => Action::Back,
        KeyCode::Char('c') if mods.contains(KeyModifiers::CONTROL) => Action::Quit,
        // The whole list, for the keys the footer has no room to hint at and
        // the menu bar puts one dropdown away.
        KeyCode::Char('?') => Action::ShowKeys,

        KeyCode::Left | KeyCode::Char('h') => Action::Scrub(-step),
        KeyCode::Right | KeyCode::Char('l') => Action::Scrub(step),
        KeyCode::Char(' ') => Action::ToggleLive,
        KeyCode::Home => Action::GotoOldest,
        KeyCode::End => Action::GotoLive,

        KeyCode::Up | KeyCode::Char('k') => Action::Select(-1),
        KeyCode::Down | KeyCode::Char('j') => Action::Select(1),
        KeyCode::PageUp => Action::Select(-10),
        KeyCode::PageDown => Action::Select(10),

        // '=' so zooming in does not require Shift on most layouts.
        KeyCode::Char('+' | '=') => Action::ZoomIn,
        KeyCode::Char('-' | '_') => Action::ZoomOut,

        // The selection is of a process, so re-sorting moves the row under it
        // and keeps it selected.
        KeyCode::Char('s') => Action::NextSort,
        // Accept the suggestion. Never applied on its own: a table that
        // reorders itself under the reader is worse than one that does not.
        KeyCode::Char('S') => Action::AcceptSuggestedSort,
        // Column sets, over the same rows and the same renderer. `v` because
        // atop spends seven keys on this and poptop has three views and few
        // free letters.
        KeyCode::Char('v') => Action::NextView,
        // The tab strip. Not `←`/`→`, which scrub time and must keep doing so:
        // the timeline is the thing poptop has that Activity Monitor does not,
        // and its keys come first.
        KeyCode::Tab => Action::NextView,
        KeyCode::BackTab => Action::PrevView,
        KeyCode::Char(c @ '1'..='9') => {
            let at = c as usize - '1' as usize;
            Action::SetView(*crate::app::View::ALL.get(at)?)
        }
        // atop's key for the same thing.
        KeyCode::Char('y') => Action::ToggleThreads,
        // atop shows cgroups on G. C here, because g is already grouping and
        // G is not free either.
        KeyCode::Char('C') => Action::ToggleCgroups,
        KeyCode::Char('K') => Action::ToggleKernel,
        KeyCode::Char('t') => Action::ToggleTree,
        KeyCode::Char('d') => Action::ToggleDetail,
        // `⏎` on a row, which is what a pointer would do to it.
        KeyCode::Enter => Action::ToggleInspect,
        // A cycle: off, by name, by user, by container — atop's `p`, `u` and
        // `j` on one key.
        KeyCode::Char('g') => Action::NextGrouping,
        KeyCode::Char('/') => Action::BeginFilter,
        // `x`, not `k`: `k` is already "select the previous process", the vim
        // binding beside `j`, and a key that quietly stopped moving the
        // selection would be an unforgivable trade here.
        KeyCode::Char('x') => Action::Signal(crate::signal::Signal::Term),
        KeyCode::Char('X') => Action::Signal(crate::signal::Signal::Kill),
        // `b` for the beginning of a moment, which is atop's `-b`. Not `j`:
        // that is already "select the next process".
        KeyCode::Char('b') => Action::BeginJump,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(line: &str) -> Result<Command, Usage> {
        let args: Vec<String> = line.split_whitespace().map(String::from).collect();
        command(&args)
    }

    fn date(s: &str) -> Option<log::Date> {
        Some(log::Date::parse(s).unwrap())
    }

    #[test]
    fn every_command_line_that_runs() {
        let rows: &[(&str, Command)] = &[
            ("", Command::Tui { day: None }),
            (
                "--read 2026-09-08",
                Command::Tui {
                    day: date("2026-09-08"),
                },
            ),
            (
                "--read=2026-09-08",
                Command::Tui {
                    day: date("2026-09-08"),
                },
            ),
            ("--once", Command::Once),
            ("--report", Command::Report(None)),
            ("--report=", Command::Report(None)),
            ("--report 2026-09-08", Command::Report(date("2026-09-08"))),
            ("--report=2026-09-08", Command::Report(date("2026-09-08"))),
            ("--schema", Command::Schema),
            (
                "--export json",
                Command::Export {
                    json: true,
                    day: None,
                    follow: false,
                    until: None,
                },
            ),
            (
                "--export=line",
                Command::Export {
                    json: false,
                    day: None,
                    follow: false,
                    until: None,
                },
            ),
            (
                "--export json 2026-09-08",
                Command::Export {
                    json: true,
                    day: date("2026-09-08"),
                    follow: false,
                    until: None,
                },
            ),
            (
                "--export=line 2026-09-08",
                Command::Export {
                    json: false,
                    day: date("2026-09-08"),
                    follow: false,
                    until: None,
                },
            ),
            (
                "--export json --follow",
                Command::Export {
                    json: true,
                    day: None,
                    follow: true,
                    until: None,
                },
            ),
            (
                "--export json 2026-09-08 --follow",
                Command::Export {
                    json: true,
                    day: date("2026-09-08"),
                    follow: true,
                    until: None,
                },
            ),
            (
                "--export=line --follow --for 30s",
                Command::Export {
                    json: false,
                    day: None,
                    follow: true,
                    until: Some(Duration::from_secs(30)),
                },
            ),
            (
                "--export json --for=5m --follow",
                Command::Export {
                    json: true,
                    day: None,
                    follow: true,
                    until: Some(Duration::from_secs(300)),
                },
            ),
            ("--days", Command::Days),
            ("--bench", Command::Bench),
            ("--help", Command::Help),
            ("-h", Command::Help),
            ("--version", Command::Version),
            ("-V", Command::Version),
            ("--check-theme safe", Command::CheckTheme("safe".into())),
            ("--check-theme=mine", Command::CheckTheme("mine".into())),
        ];
        for (line, want) in rows {
            assert_eq!(run(line).as_ref(), Ok(want), "`{line}`");
        }
    }

    #[test]
    fn every_command_line_that_is_refused_says_why() {
        // Each is an exit 2 with this text after `poptop: `. The runtime
        // failures — no state directory, a day that cannot be read — need a
        // filesystem and are not here.
        let rows: &[(&str, &str)] = &[
            ("--read", "--read needs a date"),
            ("--read=", "--read needs a date"),
            ("--read yesterday", "`yesterday` is not a date"),
            ("--read 2026-02-30", "`2026-02-30` is not a date"),
            ("--report 08/09/2026", "`08/09/2026` is not a date"),
            ("--export", "--export takes `json` or `line`"),
            ("--export csv", "--export takes `json` or `line`"),
            ("--export json today", "`today` is not a date"),
            ("--export json --for 30s", "--for is --follow's"),
            ("--export json --follow --for", "--for needs a span"),
            (
                "--export json --follow --for=soon",
                "--for `soon`: expected a span",
            ),
            ("--export json --tail", "--export does not take `--tail`"),
            ("--check-theme", "--check-theme needs a theme name"),
            ("--frobnicate", "unrecognised option '--frobnicate'"),
            ("--store on", "unrecognised option '--store'"),
            ("top", "unrecognised option 'top'"),
            // One command, and nothing left over. These were run with the
            // rest silently dropped.
            ("--read 2026-09-08 --once", "--read does not take `--once`"),
            ("--once --report", "--once does not take `--report`"),
            ("--days junk", "--days does not take `junk`"),
            ("--schema=x", "--schema does not take `x`"),
            (
                "--export json 2026-09-08 extra",
                "--export does not take `extra`",
            ),
            ("--report 2026-09-08 2026-09-09", "--report does not take"),
            ("--check-theme a b", "--check-theme does not take `b`"),
        ];
        for (line, want) in rows {
            match run(line) {
                Err(Usage(why)) => assert!(why.starts_with(want), "`{line}`: {why}"),
                Ok(c) => panic!("`{line}` ran as {c:?}"),
            }
        }
    }

    #[test]
    fn an_unrecognised_option_is_followed_by_the_usage() {
        let Err(Usage(why)) = run("--frobnicate") else {
            panic!("accepted");
        };
        assert!(why.ends_with(USAGE));
    }

    fn app() -> App {
        let mut app = App::new(60);
        app.push(sample::Sample::unknown());
        app
    }

    fn keys(app: &mut App, codes: &[KeyCode]) {
        for &c in codes {
            handle_key(app, c, KeyModifiers::NONE);
        }
    }

    #[test]
    fn a_rebound_key_does_what_the_file_said() {
        // End to end: the config file's binding, through `resolve`, into the
        // app, and the key press that follows it.
        let file = "key.quit = Q\nkey.filter = f\nkey.tree = /\n";
        let (settings, _, warnings) = config::resolve(
            config::Settings::detect(),
            config::Sources {
                file: Some(("conf", file)),
                no_color: false,
                themes: &config::read_theme,
            },
            &[],
        )
        .expect("the file is not fatal");
        assert!(warnings.is_empty(), "{warnings:?}");

        let mut a = app();
        a.keys = settings.keys.clone();
        // The keys those actions used to have do nothing now. Not `/`: it is
        // the tree in this map, and pressing it here would toggle it.
        keys(&mut a, &[KeyCode::Char('q'), KeyCode::Char('t')]);
        assert!(
            !a.should_quit && !a.editing_filter && !a.tree,
            "an old binding still fired"
        );
        // `/` is the tree, `f` is the filter, `Q` quits.
        keys(&mut a, &[KeyCode::Char('/')]);
        assert!(a.tree, "`/` was not the tree");
        keys(&mut a, &[KeyCode::Char('f')]);
        assert!(a.editing_filter, "`f` was not the filter");
        keys(&mut a, &[KeyCode::Esc]);
        handle_key(&mut a, KeyCode::Char('Q'), KeyModifiers::SHIFT);
        assert!(a.should_quit, "`Q` did not quit");
    }

    #[test]
    fn reloading_a_theme_says_what_happened() {
        // A built-in has no file, and says so rather than appearing to do
        // nothing when the key is pressed.
        let mut a = app();
        a.theme_file = None;
        keys(&mut a, &[KeyCode::Char('R')]);
        assert_eq!(
            a.theme_note.as_deref(),
            Some("the theme is built in; there is no file to read")
        );

        // A named theme whose file is not there keeps the colours on screen
        // and says why.
        let before = a.theme.ok;
        a.theme_file = Some(("nosuch".into(), std::time::SystemTime::UNIX_EPOCH));
        keys(&mut a, &[KeyCode::Char('R')]);
        assert!(
            a.theme_note
                .as_deref()
                .is_some_and(|n| n.starts_with("nosuch.theme was not read")),
            "{:?}",
            a.theme_note
        );
        assert_eq!(a.theme.ok, before, "a failed read changed the colours");
    }

    #[test]
    fn a_text_box_takes_text_and_not_chords() {
        let mut a = app();
        keys(
            &mut a,
            &[KeyCode::Char('/'), KeyCode::Char('s'), KeyCode::Char('h')],
        );
        handle_key(&mut a, KeyCode::Char('u'), KeyModifiers::CONTROL);
        handle_key(&mut a, KeyCode::Char('b'), KeyModifiers::ALT);
        // Shift is how capitals arrive, and they are text.
        handle_key(&mut a, KeyCode::Char('D'), KeyModifiers::SHIFT);
        assert_eq!(a.filter, "shD");
        assert!(a.editing_filter);

        let mut a = app();
        keys(
            &mut a,
            &[KeyCode::Char('b'), KeyCode::Char('-'), KeyCode::Char('2')],
        );
        handle_key(&mut a, KeyCode::Char('w'), KeyModifiers::CONTROL);
        assert_eq!(a.jump, "-2");
    }

    #[test]
    fn every_mode_has_a_way_out_that_is_not_quitting() {
        // The filter: both keys leave the box, which is what this test is
        // about — but they are not the same key. Enter keeps what was typed
        // and Escape puts back what was there before, because a key every
        // other program uses to undo is the wrong one to spend on "finish".
        let mut a = app();
        keys(
            &mut a,
            &[KeyCode::Char('/'), KeyCode::Char('x'), KeyCode::Enter],
        );
        assert!(!a.editing_filter && !a.should_quit);
        assert_eq!(a.filter, "x");

        // And `/` on an existing filter keeps it, so narrowing a narrowed list
        // does not mean retyping the first query. Clearing is its own command.
        keys(
            &mut a,
            &[KeyCode::Char('/'), KeyCode::Char('y'), KeyCode::Esc],
        );
        assert!(!a.editing_filter && !a.should_quit);
        assert_eq!(a.filter, "x", "Escape kept the edit instead of undoing it");
        Action::ClearFilter.apply(&mut a);
        assert_eq!(a.filter, "");
        // The jump box: Esc cancels without moving.
        let mut a = app();
        keys(
            &mut a,
            &[KeyCode::Char('b'), KeyCode::Char('1'), KeyCode::Esc],
        );
        assert!(!a.editing_jump && !a.should_quit);
        // Views that toggle come back with the same key.
        for k in ['t', 'd', 'K', 'i', 'y', 'C'] {
            let mut a = app();
            let before = (a.tree, a.detail, a.show_kernel, a.show_io);
            keys(&mut a, &[KeyCode::Char(k), KeyCode::Char(k)]);
            assert_eq!(
                (a.tree, a.detail, a.show_kernel, a.show_io),
                before,
                "`{k}` twice"
            );
        }
        // Grouping cycles back to off, and never coexists with the tree.
        let mut a = app();
        keys(&mut a, &[KeyCode::Char('t'), KeyCode::Char('g')]);
        assert!(!a.tree && a.group != app::Grouping::Off);
        for _ in 0..8 {
            keys(&mut a, &[KeyCode::Char('g')]);
            if a.group == app::Grouping::Off {
                break;
            }
        }
        assert_eq!(a.group, app::Grouping::Off);
        keys(&mut a, &[KeyCode::Char('g'), KeyCode::Char('t')]);
        assert!(a.tree && a.group == app::Grouping::Off);
        // Only then does Esc quit.
        keys(&mut a, &[KeyCode::Esc]);
        assert!(a.should_quit);
    }

    #[test]
    fn a_selection_is_a_mode_and_esc_leaves_it_before_it_quits() {
        // 0102: the arrow keys set a selection and nothing cleared it, so once
        // a process had been picked poptop followed it for the rest of the run
        // — `d` showed its history rather than the machine's, and after it
        // exited the table kept saying it was gone. Esc backs out one level, as
        // it does from the filter and the jump box: first the selection, and
        // what hangs off it; then the program. `q` still quits at once.
        let mut a = App::new(60);
        a.push(store::tests_support::big_sample(1.0, 3));
        keys(&mut a, &[KeyCode::Down, KeyCode::Char('d')]);
        assert!(a.selected.is_some() && a.detail);
        keys(&mut a, &[KeyCode::Esc]);
        assert!(a.selected.is_none(), "Esc did not let go of the process");
        assert!(!a.detail, "its history outlived the selection");
        assert!(!a.should_quit, "Esc quit with something selected");
        keys(&mut a, &[KeyCode::Esc]);
        assert!(a.should_quit, "Esc with nothing selected no longer quits");

        let mut a = App::new(60);
        a.push(store::tests_support::big_sample(1.0, 3));
        keys(&mut a, &[KeyCode::Down, KeyCode::Char('q')]);
        assert!(a.should_quit, "q waited for the selection to be cleared");
    }

    #[test]
    fn a_signal_prompt_is_answered_by_one_key_and_only_y_sends() {
        let mut a = app();
        a.signals = true;
        let mut s = sample::Sample::unknown();
        s.procs = vec![sample::ProcSample {
            pid: i32::MAX,
            name: "nobody".into(),
            started: Some(1),
            ..Default::default()
        }];
        a.push(s);
        for answer in [KeyCode::Esc, KeyCode::Char('n'), KeyCode::Char('q')] {
            keys(&mut a, &[KeyCode::Down, KeyCode::Char('x')]);
            assert!(a.pending.is_some(), "no prompt");
            keys(&mut a, &[answer]);
            assert!(a.pending.is_none() && !a.should_quit, "{answer:?}");
            assert!(
                a.signal_note
                    .as_deref()
                    .unwrap_or("")
                    .starts_with("nothing sent")
            );
        }
    }

    #[test]
    fn showing_the_io_columns_starts_collecting_them_whichever_key_did_it() {
        use collect::Source;
        // A probe that found IO unreadable stops collection and hides the
        // columns. `S` used to bring the columns back without the collection.
        let mut a = app();
        let mut s = sample::Sample::unknown();
        s.io_supported = false;
        a.probe_io(&s);
        assert!(!a.show_io && !a.needs().asked(Source::Io));
        a.reveal_io();
        assert!(a.show_io && a.needs().asked(Source::Io));
        a.toggle_io();
        assert!(!a.show_io);
        a.toggle_io();
        assert!(a.show_io && a.needs().asked(Source::Io));
    }

    /// A live monitor with a real collector, signals on, and the newest sample
    /// taken after `child` started.
    fn watching(child: &std::process::Child) -> (App, Platform) {
        let mut collector = Platform::new().expect("no collector");
        let mut a = App::new(60);
        a.signals = true;
        // Two samples, as the monitor has by its second second; the pid is
        // checked in the newest.
        for _ in 0..2 {
            let s = collector.sample(a.needs()).expect("no sample");
            a.push(s);
        }
        assert!(
            a.history
                .newest()
                .is_some_and(|s| s.procs.iter().any(|p| p.pid == child.id() as i32)),
            "the collector did not see the child"
        );
        (a, collector)
    }

    /// Select one process the way a person would: filter to its pid, then Down.
    fn select(a: &mut App, pid: u32) {
        keys(a, &[KeyCode::Char('/')]);
        for c in format!("pid = {pid}").chars() {
            keys(a, &[KeyCode::Char(c)]);
        }
        keys(a, &[KeyCode::Enter, KeyCode::Down]);
    }

    fn sleeper() -> std::process::Child {
        std::process::Command::new("sleep")
            .arg("30")
            .spawn()
            .expect("cannot run sleep")
    }

    #[test]
    fn x_then_y_signals_a_process_the_collector_found_and_it_receives_it() {
        use std::os::unix::process::ExitStatusExt;
        let mut child = sleeper();
        let (mut a, _collector) = watching(&child);
        select(&mut a, child.id());
        keys(&mut a, &[KeyCode::Char('x')]);
        let p = a.pending.as_ref().expect("x asked nothing");
        assert_eq!(p.pid, child.id() as i32, "x asked about another process");
        assert_eq!(&*p.name, "sleep");
        keys(&mut a, &[KeyCode::Char('y')]);
        let status = child.wait().unwrap();
        assert_eq!(status.signal(), Some(15), "the child did not die of TERM");
        assert_eq!(
            a.signal_note.as_deref(),
            Some(&*format!("sent TERM to sleep (pid {})", child.id()))
        );
    }

    #[test]
    fn a_stale_identity_is_refused_by_name_and_the_process_is_untouched() {
        // The pid a reader chose now belongs to another process: the one on
        // screen started at a different moment. Arranged with a real process
        // by asking about its pid with the start time of an earlier one, since
        // making the kernel hand out a particular pid again needs root.
        let mut child = sleeper();
        let (mut a, _collector) = watching(&child);
        select(&mut a, child.id());
        keys(&mut a, &[KeyCode::Char('X')]);
        let p = a.pending.as_mut().expect("X asked nothing");
        p.started = p.started.map(|t| t - 1);
        p.name = "postgres".into();
        keys(&mut a, &[KeyCode::Char('y')]);
        let alive = child.try_wait().unwrap().is_none();
        child.kill().unwrap();
        child.wait().unwrap();
        assert_eq!(
            a.signal_note.as_deref(),
            Some(&*format!(
                "pid {} is sleep now, not postgres — nothing was sent",
                child.id()
            ))
        );
        assert!(alive, "a process that was not chosen was signalled");
    }

    #[test]
    fn a_process_that_exited_after_the_sample_is_refused_and_not_signalled() {
        // Exited and reaped after the newest sample, which still shows it.
        // The sample says yes; the kernel, asked at the moment of sending, says
        // it is gone — or, if its pid has been handed on already, that it is
        // another process. Either is a refusal.
        let mut child = sleeper();
        let (mut a, _collector) = watching(&child);
        select(&mut a, child.id());
        keys(&mut a, &[KeyCode::Char('x')]);
        assert!(a.pending.is_some());
        child.kill().unwrap();
        child.wait().unwrap();
        keys(&mut a, &[KeyCode::Char('y')]);
        let note = a.signal_note.clone().unwrap_or_default();
        assert!(
            note == format!("sleep (pid {}) is no longer running", child.id())
                || note.contains("is another process now"),
            "{note}"
        );
    }

    fn k(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    /// `rejoin` fed the keys crossterm reports for `rest` arriving after a
    /// lone ESC: each byte a character, uppercase with Shift.
    fn after_esc(rest: &str) -> Vec<KeyEvent> {
        let mut rest: std::collections::VecDeque<KeyEvent> = rest
            .chars()
            .map(|c| {
                let shift = if c.is_ascii_uppercase() {
                    KeyModifiers::SHIFT
                } else {
                    KeyModifiers::NONE
                };
                KeyEvent::new(KeyCode::Char(c), shift)
            })
            .collect();
        rejoin(k(KeyCode::Esc), || rest.pop_front())
    }

    #[test]
    fn an_escape_sequence_split_after_its_esc_is_put_back_together() {
        let rows: &[(&str, KeyCode, KeyModifiers)] = &[
            ("[A", KeyCode::Up, KeyModifiers::NONE),
            ("[B", KeyCode::Down, KeyModifiers::NONE),
            ("[C", KeyCode::Right, KeyModifiers::NONE),
            ("[D", KeyCode::Left, KeyModifiers::NONE),
            ("OA", KeyCode::Up, KeyModifiers::NONE),
            ("[H", KeyCode::Home, KeyModifiers::NONE),
            ("[F", KeyCode::End, KeyModifiers::NONE),
            ("[1~", KeyCode::Home, KeyModifiers::NONE),
            ("[4~", KeyCode::End, KeyModifiers::NONE),
            ("[5~", KeyCode::PageUp, KeyModifiers::NONE),
            ("[6~", KeyCode::PageDown, KeyModifiers::NONE),
            // Shift-Right is ten samples at a time.
            ("[1;2C", KeyCode::Right, KeyModifiers::SHIFT),
            ("[1;2D", KeyCode::Left, KeyModifiers::SHIFT),
        ];
        for (rest, code, mods) in rows {
            assert_eq!(
                after_esc(rest),
                vec![KeyEvent::new(*code, *mods)],
                "ESC then {rest}"
            );
        }
    }

    #[test]
    fn a_real_esc_is_still_esc_and_nothing_half_read_is_typed() {
        // Alone, it is Esc: nothing followed within the wait.
        assert_eq!(after_esc(""), vec![k(KeyCode::Esc)]);
        // Followed by something that does not start a sequence: both.
        assert_eq!(after_esc("q"), vec![k(KeyCode::Esc), k(KeyCode::Char('q'))]);
        // A sequence cut off, or one poptop has no key for: dropped whole,
        // not typed into a filter as `[3` and `~`.
        assert_eq!(after_esc("["), vec![]);
        assert_eq!(after_esc("[5"), vec![]);
        assert_eq!(after_esc("[3~"), vec![]);
        assert_eq!(after_esc("[123456789~"), vec![]);
        // Only a bare Esc waits; anything else is itself.
        let up = k(KeyCode::Up);
        assert_eq!(rejoin(up, || panic!("waited after Up")), vec![up]);
        let alt_esc = KeyEvent::new(KeyCode::Esc, KeyModifiers::ALT);
        assert_eq!(rejoin(alt_esc, || panic!("waited")), vec![alt_esc]);
    }
}
