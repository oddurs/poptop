//! Sending a signal to the process under the cursor.
//!
//! The argument against was real and is worth restating rather than skipping:
//! a monitor that cannot change the machine is a monitor that cannot break it,
//! and poptop's entire privileged surface is otherwise reading files. That
//! property is kept — this is off unless it is asked for, once, like the store
//! and the log.
//!
//! The argument for is that the alternative is not "nothing happens". It is
//! somebody reading a pid off one screen and typing it into another terminal,
//! which is where the pid gets mistyped, and by then the number may belong to a
//! different process. poptop knows the name and the command line, so it can
//! *name* what it is about to signal.
//!
//! **And poptop has a hazard no other monitor has.** Every other tool's process
//! table is the present. poptop's may be four minutes old — the row under the
//! cursor while scrubbing is a row from history, and its pid may since have
//! been recycled onto something else. Two rules follow, and together they make
//! this safer than the tools poptop is copying rather than merely equal to
//! them:
//!
//! - Nothing is sent while scrubbing. The reader is looking at the past.
//! - The `(pid, started)` pair — the identity poptop already uses everywhere —
//!   is checked against the newest sample at the moment of sending. A pid that
//!   has been recycled is refused by name, not signalled by number.

use crate::sample::{ProcSample, Sample};
use std::sync::Arc;

unsafe extern "C" {
    fn kill(pid: i32, sig: i32) -> i32;
}

/// What poptop will send, and nothing else.
///
/// Two, not a menu of thirty-one. `TERM` is "please stop" and `KILL` is "stop",
/// which is the whole of the workflow this exists for; a signal picker is a
/// list of ways to get it wrong, and anybody who needs `SIGUSR1` is already in
/// a shell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Signal {
    Term,
    Kill,
}

impl Signal {
    /// The number, which is the same on both platforms poptop builds for.
    pub fn number(self) -> i32 {
        match self {
            Signal::Term => 15,
            Signal::Kill => 9,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Signal::Term => "TERM",
            Signal::Kill => "KILL",
        }
    }
}

/// A signal poptop has been asked to send and has not sent yet.
///
/// The name and command line are captured when the confirmation opens, so the
/// question names the process rather than the number — the number is the part
/// that gets misread.
#[derive(Clone, Debug)]
pub struct Pending {
    pub pid: i32,
    /// The other half of the identity. `None` where the platform would not say
    /// when the process started, which is a process poptop will not signal:
    /// without it a recycled pid is indistinguishable from the process the
    /// reader selected.
    pub started: Option<u64>,
    pub name: Arc<str>,
    pub cmd: Option<Arc<str>>,
    pub signal: Signal,
}

impl Pending {
    pub fn new(p: &ProcSample, signal: Signal) -> Pending {
        Pending {
            pid: p.pid,
            started: p.started,
            name: p.name.clone(),
            cmd: p.cmd.clone(),
            signal,
        }
    }

    /// The question, naming the process.
    pub fn question(&self) -> String {
        // The command line where there is one, because two `python` processes
        // are told apart by their arguments and by nothing else on the row.
        let what = match self.cmd.as_deref().filter(|c| !c.is_empty()) {
            Some(cmd) => format!("{} — {}", self.name, first_words(cmd, 60)),
            None => self.name.to_string(),
        };
        format!(
            "send {} to {what} (pid {})?  y to confirm, anything else cancels",
            self.signal.name(),
            self.pid
        )
    }
}

/// A command line short enough for one line, cut at a word.
fn first_words(cmd: &str, max: usize) -> String {
    if cmd.chars().count() <= max {
        return cmd.to_string();
    }
    let cut: String = cmd.chars().take(max).collect();
    // At a space where there is one, so the tail is a whole argument rather
    // than half of a path.
    match cut.rsplit_once(' ') {
        Some((head, _)) if head.chars().count() > max / 2 => format!("{head} …"),
        _ => format!("{cut}…"),
    }
}

