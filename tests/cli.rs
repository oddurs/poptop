//! The binary, run the way a person or a script runs it.
//!
//! Everything else in the repository tests poptop from the inside. These start
//! the built program with a command line, an empty home and no terminal, and
//! check what a caller can see: the exit code, what went to stdout, and what
//! went to stderr. A script depends on exactly those three, and nothing else
//! checked them.
//!
//! Hermetic: every run gets its own `HOME`, config and state directories, and
//! no inherited environment beyond `PATH`. Nothing needs root or the network.

// A test crate: every helper here runs inside a test, where a panic is the
// failure being reported. clippy.toml's test exemption does not reach helpers
// outside `#[test]` functions in an integration test.
#![allow(clippy::unwrap_used)]

mod common;

use common::{Home, Run, log_a_sample, logged_day};
use std::path::Path;
use std::process::Stdio;

#[test]
fn help_and_version_answer_on_stdout() {
    let home = Home::new();
    for flag in ["--help", "-h"] {
        let r = home.run(&[flag]).ok();
        assert!(
            r.out
                .starts_with("poptop — a system monitor you can rewind")
        );
        assert!(r.out.contains("EXIT STATUS:"));
    }
    for flag in ["--version", "-V"] {
        let r = home.run(&[flag]).ok();
        assert_eq!(
            r.out.trim(),
            format!("poptop {}", env!("CARGO_PKG_VERSION"))
        );
    }
}

#[test]
fn once_prints_a_sample_and_every_setting_is_accepted_on_the_command_line() {
    let home = Home::new();
    let r = home.run(&["--once", "--interval=200ms"]).ok_sampling();
    assert!(r.out.starts_with("cpu "), "{}", r.out);
    assert!(r.out.contains("\nmem "), "{}", r.out);

    // Every setting in --help, at a value it takes, in one run.
    home.run(&[
        "--once",
        "--glyphs=ascii",
        "--graph=line",
        "--scale=fit",
        "--density=compact",
        "--surface=off",
        "--mouse=off",
        "--smooth=2s",
        "--color=mono",
        "--interval=200ms",
        "--window=1m",
        "--store=off",
        "--log=off",
        "--log-interval=1s",
        "--log-days=3",
        "--log-bytes=10M",
        "--signals=off",
        "--warn=40",
        "--critical=70",
        "--theme=classic",
    ])
    .ok_sampling();
}

#[test]
fn a_setting_at_a_value_it_does_not_take_is_exit_2_naming_it() {
    let home = Home::new();
    for flag in [
        "--glyphs=crayon",
        "--graph=crayon",
        "--scale=sideways",
        "--density=airy",
        "--surface=maybe",
        "--mouse=maybe",
        "--smooth=later",
        "--color=loud",
        "--interval=soon",
        "--window=forever",
        "--store=maybe",
        "--log=maybe",
        "--log-interval=often",
        "--log-days=lots",
        "--log-bytes=big",
        "--signals=maybe",
        "--warn=hot",
        "--critical=hotter",
    ] {
        let key = flag.split('=').next().unwrap();
        home.run(&["--once", flag]).refused(2, flag);
        // The same flag, before the command word, is the same error.
        home.run(&[flag, "--once"]).refused(2, key);
    }
    // A pair that cannot hold together is refused, whichever flag broke it.
    home.run(&["--once", "--warn=90", "--critical=80"])
        .refused(2, "");
    home.run(&["--once", "--theme=nope"])
        .refused(2, "no theme `nope`");
}

#[test]
fn a_bad_line_in_the_config_file_warns_and_poptop_runs_anyway() {
    let home = Home::new();
    home.write_config("glyphs = crayon\nnonsense = 1\ntheme = classic\n");
    let r = home.run(&["--once", "--interval=200ms"]);
    assert_eq!(r.code, Some(0), "{}", r.err);
    assert!(r.out.starts_with("cpu "));
    assert!(r.err.contains("glyphs"), "{}", r.err);
    assert!(r.err.contains("nonsense"), "{}", r.err);
    // A flag overrides the file, and a bad flag is still fatal.
    home.run(&["--once", "--glyphs=crayon"])
        .refused(2, "--glyphs=crayon");
}

