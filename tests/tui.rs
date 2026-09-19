//! The monitor in a real pseudo-terminal.
//!
//! `ui_tests.rs` draws frames into a buffer, which is thorough for layout and
//! blind to the terminal itself: raw mode, the alternate screen, a resize, a
//! key sequence split across two reads, and what the terminal is left as after
//! `q`, Ctrl-C, SIGTERM or a panic. These start the built binary on a pty, as a
//! shell would, and check the terminal's settings afterwards against the ones
//! it was given.
//!
//! No dependency: `openpty`, `ioctl` and `tcgetattr` are declared by hand,
//! like the rest of poptop's FFI, and checked the same way.

// A test crate: every helper here runs inside a test, where a panic is the
// failure being reported.
#![allow(clippy::unwrap_used)]

mod common;

use common::{Home, log_a_sample, logged_day};
use std::ffi::{c_int, c_ulong, c_void};
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::process::{Child, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Winsize {
    rows: u16,
    cols: u16,
    xpixel: u16,
    ypixel: u16,
}

/// `struct termios`, as opaque bytes: only ever compared, never read. Larger
/// than either platform's (60 bytes on glibc, 72 on macOS) so neither writes
/// past it.
#[repr(C, align(8))]
#[derive(Clone, Copy, PartialEq)]
struct Termios([u8; 128]);

#[cfg_attr(target_os = "linux", link(name = "util"))]
unsafe extern "C" {
    fn openpty(
        amaster: *mut c_int,
        aslave: *mut c_int,
        name: *mut i8,
        termp: *const c_void,
        winp: *const Winsize,
    ) -> c_int;
}

unsafe extern "C" {
    fn ioctl(fd: c_int, req: c_ulong, ...) -> c_int;
    fn setsid() -> c_int;
    fn tcgetattr(fd: c_int, t: *mut Termios) -> c_int;
    fn kill(pid: c_int, sig: c_int) -> c_int;
}

#[cfg(target_os = "linux")]
const TIOCSWINSZ: c_ulong = 0x5414;
#[cfg(target_os = "linux")]
const TIOCSCTTY: c_ulong = 0x540E;
#[cfg(target_os = "macos")]
const TIOCSWINSZ: c_ulong = 0x8008_7467;
#[cfg(target_os = "macos")]
const TIOCSCTTY: c_ulong = 0x2000_7461;

const ENTER_ALT: &[u8] = b"\x1b[?1049h";
const LEAVE_ALT: &[u8] = b"\x1b[?1049l";
const SHOW_CURSOR: &[u8] = b"\x1b[?25h";

/// poptop on a pty, and everything it has written.
struct Tui {
    child: Child,
    master: std::fs::File,
    /// The terminal side, held open so the pty outlives the child until the
    /// reader has its last bytes.
    _slave: OwnedFd,
    /// Its settings before poptop touched them.
    before: Termios,
    seen: Arc<Mutex<Vec<u8>>>,
    _home: Home,
}

impl Tui {
    fn start(args: &[&str]) -> Tui {
        Tui::start_in(Home::new(), args, &[])
    }

    fn start_in(home: Home, args: &[&str], env: &[(&str, &str)]) -> Tui {
        let (mut m, mut s) = (-1, -1);
        let size = Winsize {
            rows: 40,
            cols: 120,
            ..Winsize::default()
        };
        // SAFETY: two out-parameters for the descriptors, no name buffer, no
        // settings (the defaults, which is what a terminal emulator gives),
        // and a size that outlives the call.
        let rc = unsafe {
            openpty(
                &mut m,
                &mut s,
                std::ptr::null_mut(),
                std::ptr::null(),
                &size,
            )
        };
        assert_eq!(rc, 0, "openpty: {}", std::io::Error::last_os_error());
        // SAFETY: both were just returned by openpty and are owned here alone.
        let (master, slave) = unsafe { (OwnedFd::from_raw_fd(m), OwnedFd::from_raw_fd(s)) };
        let before = settings(&master);

        let mut cmd = home.cmd(args);
        cmd.env("TERM", "xterm-256color")
            .stdin(Stdio::from(slave.try_clone().unwrap()))
            .stdout(Stdio::from(slave.try_clone().unwrap()))
            .stderr(Stdio::from(slave.try_clone().unwrap()));
        for (k, v) in env {
            cmd.env(k, v);
        }
        // SAFETY: runs in the forked child before exec, and calls only
        // `setsid` and `ioctl`, both async-signal-safe. A session of its own
        // and the pty as its controlling terminal are what a shell would give
        // it, and what makes a resize reach it as SIGWINCH.
        unsafe {
            cmd.pre_exec(|| {
                if setsid() < 0 || ioctl(0, TIOCSCTTY, 0) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let child = cmd.spawn().expect("cannot start poptop");

        let master = std::fs::File::from(master);
        let seen = Arc::new(Mutex::new(Vec::new()));
        let mut reader = master.try_clone().unwrap();
        let sink = seen.clone();
        std::thread::spawn(move || {
            let mut buf = [0u8; 8192];
            // Ends when the last holder of the terminal side closes it, or
            // with EIO on Linux once the child has gone: either way, done.
            while let Ok(n) = reader.read(&mut buf) {
                if n == 0 {
                    break;
                }
                sink.lock().unwrap().extend_from_slice(&buf[..n]);
            }
        });
        let mut t = Tui {
            child,
            master,
            _slave: slave,
            before,
            seen,
            _home: home,
        };
        t.wait_for(ENTER_ALT, "the alternate screen");
        // A frame, not just the escape that opens the screen.
        t.wait_for(b"CPU", "a first frame");
        t
    }

    fn output(&self) -> Vec<u8> {
        self.seen.lock().unwrap().clone()
    }

    /// Wait until `what` has been written, failing with what was.
    fn wait_for(&mut self, what: &[u8], called: &str) {
        let deadline = Instant::now() + Duration::from_secs(20);
        while Instant::now() < deadline {
            if find(&self.output(), what).is_some() {
                return;
            }
            if let Ok(Some(status)) = self.child.try_wait() {
                panic!(
                    "poptop exited ({status}) before {called}:\n{}",
                    String::from_utf8_lossy(&self.output())
                );
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let _ = self.child.kill();
        panic!(
            "no {called} in 20s:\n{}",
            String::from_utf8_lossy(&self.output())
        );
    }

    fn keys(&mut self, bytes: &[u8]) {
        self.master.write_all(bytes).unwrap();
        self.master.flush().unwrap();
    }

    fn resize(&mut self, cols: u16, rows: u16) {
        let size = Winsize {
            rows,
            cols,
            ..Winsize::default()
        };
        // SAFETY: the pty's master side, and a size that outlives the call.
        // The kernel sends SIGWINCH to the terminal's foreground group.
        let rc = unsafe { ioctl(self.master.as_raw_fd(), TIOCSWINSZ, &size) };
        assert_eq!(rc, 0, "resize: {}", std::io::Error::last_os_error());
    }

    /// How many bytes poptop has written so far: a frame drawn after this
    /// moment is output past it.
    fn mark(&self) -> usize {
        self.output().len()
    }

    /// Wait for more than `mark` bytes, which on a live monitor is a redraw.
    fn drawn_since(&mut self, mark: usize, called: &str) {
        let deadline = Instant::now() + Duration::from_secs(20);
        while self.output().len() <= mark {
            assert!(Instant::now() < deadline, "nothing drawn after {called}");
            assert!(
                self.child.try_wait().unwrap().is_none(),
                "poptop exited after {called}:\n{}",
                String::from_utf8_lossy(&self.output())
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn alive(&mut self) -> bool {
        self.child.try_wait().unwrap().is_none()
    }

    /// Wait for the exit, then require the terminal back as it was given.
    fn exits(mut self, code: Option<i32>, called: &str) -> Vec<u8> {
        let deadline = Instant::now() + Duration::from_secs(20);
        let status = loop {
            if let Some(s) = self.child.try_wait().unwrap() {
                break s;
            }
            if Instant::now() > deadline {
                let _ = self.child.kill();
                panic!("poptop did not exit after {called}");
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        assert_eq!(
            status.code(),
            code,
            "after {called}: {status} (signal {:?})\n{}",
            status.signal(),
            String::from_utf8_lossy(&self.output())
        );
        // The reader has the last bytes once the child's side has closed; a
        // moment for it to catch up.
        std::thread::sleep(Duration::from_millis(100));
        let out = self.output();
        assert!(
            settings(&self.master) == self.before,
            "after {called} the terminal was left in other settings (raw mode)"
        );
        let entered = rfind(&out, ENTER_ALT).expect("never entered the alternate screen");
        let left = rfind(&out, LEAVE_ALT);
        assert!(
            left.is_some_and(|l| l > entered),
            "after {called} the alternate screen was left open"
        );
        let hid = rfind(&out, b"\x1b[?25l").unwrap_or(0);
        assert!(
            rfind(&out, SHOW_CURSOR).is_some_and(|s| s > hid),
            "after {called} the cursor was left hidden"
        );
        out
    }
}

impl Drop for Tui {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// A terminal's settings, read through either side of the pty. The master
/// side answers with the terminal side's settings, and outlives the child: on
/// macOS the terminal side is revoked when its session leader exits.
fn settings(fd: &impl AsRawFd) -> Termios {
    let mut t = Termios([0; 128]);
    // SAFETY: an open terminal descriptor, and a buffer larger than either
    // platform's `struct termios`.
    let rc = unsafe { tcgetattr(fd.as_raw_fd(), &mut t) };
    assert_eq!(rc, 0, "tcgetattr: {}", std::io::Error::last_os_error());
    t
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

fn rfind(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).rposition(|w| w == needle)
}

#[test]
fn q_after_three_resizes_exits_0_and_gives_the_terminal_back() {
    let mut t = Tui::start(&[]);
    for (cols, rows) in [(60, 15), (200, 60), (30, 8)] {
        let mark = t.mark();
        t.resize(cols, rows);
        t.drawn_since(mark, &format!("a resize to {cols}x{rows}"));
    }
    t.keys(b"q");
    t.exits(Some(0), "q");
}

#[test]
fn ctrl_c_exits_0_and_gives_the_terminal_back() {
    let mut t = Tui::start(&[]);
    t.keys(b"\x03");
    t.exits(Some(0), "Ctrl-C");
}

#[test]
fn sigterm_exits_0_and_gives_the_terminal_back() {
    // What a service manager, `kill`, or a closing session sends. Without a
    // handler poptop died on the spot and left the shell in raw mode on the
    // alternate screen.
    let t = Tui::start(&[]);
    // SAFETY: a signal to our own child, which is still running.
    assert_eq!(unsafe { kill(t.child.id() as c_int, 15) }, 0);
    t.exits(Some(0), "SIGTERM");
}

#[test]
fn sighup_exits_and_gives_the_terminal_back() {
    // The terminal closing, as an ssh session dropping does.
    let t = Tui::start(&[]);
    // SAFETY: a signal to our own child, which is still running.
    assert_eq!(unsafe { kill(t.child.id() as c_int, 1) }, 0);
    t.exits(Some(0), "SIGHUP");
}

#[test]
fn a_panic_gives_the_terminal_back() {
    // Forced by a variable only a debug build reads, which the tests run.
    let t = Tui::start_in(Home::new(), &[], &[("POPTOP_PANIC_AFTER_FIRST_FRAME", "1")]);
    let out = t.exits(Some(101), "a panic");
    let text = String::from_utf8_lossy(&out);
    let message = text.find("panicked").expect("no panic message");
    // Printed after the screen was restored, where it can be read.
    let left = rfind(&out, LEAVE_ALT).unwrap();
    assert!(
        left < message,
        "the panic was printed on the alternate screen"
    );
}

#[test]
fn an_arrow_key_split_across_two_reads_is_still_an_arrow() {
    // Over a slow link the three bytes of Down arrive as ESC, then `[B`. Read
    // as Esc they would back out, and with nothing to back out of, quit.
    let mut t = Tui::start(&[]);
    // Two writes, so two reads: poptop is waiting in its input poll, which
    // wakes on the ESC alone.
    t.keys(b"\x1b");
    std::thread::sleep(Duration::from_millis(5));
    t.keys(b"[B");
    std::thread::sleep(Duration::from_millis(500));
    assert!(t.alive(), "a split Down arrow quit poptop");
    t.keys(b"q");
    t.exits(Some(0), "q");
}

#[test]
fn a_recorded_day_opens_on_a_terminal_and_q_leaves_it() {
    let home = Home::new();
    log_a_sample(&home);
    let day = logged_day(&home);
    let mut t = Tui::start_in(home, &["--read", &day], &[]);
    // The cursor opens on the oldest sample of the day, paused.
    t.wait_for(b"PAUSED", "the recorded day");
    t.keys(b"q");
    t.exits(Some(0), "q in a recorded day");
}
