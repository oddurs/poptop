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

    /// The question, naming the process, within `width` columns.
    ///
    /// A ladder, like the key hints and the jump box. Rendered raw it was
    /// simply clipped, and what an eighty-column terminal lost was the *end*:
    /// the pid, which is the whole reason this prompt exists, and the words
    /// saying how to answer — while every key is being swallowed by a modal
    /// state the reader has no visible way out of.
    ///
    /// So the parts give way in order of what the question can do without. The
    /// command line first: it is the most useful part when there is room and
    /// the only droppable one when there is not. Then the sentence explaining
    /// `y`, down to `y/n`. The name and the pid are what the question *is*.
    pub fn question(&self, width: usize) -> String {
        let sig = self.signal.name();
        let cmd = self.cmd.as_deref().filter(|c| !c.is_empty());
        // Strictly less: the footer draws a leading space of its own.
        let fits = |s: &str| s.chars().count() < width;

        // Room for the command line is whatever the rest of the line does not
        // need, and it is only worth having if a few words of it survive.
        let bare = format!(
            "send {sig} to {} (pid {})?  y to confirm, anything else cancels",
            self.name, self.pid
        );
        if let Some(cmd) = cmd {
            let room = width.saturating_sub(bare.chars().count() + 4);
            if room >= 12 {
                let with = format!(
                    "send {sig} to {} — {} (pid {})?  y to confirm, anything else cancels",
                    self.name,
                    first_words(cmd, room),
                    self.pid
                );
                if fits(&with) {
                    return with;
                }
            }
        }
        for line in [
            bare,
            format!("send {sig} to {} (pid {})? y/n", self.name, self.pid),
            format!("{sig} {} (pid {})? y/n", self.name, self.pid),
        ] {
            if fits(&line) {
                return line;
            }
        }
        // Narrower than the name and the pid together. Nothing here is worth
        // dropping to fit a terminal that cannot show a process's name, so this
        // is the one case that is allowed to be clipped.
        format!("{sig} {} (pid {})? y/n", self.name, self.pid)
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
    /// The buffer is a recorded day, not this machine's present at all.
    ///
    /// Distinct from [`Refused::Scrubbing`] because `History::is_live` means
    /// only "the cursor is untethered", and in a day opened with `--read` it
    /// says `true` the moment somebody presses `End`. The recycle check would
    /// then compare a pid against last Tuesday's process table, match, and
    /// signal whatever holds that number on the machine today — the exact
    /// hazard this whole module claims to have removed.
    Recorded,
    /// The process is not in the newest sample.
    Gone,
    /// A process with that pid is, and it is not the one that was selected.
    Recycled(Arc<str>),
    /// The platform would not say when it started, so it cannot be told apart
    /// from a later process on the same pid.
    Unidentifiable,
    /// A pid that is not one process.
    ///
    /// `kill(0, …)` signals poptop's whole process group — the reader's shell
    /// included — and a negative pid signals a group by number. Nothing routes
    /// one here, and this is the one `unsafe` call in the feature, so it is
    /// checked rather than reasoned about.
    NotAProcess,
}

impl Refused {
    /// Why, naming the process where there is one to name.
    ///
    /// `Option`, because two of these are refused before a process has been
    /// chosen — and a placeholder `Pending` built to satisfy a signature is a
    /// zero pid waiting to reach [`send`].
    pub fn why(&self, p: Option<&Pending>) -> String {
        let name = p.map_or("that process", |p| &*p.name);
        let pid = p.map_or(0, |p| p.pid);
        match self {
            Refused::Scrubbing => "not while scrubbing — this table is history, and the pid may since have been reused. Space or End to go live".to_string(),
            Refused::Recorded => "not in a recorded day — those rows are last week's, and their pids belong to other processes now".to_string(),
            Refused::Gone => format!("{name} (pid {pid}) is no longer running"),
            Refused::Recycled(now) => {
                format!("pid {pid} is {now} now, not {name} — nothing was sent")
            }
            Refused::Unidentifiable => format!("this platform did not say when {name} started, so its pid cannot be told apart from a later process"),
            Refused::NotAProcess => format!("pid {pid} is not one process — nothing was sent"),
        }
    }
}