/// Why a signal was not sent.
#[derive(Debug, PartialEq, Eq)]
pub enum Refused {
    /// The reader is looking at history. Every other monitor's table is the
    /// present; poptop's may be four minutes old, and the pid on a row from
    /// then may belong to something else now.
    Scrubbing,
    /// The process is not in the newest sample.
    Gone,
    /// A process with that pid is, and it is not the one that was selected.
    Recycled(Arc<str>),
    /// The platform would not say when it started, so it cannot be told apart
    /// from a later process on the same pid.
    Unidentifiable,
}

impl Refused {
    pub fn why(&self, p: &Pending) -> String {
        match self {
            Refused::Scrubbing => {
                "not while scrubbing — this table is history, and the pid may since \
                 have been reused. Space or End to go live"
                    .to_string()
            }
            Refused::Gone => format!("{} (pid {}) is no longer running", p.name, p.pid),
            Refused::Recycled(now) => format!(
                "pid {} is {now} now, not {} — nothing was sent",
                p.pid, p.name
            ),
            Refused::Unidentifiable => format!(
                "this platform did not say when {} started, so pid {} cannot be told \
                 apart from a later process — nothing was sent",
                p.name, p.pid
            ),
        }
    }
}

/// Check the pending signal against what is running now.
///
/// Split from the sending so the whole decision is testable without a process
/// to kill. `live` is the newest sample, not the one under the cursor.
pub fn check(p: &Pending, live: Option<&Sample>, scrubbing: bool) -> Result<(), Refused> {
    if scrubbing {
        return Err(Refused::Scrubbing);
    }
    let Some(started) = p.started else {
        return Err(Refused::Unidentifiable);
    };
    let Some(live) = live else {
        return Err(Refused::Gone);
    };
    match live.procs.iter().find(|q| q.pid == p.pid) {
        // The identity poptop uses everywhere: pid alone is a number the kernel
        // hands out again.
        Some(q) if q.started == Some(started) => Ok(()),
        Some(q) => Err(Refused::Recycled(q.name.clone())),
        None => Err(Refused::Gone),
    }
}