#[test]
fn a_command_line_that_cannot_run_is_exit_2_on_stderr() {
    let home = Home::new();
    for (args, says) in [
        (&["--frobnicate"][..], "unrecognised option '--frobnicate'"),
        (&["top"], "unrecognised option 'top'"),
        (&["--read"], "--read needs a date"),
        (&["--read", "yesterday"], "`yesterday` is not a date"),
        (&["--report", "2026-02-30"], "`2026-02-30` is not a date"),
        (&["--export"], "--export takes `json` or `line`"),
        (&["--export=csv"], "--export takes `json` or `line`"),
        // `--for` without `--follow` is a feed that was never asked for, and
        // a day that is followed is a file that does not grow. Both refused
        // rather than ignored.
        (&["--export=json", "--for", "1s"], "--for is --follow's"),
        (
            &["--export=json", "--follow", "--for"],
            "--for needs a span",
        ),
        (
            &["--export=json", "--follow", "--for=soon"],
            "--for `soon`: expected a span",
        ),
        (&["--check-theme"], "--check-theme needs a theme name"),
        (&["--verify", "today"], "`today` is not a date"),
        (&["--export=json", "--fields"], "--fields needs a list"),
        (
            &["--export=json", "--fields", "cpu_totl"],
            "no field `cpu_totl`",
        ),
        (
            &["--verify", "2026-09-08", "x"],
            "--verify does not take `x`",
        ),
        (&["--once", "--days"], "--once does not take `--days`"),
        (&["--schema", "x"], "--schema does not take `x`"),
        (&["--bench", "--once"], "--bench does not take `--once`"),
        (&["--version", "--help"], "--version does not take `--help`"),
    ] {
        home.run(args).refused(2, says);
    }
    // The usage follows an unrecognised option, on stderr.
    let r = home.run(&["--frobnicate"]);
    assert!(r.err.contains("USAGE:"), "{}", r.err);
}

#[test]
fn the_monitor_without_a_terminal_is_exit_2_and_writes_nothing() {
    // stdin is /dev/null and stdout a pipe. This panicked inside ratatui with
    // exit 101, after writing escape codes to the pipe.
    let home = Home::new();
    home.run(&[]).refused(2, "the monitor needs a terminal");
    // `--read` checks the terminal before reading the day, so a missing day
    // is not the complaint.
    home.run(&["--read", "2001-01-01"])
        .refused(2, "the monitor needs a terminal");
}

#[test]
fn a_recorded_day_is_listed_reported_and_exported() {
    let home = Home::new();
    // Nothing yet.
    let r = home.run(&["--days"]).ok();
    assert!(r.out.starts_with("no logs in "), "{}", r.out);

    log_a_sample(&home);
    log_a_sample(&home);
    let day = logged_day(&home);
    let listed = home.run(&["--days"]).ok();
    assert!(
        listed.out.starts_with(&format!("{day}  ")),
        "{}",
        listed.out
    );

    // Today's report, named and unnamed, in both spellings.
    for args in [
        vec!["--report"],
        vec!["--report", &day],
        vec![&*Box::leak(format!("--report={day}").into_boxed_str())],
    ] {
        let r = home.run(&args).ok();
        assert!(
            r.out.starts_with(&format!("poptop report for {day}")),
            "{}",
            r.out
        );
    }

    // One JSON object per sample, one per line.
    let json = home.run(&["--export", "json", &day]).ok();
    let lines: Vec<&str> = json.out.lines().collect();
    assert_eq!(lines.len(), 2, "{}", json.out);
    for l in lines {
        assert!(l.starts_with('{') && l.ends_with('}'), "{l}");
    }
    let line = home.run(&["--export=line", &day]).ok();
    assert!(line.out.lines().count() > 2, "{}", line.out);
}

#[test]
fn a_day_that_is_not_there_is_exit_1() {
    let home = Home::new();
    home.run(&["--report", "2001-01-01"]).refused(1, "");
    home.run(&["--export", "json", "2001-01-01"]).refused(1, "");
}

