//! What every test of the built binary needs: a home of its own, and a way
//! to run poptop in it and say what came out.

// Each test crate that includes this uses some of it.
#![allow(dead_code)]

use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

/// A home directory of its own, removed afterwards.
pub struct Home(pub PathBuf);

impl Home {
    pub fn new() -> Home {
        static N: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "poptop-cli-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Home(dir)
    }

    pub fn state(&self) -> PathBuf {
        self.0.join("state")
    }

    pub fn config(&self) -> PathBuf {
        self.0.join("config")
    }

    /// The command, with nothing inherited that could change what it does.
    pub fn cmd(&self, args: &[&str]) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_poptop"));
        c.args(args)
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("HOME", &self.0)
            .env("XDG_STATE_HOME", self.state())
            .env("XDG_CONFIG_HOME", self.config())
            .stdin(Stdio::null());
        c
    }

    pub fn run(&self, args: &[&str]) -> Run {
        Run::of(args, self.cmd(args).output().expect("cannot start poptop"))
    }

    /// A user theme file, as `~/.config/poptop/themes/NAME.theme`.
    pub fn write_theme(&self, name: &str, text: &str) {
        let dir = self.config().join("poptop").join("themes");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(format!("{name}.theme")), text).unwrap();
    }

    pub fn write_config(&self, text: &str) {
        let dir = self.config().join("poptop");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("poptop.conf"), text).unwrap();
    }
}

impl Drop for Home {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// What a run left behind, with assertions that say which run it was.
pub struct Run {
    pub line: String,
    pub code: Option<i32>,
    pub out: String,
    pub err: String,
}

impl Run {
    pub fn of(args: &[&str], o: Output) -> Run {
        Run {
            line: format!("poptop {}", args.join(" ")),
            code: o.status.code(),
            out: String::from_utf8_lossy(&o.stdout).into_owned(),
            err: String::from_utf8_lossy(&o.stderr).into_owned(),
        }
    }

    /// Exit 0, data on stdout, and nothing on stderr.
    pub fn ok(self) -> Run {
        assert_eq!(self.code, Some(0), "`{}` failed:\n{}", self.line, self.err);
        assert!(!self.out.is_empty(), "`{}` printed nothing", self.line);
        assert!(
            self.err.is_empty(),
            "`{}` wrote to stderr:\n{}",
            self.line,
            self.err
        );
        self
    }

    /// The code given, nothing on stdout, and one `poptop:` line on stderr
    /// that starts with `says`.
    pub fn refused(self, code: i32, says: &str) -> Run {
        assert_eq!(
            self.code,
            Some(code),
            "`{}`: stdout {:?}, stderr {:?}",
            self.line,
            self.out,
            self.err
        );
        assert!(
            self.out.is_empty(),
            "`{}` failed and still wrote to stdout:\n{}",
            self.line,
            self.out
        );
        let first = self.err.lines().next().unwrap_or("");
        assert!(
            first.starts_with(&format!("poptop: {says}")),
            "`{}` said {first:?}, not `poptop: {says}…`",
            self.line
        );
        assert!(!self.err.contains("panicked"), "`{}` panicked", self.line);
        self
    }
}

/// Today's date as the log names it, found by asking poptop rather than by
/// working out the local date here.
pub fn logged_day(home: &Home) -> String {
    let days = home.run(&["--days"]).ok();
    days.out
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().next())
        .expect("no day listed")
        .to_string()
}

/// Write one sample to today's log, the way cron would.
pub fn log_a_sample(home: &Home) {
    home.run(&["--once", "--log=on", "--interval=200ms"]).ok();
}