/// Check the pending signal against the newest sample.
///
/// Split from the sending so the whole decision is testable without a process
/// to kill. `live` is the newest sample, not the one under the cursor — and
/// "newest" is the honest word: it is at most one sample interval old, so a
/// process that exited within that interval and had its pid handed on is a gap
/// this cannot close. At the default one-second interval that gap is a second;
/// at `--interval=60s` it is a minute, and the confirmation is worth answering
/// promptly.
pub fn check(
    p: &Pending,
    live: Option<&Sample>,
    scrubbing: bool,
    replaying: bool,
) -> Result<(), Refused> {
    // Before anything else. `History::is_live` means "the cursor is
    // untethered", which in a recorded day is true the moment somebody
    // presses `End` — and every check below would then be made against last
    // Tuesday's process table.
    if replaying {
        return Err(Refused::Recorded);
    }
    if scrubbing {
        return Err(Refused::Scrubbing);
    }
    if p.pid <= 0 {
        return Err(Refused::NotAProcess);
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
    // `check` refuses these already; this is the one `unsafe` call in the
    // feature and `kill(0, SIGKILL)` signals poptop's whole process group,
    // the reader's shell included. Worth a second line rather than an argument
    // about which caller could reach it.
    if p.pid <= 0 {
        return Err(std::io::Error::other("not one process"));
    }
    // SAFETY: `kill` takes two integers and touches nothing of ours. The pid
    // is positive, was read from a live sample, and has just been checked
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
        assert_eq!(check(&p, Some(&now), true, false), Err(Refused::Scrubbing));
        assert_eq!(check(&p, Some(&now), false, false), Ok(()));
        // And it says how to get back to a table it will act on.
        assert!(Refused::Scrubbing.why(None).contains("Space or End"));
    }

    #[test]
    fn a_recycled_pid_is_refused_by_name() {
        // The reason poptop identifies a process by pid *and* start time
        // everywhere else. Between selecting a row and confirming it, the
        // process can exit and the kernel can hand the number to something
        // else — and `kill 4823` would then stop whatever that is.
        let p = Pending::new(&proc(42, "postgres", Some(7)), Signal::Kill);
        let now = live(vec![proc(42, "sshd", Some(900))]);
        let refused = check(&p, Some(&now), false, false).unwrap_err();
        assert_eq!(refused, Refused::Recycled(Arc::from("sshd")));
        let why = refused.why(Some(&p));
        assert!(why.contains("sshd") && why.contains("postgres"), "{why}");
        assert!(why.contains("nothing was sent"), "{why}");
    }

    #[test]
    fn a_process_that_has_gone_is_said_so_rather_than_signalled() {
        let p = Pending::new(&proc(42, "postgres", Some(7)), Signal::Term);
        assert_eq!(
            check(
                &p,
                Some(&live(vec![proc(1, "init", Some(0))])),
                false,
                false
            ),
            Err(Refused::Gone)
        );
        // And with no sample at all, which is the first frame.
        assert_eq!(check(&p, None, false, false), Err(Refused::Gone));
    }

    #[test]
    fn a_process_with_no_start_time_is_not_signalled_at_all() {
        // macOS reported no start time for a third of processes before poptop
        // read `sysctl` directly. Without it a recycled pid cannot be told
        // from the process that was selected, and this is the one place where
        // guessing costs somebody else's work.
        let p = Pending::new(&proc(42, "postgres", None), Signal::Kill);
        let now = live(vec![proc(42, "postgres", None)]);
        assert_eq!(
            check(&p, Some(&now), false, false),
            Err(Refused::Unidentifiable)
        );
        assert!(p.question(120).contains("postgres"));
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
        let q = Pending::new(&p, Signal::Term).question(160);
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
        let q = Pending::new(&long, Signal::Kill).question(160);
        assert!(q.contains('…'), "a long command line was not cut: {q}");
        assert!(!q.contains("production.yaml"), "{q}");
        assert!(q.chars().count() <= 160, "{}", q.chars().count());
    }

    #[test]
    fn the_question_gives_up_the_command_line_before_the_pid() {
        // Rendered raw it was clipped, and what an eighty-column terminal lost
        // was the end — the pid, which is the whole reason this prompt exists,
        // and the words saying how to answer, while every key is being
        // swallowed by a modal state with no visible way out.
        let mut p = proc(4823, "postgres", Some(7));
        p.cmd = Some(Arc::from(
            "/usr/local/pgsql/bin/postgres -D /var/db/postgres/data -c log_line_prefix=%m",
        ));
        let pending = Pending::new(&p, Signal::Term);

        for width in [40usize, 60, 80, 100, 140, 200] {
            let q = pending.question(width);
            assert!(q.contains("pid 4823"), "width {width} lost the pid: {q}");
            assert!(
                q.contains("y to confirm") || q.contains("y/n"),
                "width {width} lost how to answer: {q}"
            );
            assert!(q.contains("postgres"), "width {width} lost the name: {q}");
            assert!(q.contains("TERM"), "width {width}: {q}");
        }
        // Where there is room the command line is there, because two `python3`
        // processes are told apart by their arguments and by nothing else.
        assert!(pending.question(200).contains("log_line_prefix"));
        // Where there is not, it is the part that gives way.
        assert!(!pending.question(80).contains("log_line_prefix"));

        // And the whole line fits, which is what a clipped prompt did not.
        for width in [60usize, 80, 100, 140, 200] {
            let n = pending.question(width).chars().count();
            assert!(n <= width, "width {width} produced {n} columns");
        }
    }

    #[test]
    fn a_non_process_pid_never_reaches_kill() {
        // `kill(0, SIGKILL)` signals poptop's whole process group — the
        // reader's shell included — and a negative pid signals a group by
        // number. Nothing routes one here, and this is the one `unsafe` call
        // in the feature, so it is checked rather than reasoned about.
        for pid in [0, -1, -4823] {
            let p = Pending::new(&proc(pid, "shell", Some(7)), Signal::Kill);
            let now = live(vec![proc(pid, "shell", Some(7))]);
            assert_eq!(
                check(&p, Some(&now), false, false),
                Err(Refused::NotAProcess),
                "pid {pid} was accepted"
            );
            // And again in `send`, which is the call that would do it.
            assert!(send(&p).is_err(), "pid {pid} reached kill");
        }
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