#[test]
fn no_state_directory_is_exit_2_for_the_commands_that_need_one() {
    let home = Home::new();
    for args in [
        &["--days"][..],
        &["--report"],
        &["--export", "json", "2026-09-08"],
    ] {
        let r = home
            .cmd(args)
            .env_remove("HOME")
            .env_remove("XDG_STATE_HOME")
            .output()
            .unwrap();
        Run::of(args, r).refused(2, "no state directory");
    }
    // `--once` does not need one, and says so only if asked to log.
    let r = home
        .cmd(&["--once", "--interval=200ms", "--log=on"])
        .env_remove("HOME")
        .env_remove("XDG_STATE_HOME")
        .output()
        .unwrap();
    let r = Run::of(&["--once", "--log=on"], r);
    assert_eq!(r.code, Some(0), "{}", r.err);
    assert!(
        r.err.contains("no state directory to log into"),
        "{}",
        r.err
    );
}

#[test]
fn the_machine_now_is_exported_in_both_formats() {
    let home = Home::new();
    let json = home.run(&["--export=json", "--interval=200ms"]).ok();
    assert_eq!(json.out.lines().count(), 1, "{}", json.out);
    assert!(json.out.starts_with("{\"at\":"), "{}", json.out);
    let line = home.run(&["--export", "line", "--interval=200ms"]).ok();
    assert!(line.out.lines().count() > 2);
}

/// A feed, run end to end the way a script runs one. What it writes while it
/// runs, and how it is stopped, are in `tests/export.rs`.
#[test]
fn a_feed_runs_for_the_span_it_was_given() {
    let home = Home::new();
    let r = home
        .run(&[
            "--export=json",
            "--follow",
            "--interval=200ms",
            "--for",
            "1s",
        ])
        .ok();
    let records = r.out.lines().count();
    assert!((2..=8).contains(&records), "{records} records:\n{}", r.out);
    for line in r.out.lines() {
        assert!(line.starts_with("{\"at\":"), "not a record: {line}");
    }
}

#[test]
fn config_says_what_every_setting_is_and_where_it_came_from() {
    let home = Home::new();
    // Nothing set: every setting is its default.
    let r = home.run(&["--config"]).ok();
    let lines: Vec<&str> = r.out.lines().collect();
    assert!(
        lines.iter().all(|l| l.ends_with("the default")),
        "{}",
        r.out
    );
    assert!(
        lines.iter().any(|l| l.starts_with("interval ")),
        "{}",
        r.out
    );

    // A file, a flag and the environment each name themselves.
    home.write_config("# a comment\ninterval = 2s\n");
    let r = home
        .cmd(&["--config", "--window=30m"])
        .env("NO_COLOR", "1")
        .output()
        .unwrap();
    let r = Run::of(&["--config", "--window=30m"], r).ok();
    let at = |key: &str| {
        r.out
            .lines()
            .find(|l| l.starts_with(&format!("{key} ")))
            .unwrap_or_else(|| panic!("no `{key}` in:\n{}", r.out))
    };
    assert!(
        at("interval").ends_with("poptop.conf:2"),
        "{}",
        at("interval")
    );
    assert!(at("interval").contains(" 2s "), "{}", at("interval"));
    assert!(at("window").ends_with("--window=30m"), "{}", at("window"));
    assert!(at("color").ends_with("NO_COLOR"), "{}", at("color"));
    assert!(
        at("log-days").ends_with("the default"),
        "{}",
        at("log-days")
    );
}

