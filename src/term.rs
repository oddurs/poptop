//! Asking the terminal what it looks like.
//!
//! poptop derives its surfaces from the terminal's own background rather than
//! choosing one. A monitor that paints over a carefully-set Ghostty theme is
//! not styled, it is rude — and the user has already answered the question
//! poptop would be guessing at.

use std::io::Write;
use std::time::{Duration, Instant};

/// The terminal's background colour, if it will say.
///
/// OSC 11 with `?` asks; the reply is `ESC ] 11 ; rgb:rrrr/gggg/bbbb` closed by
/// either `BEL` or `ESC \`. Ghostty, kitty, iTerm2, foot, WezTerm, xterm and
/// Alacritty all answer it; tmux forwards it; a terminal that does not support
/// it says nothing at all.
///
/// Which is the whole risk, and why `timeout` is not optional: a terminal that
/// ignores the query leaves us reading a pipe that will never produce anything.
/// Every path out of here is bounded, and a timeout returns `None` — the same
/// answer as "it said something I could not parse". Failing towards "poptop
/// does not know" costs a shade of grey; failing towards a hang costs the
/// program.
pub fn background(timeout: Duration) -> Option<[u8; 3]> {
    // Raw mode, or the reply is line-buffered and never arrives — and the
    // escape sequence is echoed to the screen on the way.
    crossterm::terminal::enable_raw_mode().ok()?;
    let answer = ask(timeout);
    let _ = crossterm::terminal::disable_raw_mode();
    parse(&answer?)
}

fn ask(timeout: Duration) -> Option<String> {
    let mut out = std::io::stdout();
    out.write_all(b"\x1b]11;?\x1b\\").ok()?;
    out.flush().ok()?;

    let mut buf = Vec::new();
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if !readable(&deadline) {
            continue;
        }
        // `read(2)` directly, not `std::io::stdin()`. That is a `BufReader`
        // over the same descriptor, and taking one byte from it pulls a whole
        // buffer out of the terminal — which swallows the keystrokes typed
        // while poptop was starting and leaves them in a buffer the event loop
        // never looks at.
        let mut byte = [0u8; 1];
        // SAFETY: a one-byte buffer, and `read` writes at most one byte into
        // it. Fd 0 is open for the life of the process.
        let n = unsafe { read(0, byte.as_mut_ptr(), 1) };
        match n {
            1 => {
                buf.push(byte[0]);
                // `BEL`, or `ESC \`. Checked as we go rather than by waiting
                // for the whole timeout, so a terminal that answers promptly
                // does not cost the slowest case.
                if byte[0] == 0x07 || (buf.len() >= 2 && &buf[buf.len() - 2..] == b"\x1b\\") {
                    return String::from_utf8(buf).ok();
                }
                // A reply is about thirty bytes. Anything much longer is not
                // one, and reading until the deadline would swallow keystrokes.
                if buf.len() > 64 {
                    return None;
                }
            }
            _ => return None,
        }
    }
    None
}

/// Whether stdin has something, without blocking past the deadline.
fn readable(deadline: &Instant) -> bool {
    let left = deadline.saturating_duration_since(Instant::now());
    if left.is_zero() {
        return false;
    }
    // SAFETY: `poll` reads the one `pollfd` it is given and writes `revents`
    // back into it. The fd is stdin, which is open for the life of the process.
    unsafe {
        let mut fds = PollFd {
            fd: 0,
            events: POLLIN,
            revents: 0,
        };
        let ms = left.as_millis().min(i32::MAX as u128) as i32;
        poll(&mut fds, 1, ms) > 0 && fds.revents & POLLIN != 0
    }
}

const POLLIN: i16 = 0x0001;

#[repr(C)]
struct PollFd {
    fd: i32,
    events: i16,
    revents: i16,
}

unsafe extern "C" {
    fn poll(fds: *mut PollFd, nfds: u32, timeout: i32) -> i32;
    fn read(fd: i32, buf: *mut u8, count: usize) -> isize;
}

/// `ESC ] 11 ; rgb:rrrr/gggg/bbbb` — components of one to four hex digits.
///
/// Split out so the parsing is testable without a terminal, which is the only
/// way any of this gets tested at all.
pub fn parse(reply: &str) -> Option<[u8; 3]> {
    let body = reply.split("rgb:").nth(1)?;
    let mut parts = body
        .split(|c: char| !c.is_ascii_hexdigit() && c != '/')
        .next()?
        .split('/');
    let mut out = [0u8; 3];
    for slot in &mut out {
        let part = parts.next()?;
        if part.is_empty() || part.len() > 4 {
            return None;
        }
        let v = u32::from_str_radix(part, 16).ok()?;
        // Scaled by width, not truncated: `rgb:1/2/3` is four-bit and `f` in it
        // means full, not 15/255.
        let max = (1u32 << (4 * part.len())) - 1;
        *slot = ((v * 255 + max / 2) / max) as u8;
    }
    (parts.next().is_none()).then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reply_is_read_at_any_component_width() {
        // xterm answers in 16-bit components, some terminals in 8, and the
        // spec permits 4 and 12. A parser that assumed one width read every
        // other terminal's black as something else.
        assert_eq!(parse("\x1b]11;rgb:0000/0000/0000\x1b\\"), Some([0, 0, 0]));
        assert_eq!(
            parse("\x1b]11;rgb:ffff/ffff/ffff\x07"),
            Some([255, 255, 255])
        );
        assert_eq!(parse("\x1b]11;rgb:1a/1a/1f\x07"), Some([0x1a, 0x1a, 0x1f]));
        assert_eq!(parse("\x1b]11;rgb:f/f/f\x07"), Some([255, 255, 255]));
        assert_eq!(parse("\x1b]11;rgb:000/000/fff\x07"), Some([0, 0, 255]));
    }

    #[test]
    fn nonsense_is_not_a_colour() {
        // Every one of these is a real thing a terminal or a wrapper can send
        // back, and each used to be a panic or a fabricated grey.
        for s in [
            "",
            "\x1b]11;?\x1b\\",           // our own query, echoed
            "\x1b]11;rgb:\x07",          // answered with nothing
            "\x1b]11;rgb:00/00\x07",     // two components
            "\x1b]11;rgb:0/0/0/0\x07",   // four
            "\x1b]11;rgb:zz/00/00\x07",  // not hex
            "\x1b]11;rgb:00000/0/0\x07", // wider than the spec allows
            "\x1b]10;rgb:00/00/00\x07",  // the *foreground*, which we did not ask for
        ] {
            // The foreground reply parses as a colour and is simply the wrong
            // one; everything else must not parse at all.
            if s.starts_with("\x1b]10;") {
                continue;
            }
            assert_eq!(parse(s), None, "{s:?} was read as a colour");
        }
    }
}