/// Send it, having checked.
///
/// The `Err` is the operating system's own words — `Operation not permitted`
/// for somebody else's process is the answer, and dressing it up would only
/// hide which of the several reasons it was.
pub fn send(p: &Pending) -> std::io::Result<()> {
    // SAFETY: `kill` takes two integers and touches nothing of ours. The pid
    // is one poptop read from a live sample moments ago and has just checked
    // against the newest one.
    let rc = unsafe { kill(p.pid, p.signal.number()) };
    if rc == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;

    fn proc(pid: i32, name: &str, started: Option<u64>) -> ProcSample {
        ProcSample {
            pid,
            name: Arc::from(name),
            user: Arc::from("root"),
            started,
            ..ProcSample::default()
        }
    }

    fn live(procs: Vec<ProcSample>) -> Sample {
        let mut s = Sample::unknown();
        s.at = SystemTime::now();
        s.procs = procs;
        s
    }

    #[test]
    fn nothing_is_sent_while_looking_at_history() {
        // The hazard no other monitor has. Every other tool's process table is
        // the present; poptop's may be four minutes old, and the pid on a row
        // from then may belong to something else now.
        let p = Pending::new(&proc(42, "postgres", Some(7)), Signal::Term);
        let now = live(vec![proc(42, "postgres", Some(7))]);
        assert_eq!(check(&p, Some(&now), true), Err(Refused::Scrubbing));
        assert_eq!(check(&p, Some(&now), false), Ok(()));
        // And it says how to get back to a table it will act on.
        assert!(Refused::Scrubbing.why(&p).contains("Space or End"));
    }

    #[test]
    fn a_recycled_pid_is_refused_by_name() {
        // The reason poptop identifies a process by pid *and* start time
        // everywhere else. Between selecting a row and confirming it, the
        // process can exit and the kernel can hand the number to something
        // else — and `kill 4823` would then stop whatever that is.
        let p = Pending::new(&proc(42, "postgres", Some(7)), Signal::Kill);
        let now = live(vec![proc(42, "sshd", Some(900))]);
        let refused = check(&p, Some(&now), false).unwrap_err();
        assert_eq!(refused, Refused::Recycled(Arc::from("sshd")));
        let why = refused.why(&p);
        assert!(why.contains("sshd") && why.contains("postgres"), "{why}");
        assert!(why.contains("nothing was sent"), "{why}");
    }

    #[test]
    fn a_process_that_has_gone_is_said_so_rather_than_signalled() {
        let p = Pending::new(&proc(42, "postgres", Some(7)), Signal::Term);
        assert_eq!(
            check(&p, Some(&live(vec![proc(1, "init", Some(0))])), false),
            Err(Refused::Gone)
        );
        // And with no sample at all, which is the first frame.
        assert_eq!(check(&p, None, false), Err(Refused::Gone));
    }

    #[test]
    fn a_process_with_no_start_time_is_not_signalled_at_all() {
        // macOS reported no start time for a third of processes before poptop
        // read `sysctl` directly. Without it a recycled pid cannot be told
        // from the process that was selected, and this is the one place where
        // guessing costs somebody else's work.
        let p = Pending::new(&proc(42, "postgres", None), Signal::Kill);
        let now = live(vec![proc(42, "postgres", None)]);
        assert_eq!(check(&p, Some(&now), false), Err(Refused::Unidentifiable));
        assert!(p.question().contains("postgres"));
    }

    #[test]
    fn the_question_names_the_process_and_not_only_the_number() {
        // The number is the part that gets misread, and it is the only thing
        // the alternative workflow — reading a pid off one screen and typing
        // it into another terminal — carries across.
        let mut p = proc(4823, "python3", Some(7));
        p.cmd = Some(Arc::from(
            "python3 /srv/app/manage.py runworker --queue=email",
        ));
        let q = Pending::new(&p, Signal::Term).question();
        assert!(q.contains("python3"), "{q}");
        assert!(q.contains("runworker"), "{q}");
        assert!(q.contains("pid 4823"), "{q}");
        assert!(q.contains("TERM"), "{q}");
        // Two `python3` processes are told apart by their arguments and by
        // nothing else on the row.
        assert!(q.contains("manage.py"), "{q}");

        // A very long command line is cut at a word, not through a path.
        let mut long = proc(1, "java", Some(1));
        long.cmd = Some(Arc::from(
            "java -Xmx8g -cp /opt/service/lib/one.jar:/opt/service/lib/two.jar \
             com.example.Service --config /etc/service/production.yaml",
        ));
        let q = Pending::new(&long, Signal::Kill).question();
        assert!(q.contains('…'), "a long command line was not cut: {q}");
        assert!(!q.contains("production.yaml"), "{q}");
        assert!(q.chars().count() < 140, "{}", q.chars().count());
    }

    #[test]
    fn the_signal_numbers_are_the_ones_the_kernel_means() {
        // Hard-coded rather than read from a header, so they are worth
        // asserting: both platforms agree on these two and on very little else.
        assert_eq!(Signal::Term.number(), 15);
        assert_eq!(Signal::Kill.number(), 9);
        assert_eq!(Signal::Term.name(), "TERM");
    }

    #[test]
    fn a_signal_that_is_allowed_actually_reaches_a_process() {
        // Sending to poptop's own process group is not a test anybody wants,
        // so signal 0 — the existence check — is the one that can be run: it
        // takes the same path through `kill` and returns the same errors.
        let me = std::process::id() as i32;
        // SAFETY: signal 0 sends nothing; it asks whether the pid exists and
        // whether we could signal it.
        assert_eq!(unsafe { kill(me, 0) }, 0, "poptop cannot signal itself");
        // A pid that cannot exist: the check fails and the error is the
        // operating system's own.
        assert_eq!(unsafe { kill(i32::MAX, 0) }, -1);
        assert!(
            std::io::Error::last_os_error().raw_os_error().is_some(),
            "no errno for a failed kill"
        );
    }
}