#[test]
fn write_config_writes_a_file_poptop_reads_back() {
    let home = Home::new();
    let r = home.run(&["--write-config"]).ok();
    assert!(r.out.starts_with("wrote "), "{}", r.out);
    let path = r.out.trim().trim_start_matches("wrote ").to_string();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.starts_with("# poptop configuration"), "{text}");

    // Every setting is in it, and reading it back changes nothing: each line
    // is now the origin of a value that is the same as the default it wrote.
    let after = home.run(&["--config"]).ok();
    for line in after.out.lines() {
        // A setting with nothing to show — no columns hidden — is written as
        // a comment, because a line with an empty value is one poptop will
        // not read back. It reads as the default, which is what it is.
        let empty_valued = line.starts_with("hide-columns ");
        assert!(
            line.contains("poptop.conf:") || empty_valued,
            "not from the file: {line}"
        );
    }
    let plain = Home::new().run(&["--config"]).ok();
    let values = |out: &str| -> Vec<String> {
        out.lines()
            .map(|l| {
                l.rsplit_once("  ")
                    .map(|(v, _)| v.trim().to_string())
                    .unwrap_or_default()
            })
            .collect()
    };
    assert_eq!(
        values(&after.out),
        values(&plain.out),
        "the round trip changed a value"
    );

    // It never writes over one.
    home.run(&["--write-config"]).refused(2, &path);
}

#[test]
fn keys_lists_every_action_and_a_file_can_move_one() {
    let home = Home::new();
    let r = home.run(&["--keys"]).ok();
    assert!(r.out.lines().count() >= 20, "{}", r.out);
    let row = |out: &str, action: &str| {
        out.lines()
            .find(|l| l.starts_with(&format!("{action} ")))
            .unwrap_or_else(|| panic!("no `{action}` in:\n{out}"))
            .to_string()
    };
    assert!(
        row(&r.out, "quit").contains(" q "),
        "{}",
        row(&r.out, "quit")
    );
    assert!(row(&r.out, "quit").ends_with("the default"));

    // Moved, with the file named as the origin.
    home.write_config("key.quit = Q, ctrl-q\nkey.filter = /, f\n");
    let r = home.run(&["--keys"]).ok();
    assert!(
        row(&r.out, "quit").contains("Q, ctrl-q"),
        "{}",
        row(&r.out, "quit")
    );
    assert!(
        row(&r.out, "quit").contains("poptop.conf:1"),
        "{}",
        row(&r.out, "quit")
    );
    assert!(
        row(&r.out, "filter").contains("/, f"),
        "{}",
        row(&r.out, "filter")
    );

    // A key another action holds, and an action that does not exist: both
    // warn, naming what is wrong, and poptop still starts.
    home.write_config("key.filter = t\nkey.nonsense = z\n");
    let r = home.run(&["--keys"]);
    assert_eq!(r.code, Some(0), "{}", r.err);
    assert!(r.err.contains("already `tree`"), "{}", r.err);
    assert!(r.err.contains("unknown action `nonsense`"), "{}", r.err);
    assert!(
        row(&r.out, "filter").contains(" / "),
        "the refused binding was taken"
    );
    assert!(
        row(&r.out, "tree").contains(" t "),
        "{}",
        row(&r.out, "tree")
    );
}

#[test]
fn the_starting_state_comes_from_the_file() {
    let home = Home::new();
    home.write_config(
        "view = memory\nsort = pid\nzoom = 4\ngroup = user\n\
         kernel-threads = on\nio-columns = off\nhide-columns = thr, cid\n",
    );
    let r = home.run(&["--config"]).ok();
    let at = |key: &str| {
        r.out
            .lines()
            .find(|l| l.starts_with(&format!("{key} ")))
            .unwrap_or_else(|| panic!("no `{key}` in:\n{}", r.out))
            .to_string()
    };
    assert!(at("view").contains(" memory "), "{}", at("view"));
    assert!(at("sort").contains(" pid "), "{}", at("sort"));
    assert!(at("zoom").contains(" 4 "), "{}", at("zoom"));
    assert!(at("group").contains(" user "), "{}", at("group"));
    assert!(
        at("kernel-threads").contains(" on "),
        "{}",
        at("kernel-threads")
    );
    assert!(at("io-columns").contains(" off "), "{}", at("io-columns"));
    assert!(
        at("hide-columns").contains("thr, cid"),
        "{}",
        at("hide-columns")
    );

    // A pair that cannot both hold warns, and poptop still starts.
    home.write_config("tree = on\ngroup = name\n");
    let r = home.run(&["--once", "--interval=200ms"]);
    assert_eq!(r.code, Some(0), "{}", r.err);

    // A value that is not one of the choices warns and keeps the default.
    home.write_config("view = sideways\n");
    let r = home.run(&["--config"]);
    assert_eq!(r.code, Some(0), "{}", r.err);
    assert!(r.err.contains("cpu, memory or disk"), "{}", r.err);
    assert!(
        r.out
            .lines()
            .any(|l| l.starts_with("view ") && l.contains(" cpu ")),
        "{}",
        r.out
    );
}

#[test]
fn the_schema_is_json_on_stdout() {
    let r = Home::new().run(&["--schema"]).ok();
    assert!(r.out.trim_start().starts_with('{') && r.out.trim_end().ends_with('}'));
}

#[test]
fn bench_times_every_collection_level() {
    let r = Home::new().run(&["--bench"]).ok();
    assert_eq!(r.out.lines().count(), 6, "{}", r.out);
    assert!(r.out.lines().all(|l| l.contains("/sample")), "{}", r.out);
}

#[test]
fn a_theme_check_exits_by_its_verdict() {
    let home = Home::new();
    // `safe` is the default and passes; `classic` restores green, which is
    // the documented failure under simulated protanopia.
    let pass = home.run(&["--check-theme", "safe"]).ok();
    assert!(pass.out.contains("PASS"), "{}", pass.out);
    let fail = home.run(&["--check-theme=classic"]);
    assert_eq!(fail.code, Some(1), "{}", fail.out);
    assert!(fail.out.contains("FAIL"), "{}", fail.out);
    home.run(&["--check-theme", "nope"])
        .refused(2, "no theme `nope`");
}

#[test]
fn a_reader_that_goes_away_is_not_a_crash() {
    // `poptop --schema | head -c1`: the pipe closes under the writer. Rust
    // turns the write error into a panic unless it is handled.
    let home = Home::new();
    for args in [
        &["--schema"][..],
        &["--once", "--interval=200ms"],
        &["--help"],
    ] {
        let mut child = home
            .cmd(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        drop(child.stdout.take());
        let o = child.wait_with_output().unwrap();
        let err = String::from_utf8_lossy(&o.stderr);
        assert!(!err.contains("panicked"), "{args:?}: {err}");
        assert_eq!(o.status.code(), Some(0), "{args:?}: {err}");
    }
}

/// Every flag `--help` names appears somewhere in this file, so a new one
/// without a test fails here rather than going unnoticed.
#[test]
fn every_flag_in_the_help_is_run_by_a_test_here() {
    let help = Home::new().run(&["--help"]).ok().out;
    let me = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/cli.rs"))
        .unwrap();
    let mut missing = Vec::new();
    // Long flags wherever they are named; short ones only where a line
    // defines one (`-h, --help`), since the prose mentions atop's `-b`.
    let defined = help
        .lines()
        .filter_map(|l| l.trim_start().strip_prefix('-'))
        .filter_map(|rest| rest.split([',', ' ']).next())
        .filter(|f| f.len() == 1)
        .map(|f| format!("-{f}"));
    let long = help
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
        .map(|w| w.trim_end_matches('-'))
        .filter(|w| w.starts_with("--") && w.len() > 2)
        .map(String::from);
    for flag in defined.chain(long) {
        if !me.contains(&format!("\"{flag}")) && !missing.contains(&flag) {
            missing.push(flag);
        }
    }
    assert!(
        missing.is_empty(),
        "flags in --help with no test: {missing:?}"
    );
}

/// The byte budget, end to end: poptop keeps the most recent `log-bytes` of
/// history rather than the first of it, and `--days` says what is left.
#[test]
fn at_the_byte_budget_the_log_keeps_the_newest_and_says_what_it_holds() {
    let home = Home::new();
    log_a_sample(&home);
    let day = logged_day(&home);
    let one = std::fs::metadata(
        home.state()
            .join("poptop")
            .join("log")
            .join(format!("poptop-{}", day.replace('-', ""))),
    )
    .unwrap()
    .len();

    // Room for two entries, and four samples logged into it.
    let cap = format!("--log-bytes={}", one * 2);
    let mut said = Vec::new();
    for _ in 0..3 {
        let r = home.run(&["--once", "--log=on", "--interval=200ms", &cap]);
        assert_eq!(r.code, Some(0), "{}", r.err);
        said.push(r.err);
    }
    let size = std::fs::metadata(
        home.state()
            .join("poptop")
            .join("log")
            .join(format!("poptop-{}", day.replace('-', ""))),
    )
    .unwrap()
    .len();
    assert!(
        size <= one * 2,
        "the log grew past log-bytes: {size} > {}",
        one * 2
    );
    assert!(
        said.iter().any(|e| e.contains("log-bytes")),
        "history was given up silently: {said:?}"
    );

    // Still a readable day, and one that says where it now starts.
    let export = home.run(&["--export=json", &day]).ok();
    assert!(
        (1..=2).contains(&export.out.lines().count()),
        "{} records in a two-entry budget",
        export.out.lines().count()
    );
    let listed = home.run(&["--days"]).ok();
    assert!(
        listed.out.contains(" from "),
        "--days does not say what the day holds: {}",
        listed.out
    );
}

/// `--verify` is the answer to "is this file intact, and what is in it" from
/// a script — which until now needed a terminal.
#[test]
fn verify_reports_a_day_and_exits_by_whether_it_is_intact() {
    let home = Home::new();
    log_a_sample(&home);
    log_a_sample(&home);
    let day = logged_day(&home);
    let r = home.run(&["--verify"]).ok();
    assert!(r.out.contains("entries 2 whole"), "{}", r.out);
    assert!(r.out.contains("period  "), "{}", r.out);
    assert!(r.out.trim_end().ends_with("intact"), "{}", r.out);
    // A date, and the same answer.
    assert_eq!(home.run(&["--verify", &day]).ok().out, r.out);

    // Now break it: a length with nothing behind it, appended to the day.
    let path = home
        .state()
        .join("poptop")
        .join("log")
        .join(format!("poptop-{}", day.replace('-', "")));
    let mut bytes = std::fs::read(&path).unwrap();
    bytes.extend_from_slice(&[64, 0, 0, 0, 9, 9, 9]);
    std::fs::write(&path, &bytes).unwrap();

    let broken = home.run(&["--verify"]);
    assert_eq!(
        broken.code,
        Some(1),
        "damage did not change the exit status"
    );
    assert!(
        broken.out.contains("damage  at ") && broken.out.contains("cut short"),
        "{}",
        broken.out
    );
    // Still on stdout: it is the answer, not a warning about one.
    assert!(broken.err.is_empty(), "{}", broken.err);

    // A day nobody recorded says so, as every other command does.
    let empty = Home::new();
    empty
        .run(&["--verify", "2026-09-08"])
        .refused(1, "nothing recorded on 2026-09-08");
}

/// `--fields` end to end: the narrow feed a dashboard actually wants.
#[test]
fn a_feed_can_be_narrowed_to_named_fields() {
    let home = Home::new();
    let whole = home.run(&["--export=json", "--interval=200ms"]).ok();
    let narrow = home
        .run(&[
            "--export=json",
            "--fields",
            "cpu_total,mem.used",
            "--interval=200ms",
        ])
        .ok();
    assert!(
        narrow.out.len() * 10 < whole.out.len(),
        "narrowing saved almost nothing: {} of {} bytes",
        narrow.out.len(),
        whole.out.len()
    );
    assert!(narrow.out.starts_with("{\"at\":"), "{}", narrow.out);
    assert!(narrow.out.contains("\"cpu_total\":"), "{}", narrow.out);
    assert!(narrow.out.contains("\"used\":"), "{}", narrow.out);
    assert!(!narrow.out.contains("\"procs\""), "{}", narrow.out);

    // And in the line format, where the header names exactly what the rows
    // carry.
    let lines = home
        .run(&["--export=line", "--fields=cpu_total", "--interval=200ms"])
        .ok();
    let header = lines
        .out
        .lines()
        .find(|l| l.starts_with("#sample\t"))
        .expect("no header");
    assert_eq!(header, "#sample\tat\tcpu_total", "{}", lines.out);
    assert_eq!(lines.out.lines().count(), 2, "{}", lines.out);
}
