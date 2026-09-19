//! A history that outlives the process, addressable by date.
//!
//! The in-session buffer is what poptop's whole position rests on: you connect
//! to a box that is slow now, start poptop, and scrub back through the last ten
//! minutes with nothing having been running beforehand. atop gives you
//! twenty-eight days *if its daemon was running* and nothing at all if it was
//! not.
//!
//! These do not conflict, and the rule is written down because the temptation
//! to blur it is real: **poptop logs if it is left running, and works if it was
//! not.** The buffer is unchanged. The log is what accumulates when the tool
//! happens to have been up, it is off until somebody asks for it once, and no
//! figure poptop draws depends on it existing.
//!
//! One file a day, `poptop-YYYYMMDD`, in the same state directory as the
//! restart store. Each append is a complete, independently-decodable store
//! block behind a length, with a checksum after it: an upgrade halfway through
//! a day leaves the morning readable, because every block carries its own
//! schema. That is what the format in `persist` was for.

use crate::sample::Sample;
use crate::store;
use std::fs;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// A civil date, which is what a person means by "last Tuesday".
///
/// Local, not UTC. An operator asking what happened at 03:00 means 03:00 where
/// the machine is, and a log filed by UTC date would put half of one of their
/// nights in each of two files.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Date {
    pub year: i32,
    pub month: u32,
    pub day: u32,
}

impl std::fmt::Display for Date {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

impl Date {
    /// `YYYY-MM-DD`, and nothing else.
    ///
    /// Deliberately strict. A date parser that accepts `2026-9-8` and
    /// `26/09/08` has to decide what `03/04` means, and there is no answer to
    /// that which is right in both London and Chicago.
    pub fn parse(s: &str) -> Option<Date> {
        let mut parts = s.split('-');
        let (y, m, d) = (parts.next()?, parts.next()?, parts.next()?);
        if parts.next().is_some() || y.len() != 4 || m.len() != 2 || d.len() != 2 {
            return None;
        }
        let date = Date {
            year: y.parse().ok()?,
            month: m.parse().ok()?,
            day: d.parse().ok()?,
        };
        ((1..=12).contains(&date.month) && (1..=date.days_in_month()).contains(&date.day))
            .then_some(date)
    }

    /// How long this date's month is, by the proleptic Gregorian calendar.
    ///
    /// Checked here, at the parse, rather than by seeing where `mktime` lands:
    /// `2026-02-30` is not a date, and `--read` looking for a file called
    /// `poptop-20260230` and finding "nothing recorded" says the wrong thing
    /// about why.
    fn days_in_month(self) -> u32 {
        let leap = self.year.rem_euclid(4) == 0
            && (self.year.rem_euclid(100) != 0 || self.year.rem_euclid(400) == 0);
        match self.month {
            2 if leap => 29,
            2 => 28,
            4 | 6 | 9 | 11 => 30,
            _ => 31,
        }
    }
}

/// `struct tm`, as both platforms lay it out.
///
/// Declared here rather than pulled in with a crate, like every other bit of
/// FFI in poptop. The trailing two fields are BSD/glibc extensions that both
/// macOS and Linux carry; they are unread, and present so the struct is the
/// size the C library will write into.
#[repr(C)]
#[derive(Default)]
struct Tm {
    sec: i32,
    min: i32,
    hour: i32,
    mday: i32,
    mon: i32,
    year: i32,
    wday: i32,
    yday: i32,
    isdst: i32,
    gmtoff: i64,
    zone: *const i8,
}

unsafe extern "C" {
    fn localtime_r(time: *const i64, result: *mut Tm) -> *mut Tm;
    fn mktime(tm: *mut Tm) -> i64;
}

/// The local civil date of an instant.
///
/// `None` before the epoch, which is not a time any sample carries and not one
/// worth a branch elsewhere.
pub fn date_of(at: SystemTime) -> Option<Date> {
    let secs = at.duration_since(UNIX_EPOCH).ok()?.as_secs() as i64;
    let mut tm = Tm::default();
    // SAFETY: `localtime_r` writes into the caller's `struct tm` and reads a
    // `time_t` by pointer. Both are stack locals of the right layout, and the
    // reentrant form takes no global lock and returns no shared buffer.
    let ok = unsafe { localtime_r(&secs, &mut tm) };
    if ok.is_null() {
        return None;
    }
    Some(Date {
        year: tm.year + 1900,
        month: tm.mon as u32 + 1,
        day: tm.mday as u32,
    })
}

/// A moment somebody typed, resolved against the moment they typed it.
///
/// Two forms, because both are how the question is asked. An incident has a
/// time — `03:00` — and it also has a distance — `-2h`. Scrubbing to either by
/// pressing the left arrow six hundred times is not a workflow.
///
/// - `-2h`, `-30m`, `-90s`, `+5m` — relative to `now`.
/// - `03:00`, `03:00:15` — today, in local time.
/// - `2026-09-08 03:00` — a date and a time, in local time.
///
/// Local, not UTC, for the reason the daily file is: `03:00` means 03:00 where
/// the machine is. `Err` carries what to say, because a jump box that rejects
/// what you typed without saying which forms it takes is a box you type into
/// twice.
pub fn parse_when(text: &str, now: SystemTime) -> Result<SystemTime, String> {
    const FORMS: &str = "try -2h, 03:00, or 2026-09-08 03:00";
    let text = text.trim();
    if text.is_empty() {
        return Err(format!("nothing to jump to — {FORMS}"));
    }
    if let Some(sign) = text.chars().next().filter(|c| *c == '-' || *c == '+') {
        let span =
            parse_span(&text[1..]).ok_or_else(|| format!("`{text}` is not a span — {FORMS}"))?;
        return if sign == '-' {
            now.checked_sub(span)
                .ok_or_else(|| format!("{text} is before the epoch"))
        } else {
            now.checked_add(span)
                .ok_or_else(|| format!("{text} is past the end of time"))
        };
    }

    // An optional date, then a time. Split on whitespace rather than guessing
    // from the shape: `2026-09-08` alone is a date with no time, which is
    // midnight, and `03:00` alone is a time today.
    let (date, clock) = match text.split_once(char::is_whitespace) {
        Some((d, c)) => (Some(d), c.trim()),
        None if text.contains('-') => (Some(text), "00:00"),
        None => (None, text),
    };
    let date = match date {
        Some(d) => Date::parse(d).ok_or_else(|| format!("`{d}` is not a date — {FORMS}"))?,
        None => date_of(now).ok_or("this machine's clock is before the epoch")?,
    };
    let (h, m, s) =
        parse_clock(clock).ok_or_else(|| format!("`{clock}` is not a time — {FORMS}"))?;
    let at =
        at_local(date, h, m, s).ok_or_else(|| format!("`{text}` is before the epoch — {FORMS}"))?;
    // `mktime` normalises rather than rejects: `2026-02-30` would be 2 March,
    // and answering with a different day is exactly what "landing in a gap
    // says so" exists to prevent. So nothing reaches it that it would move to
    // another day by mistake — `Date::parse` knows how long each month is and
    // `parse_clock` how long a day is. What it still moves, it moves on
    // purpose: `24:00` to the next midnight, and `23:59:60`, a leap second
    // the C library has no room for, to the midnight a second after it.
    //
    // This used to be caught afterwards, by checking that the instant landed
    // on the date typed. That also caught the leap second, and blamed today's
    // date for it: "`2027-01-15` is not a date on the calendar".
    Ok(at)
}

/// `2h`, `30m`, `90s`, `1h30m` is not accepted — one unit, deliberately.
///
/// A span language that takes `1h30m` has to decide what `1h30` means, and the
/// jump box is one line with no room to explain. Two jumps are not a hardship.
fn parse_span(text: &str) -> Option<std::time::Duration> {
    let (digits, scale) = match text.as_bytes().last()? {
        b's' => (&text[..text.len() - 1], 1.0),
        b'm' => (&text[..text.len() - 1], 60.0),
        b'h' => (&text[..text.len() - 1], 3600.0),
        b'd' => (&text[..text.len() - 1], 86_400.0),
        _ => (text, 1.0),
    };
    let n: f64 = digits.trim().parse().ok()?;
    // A year, which is past anything a buffer or a day's log can hold and far
    // short of what `from_secs_f64` panics on.
    (n.is_finite() && (0.0..=366.0 * 86_400.0).contains(&n))
        .then(|| std::time::Duration::from_secs_f64(n * scale))
}

/// `03:00` or `03:00:15`, as hours, minutes and seconds.
fn parse_clock(text: &str) -> Option<(i32, i32, i32)> {
    let mut parts = text.split(':');
    let h: i32 = parts.next()?.trim().parse().ok()?;
    let m: i32 = parts.next()?.trim().parse().ok()?;
    let s: i32 = match parts.next() {
        Some(s) => s.trim().parse().ok()?,
        None => 0,
    };
    if parts.next().is_some() {
        return None;
    }
    // 24:00 is a real way to write the end of a day, and the C library
    // normalises it to the next midnight on purpose. `24:30` is not a
    // convention, it is a typo, and `mktime` would answer it with half past
    // midnight the following morning.
    let hour = (0..24).contains(&h) || (h == 24 && m == 0 && s == 0);
    // Sixty is a leap second, which is a real second in a real minute.
    (hour && (0..60).contains(&m) && (0..=60).contains(&s)).then_some((h, m, s))
}

/// A local wall-clock moment as an instant.
///
/// Through `mktime`, which is the only thing on either platform that knows
/// what the offset was on *that* date. Computing it as midnight plus seconds
/// would be an hour out for half the year, and would be an hour out in the
/// other direction on the two nights a year somebody is most likely to be
/// reading a log.
fn at_local(date: Date, hour: i32, min: i32, sec: i32) -> Option<SystemTime> {
    let mut tm = Tm {
        sec,
        min,
        hour,
        mday: date.day as i32,
        mon: date.month as i32 - 1,
        year: date.year - 1900,
        // "Work it out": whether summer time was in force on that date, which
        // is the whole reason this goes through the C library.
        isdst: -1,
        ..Tm::default()
    };
    // SAFETY: `mktime` reads and normalises the caller's `struct tm`, which is
    // a stack local of the right layout. It is not reentrant only with respect
    // to the timezone, which poptop never sets.
    let t = unsafe { mktime(&mut tm) };
    (t >= 0).then(|| UNIX_EPOCH + std::time::Duration::from_secs(t as u64))
}

/// The local wall clock at an instant, as hours, minutes and seconds.
///
/// The other half of [`date_of`], and local for the same reason: a report of
/// what happened at 03:00 is read by somebody who was asleep at 03:00 where the
/// machine is.
pub fn local_clock(at: SystemTime) -> Option<(u32, u32, u32)> {
    let secs = at.duration_since(UNIX_EPOCH).ok()?.as_secs() as i64;
    let mut tm = Tm::default();
    // SAFETY: as `date_of`. A stack local of the right layout, and the
    // reentrant form takes no lock and returns no shared buffer.
    if unsafe { localtime_r(&secs, &mut tm) }.is_null() {
        return None;
    }
    Some((tm.hour as u32, tm.min as u32, tm.sec as u32))
}

/// `03:04:05` in local time, or `??:??:??` for an instant the C library will
/// not place.
pub fn clock_string(at: SystemTime) -> String {
    match local_clock(at) {
        Some((h, m, s)) => format!("{h:02}:{m:02}:{s:02}"),
        None => "??:??:??".into(),
    }
}

/// Where the daily files live.
///
/// Beside the restart store rather than in `/var/log`: this is per-user state
/// that needs no privileges, and poptop is not a system service. A log in
/// `/var/log` would either need root or would silently not be written.
pub fn dir() -> Option<PathBuf> {
    store::path().map(|p| p.with_file_name("log"))
}

/// The file a date's samples belong in.
pub fn file_name(date: Date) -> String {
    format!("poptop-{:04}{:02}{:02}", date.year, date.month, date.day)
}

/// The date a file name is for, or `None` if it is not one of poptop's.
///
/// The inverse of [`file_name`], and used to decide what to prune — a rule that
/// deletes files has to be certain which files are its own.
pub fn date_in_name(name: &str) -> Option<Date> {
    let digits = name.strip_prefix("poptop-")?;
    if digits.len() != 8 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Date::parse(&format!(
        "{}-{}-{}",
        &digits[..4],
        &digits[4..6],
        &digits[6..]
    ))
}

/// How much of a day's file is a length header.
///
/// Four bytes, little-endian, before each block. A block is a complete store
/// image — magic, version, schema, samples — so the length is what lets a
/// reader skip one it cannot decode and keep the rest of the day. It counts
/// the checksum after the block too; see [`frame`].
const LEN: usize = 4;

/// Append samples to today's file, creating it if this is the first of the day.
///
/// One block per call. Each carries its own schema, which is what makes an
/// upgrade halfway through a day leave the morning readable — and what costs
/// about 1.5 KB a block, measured against a sample of twenty-odd.
///
/// Appended rather than rewritten, so a `kill -9` loses at most the interval
/// since the last append rather than the day. That is a different bargain from
/// the restart store's, which writes only on a clean exit — because a log
/// nobody asked for must not become a background writer, and a log somebody
/// did ask for is useless if the crash takes it.
pub fn append(dir: &Path, at: SystemTime, samples: &[&Sample], cap: u64) -> io::Result<bool> {
    if samples.is_empty() {
        return Ok(true);
    }
    let date = date_of(at).ok_or_else(|| io::Error::other("no local date for this sample"))?;
    fs::create_dir_all(dir)?;
    // The byte bound has to stop the *writing*, not only the keeping. Today's
    // file is never pruned — it is the history of the session that is running —
    // so without this a short `log-interval` fills the disk in a day and the
    // retention rule watches it happen. Measured on this laptop at 87 KB a
    // sample, a one-second interval is seven gigabytes a day.
    // Weighed against the whole directory, not against today's file alone. One
    // budget, one meaning: `log-bytes` is what poptop's logs may occupy, and a
    // cap that applied per-file while the retention rule applied across files
    // was two different limits wearing one name — a single day reaching the
    // budget would have made the next prune delete every other day at once.
    let path = dir.join(file_name(date));
    if total_bytes(dir) >= cap {
        return Ok(false);
    }
    let framed = frame(samples).ok_or_else(|| io::Error::other("block too large"))?;
    let mut f = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;
    // One write, not two. A length that reached the file without its block —
    // the disk filled between the calls — is a file every later reader stops
    // at, and the block after it is lost with it.
    f.write_all(&framed)?;
    f.flush()?;
    Ok(true)
}

/// One entry as it goes into a day's file: a length, a store block, and a
/// checksum of the block.
///
/// The checksum is inside the length, after the block, and that placement is
/// the whole of its compatibility story. A poptop from before it reads the
/// length, decodes the block, and never looks at the four bytes after — its
/// decoder does not ask to reach the end — so a newer log reads in an older
/// poptop as it did. A reader that knows about it checks it when it is there
/// and reads an entry without one the old way, so an older log reads in this
/// one.
///
/// What it is for is 0100: an entry cut short by a crash, whose missing bytes
/// the next torn write happened to supply. Without it such an entry decoded
/// and passed for whole, with somebody else's bytes in its last fields.
pub fn frame(samples: &[&Sample]) -> Option<Vec<u8>> {
    let block = store::encode(samples);
    let len = u32::try_from(block.len() + SUM).ok()?;
    let mut framed = Vec::with_capacity(LEN + block.len() + SUM);
    framed.extend_from_slice(&len.to_le_bytes());
    framed.extend_from_slice(&block);
    framed.extend_from_slice(&checksum(&block).to_le_bytes());
    Some(framed)
}

/// How much of an entry is its checksum.
const SUM: usize = 4;

/// A 32-bit checksum of an entry's block, against accidents rather than
/// adversaries: anyone who can write the file can write a matching sum.
///
/// A word at a time — a multiply and a rotate per eight bytes — because every
/// entry is summed on every read, and a bytewise CRC is slower than decoding
/// the entry it guards. Each step is a bijection of the running state for a
/// given word, so any single changed word changes the state, and the fold to
/// 32 bits leaves one chance in four billion of a torn entry passing.
fn checksum(bytes: &[u8]) -> u32 {
    const K: u64 = 0x9e37_79b9_7f4a_7c15;
    let mut h = (bytes.len() as u64) ^ K;
    let (words, rest) = bytes.as_chunks::<8>();
    for w in words {
        h = (h ^ u64::from_le_bytes(*w)).wrapping_mul(K).rotate_left(29);
    }
    let mut tail = [0u8; 8];
    tail[..rest.len()].copy_from_slice(rest);
    h = (h ^ u64::from_le_bytes(tail)).wrapping_mul(K);
    (h ^ (h >> 32)) as u32
}

/// What poptop's logs occupy, in bytes.
///
/// Only poptop's own files. A state directory somebody else also writes into is
/// not this tool's budget to spend.
pub fn total_bytes(dir: &Path) -> u64 {
    fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_name().to_str().and_then(date_in_name).is_some())
        .filter_map(|e| e.metadata().ok())
        .map(|m| m.len())
        .sum()
}

/// Every sample recorded on a date, oldest first.
///
/// A block that will not decode is skipped rather than fatal, and the reader
/// says how many it skipped: a day whose last write was cut short by a power
/// failure is still a day worth reading, and so is one written across an
/// upgrade that changed a field this build does not have.
pub fn read_day(dir: &Path, date: Date) -> (Vec<Sample>, Vec<String>) {
    let name = file_name(date);
    match store::read_regular(&dir.join(&name)) {
        Ok(Some(bytes)) => read_blocks(&bytes, &name),
        Ok(None) => (Vec::new(), Vec::new()),
        Err(why) => (Vec::new(), vec![format!("{name}: {why}")]),
    }
}

/// The samples in a day file's bytes, and what the reader had to say about
/// them. `name` is only for the notes.
///
/// Split from [`read_day`] so the framing can be tested — and fuzzed — without
/// a filesystem: this is the one reader in poptop whose input is a file
/// somebody else's crash may have left half-written.
///
/// Every entry written whole is read, whatever was torn before or after it,
/// and no torn one is; `fuzz/log_torn` holds that as a property. The one
/// exception is an entry written before checksums: it can pass for whole when
/// the torn write after it supplies exactly the bytes it was missing and they
/// decode, and nothing in it tells the two apart.
pub fn read_blocks(bytes: &[u8], name: &str) -> (Vec<Sample>, Vec<String>) {
    let mut notes = Vec::new();
    let mut out = Vec::new();
    let mut at = 0usize;
    let mut skipped = 0usize;
    let mut torn = 0usize;
    let mut zeroes = false;
    // The entry read last, until something shows whether it was whole.
    let mut last: Option<Last> = None;
    loop {
        let step = if at + LEN <= bytes.len() {
            step(bytes, at)
        } else {
            Step::End
        };
        let failed = match step {
            Step::Read { to, samples, said } => {
                last = Some(Last {
                    start: at,
                    end: to,
                    before: out.len(),
                    foreign: false,
                });
                out.extend(samples);
                note_once(&mut notes, said);
                at = to;
                continue;
            }
            Step::Foreign { to, said } => {
                last = Some(Last {
                    start: at,
                    end: to,
                    before: out.len(),
                    foreign: true,
                });
                skipped += 1;
                note_once(&mut notes, said);
                at = to;
                continue;
            }
            failed => failed,
        };
        // Something here could not be read, or the day ended. Either way,
        // first: was the entry before it whole? A write cut short with more
        // appended after it still carries the length it meant to have, and the
        // fragment can borrow enough of the next entry to decode exactly — into
        // a sample whose last fields are somebody else's. If it did, a whole
        // entry starts inside the span it claimed, and reading it lands here,
        // in the middle of that entry. So its samples go back and reading
        // resumes at the entry it swallowed.
        //
        // Checked here, when something has gone wrong, rather than for every
        // entry: a day with no crash in it — nearly every day — then pays
        // nothing for the check, where scanning each entry up front cost more
        // than decoding it.
        if let Some(prev) = last.take()
            && let Some(inner) = next_block(bytes, prev.start).filter(|n| *n < prev.end)
        {
            out.truncate(prev.before);
            if prev.foreign {
                skipped -= 1;
            }
            torn += 1;
            at = inner;
            continue;
        }
        match failed {
            Step::End => break,
            // A run of zeroes: a sparse region, or a file the filesystem
            // extended and never filled. Named as what it is rather than
            // counted with the entries a different version wrote — that
            // message sends somebody chasing an upgrade that never happened.
            // It says nothing about what follows it.
            Step::Zeroes => zeroes = true,
            _ => torn += 1,
        }
        // The length here could not be trusted, so the next entry is found
        // by what it starts with instead.
        match next_block(bytes, at) {
            Some(next) => at = next,
            None => break,
        }
    }
    if zeroes {
        notes.push(format!(
            "{name}: a run of empty bytes; the entries either side of it were read"
        ));
    }
    if torn > 0 {
        notes.push(if torn == 1 {
            format!("{name}: an entry was cut short and was skipped")
        } else {
            format!("{name}: {torn} entries were cut short and were skipped")
        });
    }
    if skipped > 0 {
        notes.push(format!(
            "{name}: {skipped} entries could not be read, most likely written by a different version"
        ));
    }
    // Written in order and read in order, so this is already sorted — but a
    // day assembled from two poptops running at once is not, and the cursor
    // has to move forwards in time whatever wrote the file.
    out.sort_by_key(|s| s.at);
    (out, notes)
}

/// The entry `read_blocks` read last, kept until the next step shows whether
/// it was whole.
struct Last {
    start: usize,
    end: usize,
    /// How many samples had been read before it, so its own can be taken back.
    before: usize,
    /// Counted as another version's rather than read.
    foreign: bool,
}

/// What is at one offset of a day file.
enum Step {
    /// An entry that decoded, using exactly the bytes its length names.
    Read {
        to: usize,
        samples: Vec<Sample>,
        said: Vec<String>,
    },
    /// An entry that would not decode but is framed like one — the next entry
    /// starts where it ends — so a different version wrote it.
    Foreign { to: usize, said: Vec<String> },
    /// A length of zero.
    Zeroes,
    /// A length that runs past the end of the file, or one whose bytes neither
    /// decode nor end where another entry starts: a fragment.
    Torn,
    /// Fewer bytes left than a length takes.
    End,
}

fn step(bytes: &[u8], at: usize) -> Step {
    let len = u32::from_le_bytes(bytes[at..at + LEN].try_into().unwrap()) as usize;
    let from = at + LEN;
    if len == 0 {
        return Step::Zeroes;
    }
    let Some(to) = from.checked_add(len).filter(|to| *to <= bytes.len()) else {
        return Step::Torn;
    };
    // `decode_exactly`, not `decode_reporting`: the block must end on the byte
    // its length names. A fragment that borrowed bytes from the next entry can
    // still pass that — see the caller — but most cannot.
    //
    // An entry whose last four bytes are its block's checksum is read as the
    // block before them; one without is an entry from before checksums, read
    // whole. A torn entry fails the sum, and then fails as an old one too:
    // its block would have to decode to four bytes longer than it is.
    let span = &bytes[from..to];
    let body = match span.split_last_chunk::<SUM>() {
        Some((block, sum)) if checksum(block) == u32::from_le_bytes(*sum) => block,
        _ => span,
    };
    let (block, said) = store::decode_exactly(body);
    match block {
        Some(samples) => Step::Read { to, samples, said },
        None if boundary(bytes, to) => Step::Foreign { to, said },
        None => Step::Torn,
    }
}

/// Each note once, however many entries say it: a day written across an
/// upgrade would otherwise repeat one sentence a hundred and forty-four times.
fn note_once(notes: &mut Vec<String>, said: Vec<String>) {
    for note in said {
        if !notes.contains(&note) {
            notes.push(note);
        }
    }
}

/// Whether a whole entry starts at `at`: a length that fits in the file, and a
/// store behind it — magic, version and a schema block this build can parse.
///
/// Strict, because every use of it moves the reader: resuming after a torn
/// entry, and deciding that a length is lying because an entry starts inside
/// it. The magic alone was not strict enough. The store writes each string as
/// a length and its bytes, so a process named `poptophist` was "a length, then
/// the magic" inside a whole entry, and the reader threw the real entry away as
/// torn — which let anyone who can name a process erase a stretch of the log.
fn starts_block(bytes: &[u8], at: usize) -> bool {
    let Some(header) = bytes.get(at..at + LEN) else {
        return false;
    };
    let len = u32::from_le_bytes(header.try_into().unwrap()) as usize;
    let from = at + LEN;
    len > 0
        && from.checked_add(len).is_some_and(|to| to <= bytes.len())
        && store::starts_store(&bytes[from..from + len])
}

/// Whether something that looks like the start of an entry is at `at`, whole
/// or cut short: a length, then the magic, or as much of it as the file has.
///
/// Loose, because it moves nothing: it only decides what to call an entry that
/// would not decode — one from a different version, or a fragment.
fn looks_like_block(bytes: &[u8], at: usize) -> bool {
    let Some(header) = bytes.get(at..at + LEN) else {
        return false;
    };
    let after = &bytes[at + LEN..];
    let magic = store::MAGIC;
    header != [0; LEN]
        && (after.starts_with(magic) || (after.len() < magic.len() && magic.starts_with(after)))
}

/// Whether an entry could end at `at` — whether what follows is the end of
/// the file, or the start of another entry, whole or not.
///
/// Not a run of zeroes, though one can follow an entry. Payloads are full of
/// zero bytes, so a reader that had lost its place took four of them for a
/// boundary, called what it was reading another version's entry, and carried
/// on from wherever that entry's length pointed.
fn boundary(bytes: &[u8], at: usize) -> bool {
    let rest = &bytes[at..];
    if rest.len() < LEN || looks_like_block(bytes, at) {
        // The end of the file, a length cut short in the writing, or the next
        // entry.
        return true;
    }
    // An entry cut short inside its length or its magic, with a later append
    // straight after it: another entry starts within those first few bytes,
    // and whatever of the magic came before it is the magic.
    let magic = store::MAGIC;
    (1..LEN + magic.len()).any(|j| {
        let partial = bytes.get(at + LEN..at + j).unwrap_or_default();
        looks_like_block(bytes, at + j) && magic.starts_with(partial)
    })
}

/// The first framed block that starts after the one at `bad`, or `None` if the
/// rest of the file holds none.
///
/// Found by its magic rather than by any length, because the length is what
/// could not be trusted. It can start inside `bad`'s own length, when that is
/// all a crash left of it. A block's payload can contain the magic — any
/// string can — and [`starts_block`] is what tells that apart from an entry.
///
/// Only whole entries: one cut short is not somewhere reading can resume, and
/// it is found as a fragment when the entry before it is read.
fn next_block(bytes: &[u8], bad: usize) -> Option<usize> {
    let magic = store::MAGIC;
    let mut look = bad + 1 + LEN;
    while look + magic.len() <= bytes.len() {
        // The first byte, then the rest: every entry's payload is scanned once
        // by the check that no entry starts inside it, and comparing ten bytes
        // at every offset made that a third of the cost of reading a day.
        let found = bytes[look..=bytes.len() - magic.len()]
            .iter()
            .position(|b| *b == magic[0])?;
        let at = look + found;
        if bytes[at..].starts_with(magic) && starts_block(bytes, at - LEN) {
            return Some(at - LEN);
        }
        look = at + 1;
    }
    None
}

/// The dates a log directory holds, newest first.
pub fn days(dir: &Path) -> Vec<Date> {
    let mut days: Vec<Date> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| date_in_name(e.file_name().to_str()?))
        .collect();
    days.sort_unstable_by(|a, b| b.cmp(a));
    days
}

/// Which files a retention rule would delete, newest kept first.
///
/// Bounded by **both** age and bytes, and the bytes bound is the one that
/// matters: a sample carries a whole process table, so a build box with four
/// thousand processes writes twenty times what a laptop does at the same
/// settings, and a rule in days alone would be a different rule on every
/// machine.
///
/// Split from the deleting so the rule can be tested without a filesystem —
/// this is the one function in poptop that removes a user's data.
pub fn to_prune(files: &[(Date, u64)], keep_days: u32, max_bytes: u64, today: Date) -> Vec<Date> {
    let mut by_age: Vec<(Date, u64)> = files.to_vec();
    by_age.sort_unstable_by_key(|(date, _)| std::cmp::Reverse(*date));
    let mut drop = Vec::new();
    let mut kept = 0u64;
    // Once the budget is spent it stays spent. Advancing `kept` only for the
    // files that fitted made the rule non-monotonic in age: after one large
    // day was dropped, a smaller older one still fitted and survived, leaving
    // retention with a hole in it — and the user told the older file was past
    // a limit the newer one had already broken.
    let mut full = false;
    for (date, size) in by_age {
        // Today is never pruned, however large it is or however the clock has
        // moved: it is the file being written, and deleting it would take the
        // history of the session that is running.
        if date == today {
            kept = kept.saturating_add(size);
            continue;
        }
        // Days, not files. Counting positions made `log-days = 7` mean "the
        // seven newest files", which on a machine that runs poptop
        // occasionally keeps one from last year and expires nothing.
        // `log-days = 7` keeps seven days: today and the six before it. A
        // file exactly `keep_days` old is the eighth.
        let too_old = days_between(date, today) >= keep_days as i64;
        full |= kept.saturating_add(size) > max_bytes;
        if too_old || full {
            drop.push(date);
        } else {
            kept = kept.saturating_add(size);
        }
    }
    drop
}

/// Whole days from `then` to `now`, by the calendar.
///
/// Civil arithmetic, not clock arithmetic: both ends are dates already, so this
/// needs no timezone and cannot be wrong on the night a clock changes.
fn days_between(then: Date, now: Date) -> i64 {
    days_from_civil(now) - days_from_civil(then)
}

/// Days from 1970-01-01 to a date, by Howard Hinnant's `days_from_civil`.
///
/// The standard algorithm rather than one worked out here: it is exact for
/// every proleptic Gregorian date, leap years and centuries included, and a
/// month-length table written by hand is the kind of code that is wrong once
/// every four hundred years.
fn days_from_civil(d: Date) -> i64 {
    let (y, m, day) = (d.year as i64, d.month as i64, d.day as i64);
    let y = y - i64::from(m <= 2);
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Apply the retention rule, and say what went.
///
/// Reports rather than logs silently: deleting somebody's history without
/// saying so is the failure mode this whole feature has to avoid.
pub fn prune(dir: &Path, keep_days: u32, max_bytes: u64, today: Date) -> Vec<String> {
    let files: Vec<(Date, u64)> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let date = date_in_name(e.file_name().to_str()?)?;
            Some((date, e.metadata().ok()?.len()))
        })
        .collect();
    let mut notes = Vec::new();
    for date in to_prune(&files, keep_days, max_bytes, today) {
        match fs::remove_file(dir.join(file_name(date))) {
            Ok(()) => notes.push(format!(
                "dropped the log for {date}, past the retention limit"
            )),
            Err(e) => notes.push(format!("could not drop the log for {date}: {e}")),
        }
    }
    notes
}

/// A day, ready to be scrubbed, or the reason there is not one.
///
/// The whole of what `--read` does before the buffer exists, in one place so it
/// can be tested without a terminal: which file, whether it had anything in it,
/// and what is worth saying about what came back.
/// The spacing of a recorded day, as the median gap between its samples.
///
/// A replayed buffer is not on the live interval — entries are ten minutes
/// apart by default, not one second — and almost everything downstream is
/// scaled by it: the timeline draws a seam wherever a gap exceeds twice the
/// nominal interval, the growth column refuses to divide by an unknown span,
/// and the panel title says how much time is buffered. Left at the live
/// interval a replayed day is drawn as nothing but seams and labelled as two
/// minutes of history.
///
/// The median, not the mean: a day with one four-hour gap in it — poptop was
/// not running — is still a ten-minute log, and averaging would call it an
/// hourly one.
pub fn spacing(samples: &[Sample]) -> Option<std::time::Duration> {
    let mut gaps: Vec<std::time::Duration> = samples
        .windows(2)
        .filter_map(|w| w[1].at.duration_since(w[0].at).ok())
        .filter(|d| !d.is_zero())
        .collect();
    if gaps.is_empty() {
        return None;
    }
    gaps.sort_unstable();
    // The lower middle where the count is even. With two gaps — three samples,
    // one of them after a hole — the upper middle *is* the hole, and a day
    // recorded every second would be called a two-hour log because it was
    // interrupted once.
    Some(gaps[(gaps.len() - 1) / 2])
}

pub fn open_day(dir: &Path, date: Date) -> Result<(Vec<Sample>, Vec<String>), String> {
    let (samples, mut notes) = read_day(dir, date);
    if samples.is_empty() {
        return Err(format!(
            "nothing recorded on {date}. `poptop --days` lists what there is"
        ));
    }
    // A day that spans a reboot is kept whole rather than trimmed — it is the
    // history that was asked for. Said out loud because a process is identified
    // by pid and start time, and start time only means anything within one
    // boot: two unrelated programs either side of the restart can share a pid,
    // and the `HISTORY` column would draw them as one line.
    let boots = samples
        .iter()
        .fold(Vec::new(), |mut seen: Vec<SystemTime>, s| {
            let b = store::boot_time(s);
            if !seen.iter().any(|x| store::same_boot(*x, b)) {
                seen.push(b);
            }
            seen
        });
    if boots.len() > 1 {
        notes.push(format!(
            "{date} spans {} boots: a process is identified by pid and start time, \
             and start time only means anything within one boot, so a history line \
             may join two unrelated programs across the restart",
            boots.len()
        ));
    }
    Ok((samples, notes))
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::sample::Sample;
    use std::sync::Arc;

    fn at(secs: u64) -> SystemTime {
        UNIX_EPOCH + std::time::Duration::from_secs(secs)
    }

    /// A sample carrying one distinguishable figure, which is all these tests
    /// need to tell one from another after a round trip.
    fn sample(secs: u64, cpu: f32) -> Sample {
        Sample {
            at: at(secs),
            cpu_total: cpu,
            ..Sample::unknown()
        }
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("poptop-log-{name}"));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn a_day_is_appended_to_and_read_back_whole() {
        let dir = scratch("append");
        // Three writes, as three separate blocks. This is the shape that
        // matters: a log is appended to while poptop runs, so a reader has to
        // reassemble a day from however many times it was written to.
        let day = at(1_800_000_000);
        for (i, cpu) in [11.0f32, 22.0, 33.0].into_iter().enumerate() {
            append(
                &dir,
                day,
                &[&sample(1_800_000_000 + i as u64, cpu)],
                u64::MAX,
            )
            .unwrap();
        }
        let date = date_of(day).unwrap();
        let (back, notes) = read_day(&dir, date);
        assert!(notes.is_empty(), "{notes:?}");
        let cpus: Vec<f32> = back.iter().map(|s| s.cpu_total).collect();
        assert_eq!(cpus, [11.0, 22.0, 33.0], "a day did not come back whole");
        assert_eq!(days(&dir), [date]);

        // A day nothing was written on is empty rather than an error.
        let (none, notes) = read_day(&dir, Date { day: 1, ..date });
        assert!(none.is_empty() && notes.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_byte_bound_stops_the_writing_and_not_only_the_keeping() {
        // Today's file is never pruned — it is the running session's history —
        // so a bound that only decides what to *keep* watches a short interval
        // fill the disk in a single day and does nothing about it.
        let dir = scratch("cap");
        let day = at(1_800_000_000);
        let date = date_of(day).unwrap();
        assert!(append(&dir, day, &[&sample(1_800_000_000, 11.0)], 1 << 30).unwrap());
        let size = fs::metadata(dir.join(file_name(date))).unwrap().len();

        assert!(
            !append(&dir, day, &[&sample(1_800_000_001, 22.0)], size).unwrap(),
            "the day was at its limit and was written to anyway"
        );
        assert_eq!(
            fs::metadata(dir.join(file_name(date))).unwrap().len(),
            size,
            "a refused append still grew the file"
        );
        // What was already written is still readable, and is what it was.
        let (back, _) = read_day(&dir, date);
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].cpu_total, 11.0);

        // And it starts again once there is room.
        assert!(append(&dir, day, &[&sample(1_800_000_002, 33.0)], size + 1).unwrap());
        assert_eq!(read_day(&dir, date).0.len(), 2);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_write_cut_short_costs_the_last_entry_and_not_the_day() {
        // A power failure between the length and the block, or in the middle
        // of one. Everything written before it is still a recorded day, and
        // losing the morning because the evening was interrupted would be the
        // worst possible failure for a feature whose point is the morning.
        let dir = scratch("truncated");
        let day = at(1_800_000_000);
        let date = date_of(day).unwrap();
        for (i, cpu) in [11.0f32, 22.0].into_iter().enumerate() {
            append(
                &dir,
                day,
                &[&sample(1_800_000_000 + i as u64, cpu)],
                u64::MAX,
            )
            .unwrap();
        }
        let path = dir.join(file_name(date));
        let whole = fs::read(&path).unwrap();

        // Every cut through the second block, and a bare length with nothing
        // after it. None of them may lose the first sample or panic.
        // From the first byte of the second block's payload: before that the
        // cut lands inside its length header, and a header with nothing after
        // it is not an interrupted entry, it is no entry.
        let second = LEN + u32::from_le_bytes(whole[..LEN].try_into().unwrap()) as usize + LEN;
        for cut in second..whole.len() {
            fs::write(&path, &whole[..cut]).unwrap();
            let (back, notes) = read_day(&dir, date);
            assert_eq!(
                back.first().map(|s| s.cpu_total),
                Some(11.0),
                "a cut at {cut} lost the entry before it"
            );
            assert_eq!(
                back.len(),
                1,
                "a cut at {cut} produced a whole second entry"
            );
            // Named for what happened. Decoding the fragment as far as it goes
            // and calling it "a different version of poptop" would send
            // somebody chasing an upgrade that never happened.
            assert!(
                notes.iter().any(|n| n.contains("cut short")),
                "a cut at {cut} was not reported as one: {notes:?}"
            );
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_write_cut_short_does_not_cost_the_entries_appended_after_it() {
        // The sequence a crash actually produces: poptop dies partway through
        // an append, is started again, and keeps appending to the same day.
        // The torn entry's length still says how long it meant to be, so a
        // reader that trusts it lands in the middle of whatever came next, and
        // from there every length it reads is noise — the afternoon was lost
        // to a crash in the morning. Every cut through the middle entry must
        // cost that entry and nothing either side of it.
        let dir = scratch("torn");
        let day = at(1_800_000_000);
        let date = date_of(day).unwrap();
        // In both framings: logs from before checksums are still read.
        for block in [legacy as fn(f32) -> Vec<u8>, framed] {
            let (first, torn, last) = (block(11.0), block(22.0), block(33.0));
            let path = dir.join(file_name(date));
            fs::create_dir_all(&dir).unwrap();
            for cut in 1..torn.len() {
                let mut day = first.clone();
                day.extend_from_slice(&torn[..cut]);
                day.extend_from_slice(&last);
                fs::write(&path, &day).unwrap();
                let (back, notes) = read_day(&dir, date);
                let cpus: Vec<f32> = back.iter().map(|s| s.cpu_total).collect();
                assert_eq!(
                    cpus,
                    [11.0, 33.0],
                    "a cut at {cut} of the middle entry, of {}: {notes:?}",
                    torn.len()
                );
                assert!(
                    notes.iter().any(|n| n.contains("cut short")),
                    "a cut at {cut} was not reported as one: {notes:?}"
                );
                assert!(
                    !notes.iter().any(|n| n.contains("different version")),
                    "a cut at {cut} was blamed on an upgrade: {notes:?}"
                );
            }
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_date_is_checked_against_the_length_of_its_month() {
        for ok in [
            "2026-01-31",
            "2024-02-29",
            "2000-02-29",
            "2026-04-30",
            "2026-12-31",
        ] {
            assert!(Date::parse(ok).is_some(), "{ok} was refused");
        }
        for bad in [
            "2026-02-29",
            "2100-02-29",
            "2026-02-30",
            "2026-04-31",
            "2026-06-31",
        ] {
            assert!(Date::parse(bad).is_none(), "{bad} was accepted");
        }
    }

    #[test]
    fn a_leap_second_is_a_time_and_not_an_error_about_the_date() {
        // `parse_clock` accepts `:60` on purpose. It used to be refused after
        // the fact, with an error blaming the date it was typed on.
        let now = at(1_800_000_000);
        let leap = parse_when("23:59:60", now).expect("a leap second was refused");
        let midnight = parse_when("24:00", now).unwrap();
        assert_eq!(leap, midnight, "23:59:60 is the second before 00:00:01");
    }

    #[test]
    fn a_day_that_is_not_a_regular_file_is_refused_rather_than_read() {
        // A day's name pointing at `/dev/zero` read until memory ran out, and
        // one pointing at a FIFO waited for a writer forever. Neither is a file
        // poptop wrote; both are refused with a reason.
        let dir = scratch("not-a-file");
        fs::create_dir_all(&dir).unwrap();
        let date = Date::parse("2026-09-18").unwrap();
        let path = dir.join(file_name(date));
        std::os::unix::fs::symlink("/dev/zero", &path).unwrap();
        let (back, notes) = read_day(&dir, date);
        assert!(back.is_empty());
        assert!(
            notes.iter().any(|n| n.contains("not a regular file")),
            "{notes:?}"
        );
        fs::remove_file(&path).unwrap();

        let made = std::process::Command::new("mkfifo").arg(&path).status();
        if made.is_ok_and(|s| s.success()) {
            let (back, notes) = read_day(&dir, date);
            assert!(back.is_empty());
            assert!(
                notes.iter().any(|n| n.contains("not a regular file")),
                "{notes:?}"
            );
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_log_directory_that_is_a_symlink_is_followed_and_pruning_stays_inside_it() {
        // Pointing the log at a bigger disk with a symlink is a reasonable
        // thing to do, so the directory is followed. A day file that is itself
        // a symlink is read through, but pruning removes the link, never what
        // it points at: the one function that deletes has to stay inside the
        // directory it was given.
        let real = scratch("symlink-real");
        let outside = scratch("symlink-outside");
        let link = scratch("symlink-link");
        fs::create_dir_all(&real).unwrap();
        fs::create_dir_all(&outside).unwrap();
        std::os::unix::fs::symlink(&real, &link).unwrap();

        let day = at(1_800_000_000);
        append(&link, day, &[&sample(1_800_000_000, 11.0)], u64::MAX).unwrap();
        let date = date_of(day).unwrap();
        assert_eq!(
            read_day(&link, date).0.len(),
            1,
            "the linked directory was not used"
        );

        // An old day that is a link to a file outside the directory.
        let victim = outside.join("keep-me");
        fs::write(&victim, b"not poptop's").unwrap();
        let old = Date::parse("2020-01-01").unwrap();
        std::os::unix::fs::symlink(&victim, link.join(file_name(old))).unwrap();
        let _ = prune(&link, 1, u64::MAX, date);
        assert!(
            victim.exists(),
            "pruning deleted a file outside the log directory"
        );
        assert!(
            !link.join(file_name(old)).exists(),
            "the expired link itself was not pruned"
        );
        for d in [&real, &outside, &link] {
            let _ = fs::remove_dir_all(d);
            let _ = fs::remove_file(d);
        }
    }

    #[test]
    fn a_log_that_cannot_be_written_is_an_error_and_not_a_crash() {
        // EACCES: a state directory somebody made read-only. `append` says so
        // and returns, and the caller turns it into one note — `run` keeps a
        // note it has already made out of the exit lines, so a full disk at
        // 10:00 is said once and not every interval until the tool is closed.
        use std::os::unix::fs::PermissionsExt;
        let dir = scratch("read-only");
        fs::create_dir_all(&dir).unwrap();
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o500)).unwrap();
        let day = at(1_800_000_000);
        let got = append(&dir, day, &[&sample(1_800_000_000, 1.0)], u64::MAX);
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).unwrap();
        // Root ignores the mode bits, so under root there is nothing to test.
        let root = std::process::Command::new("id")
            .arg("-u")
            .output()
            .is_ok_and(|o| o.stdout.trim_ascii() == b"0");
        if !root {
            assert!(got.is_err(), "a read-only directory took a write: {got:?}");
        }

        // ENOSPC: `/dev/full` fails every write with it, which is a full disk
        // without having to fill one. Linux only; macOS has no such device.
        if std::path::Path::new("/dev/full").exists() {
            let date = date_of(day).unwrap();
            // Root's write above went through, and left a file in the way.
            let _ = fs::remove_file(dir.join(file_name(date)));
            std::os::unix::fs::symlink("/dev/full", dir.join(file_name(date))).unwrap();
            let got = append(&dir, day, &[&sample(1_800_000_000, 1.0)], u64::MAX);
            let err = got.expect_err("a full disk took a write");
            assert_eq!(err.raw_os_error(), Some(28), "{err}");
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn two_poptops_logging_to_one_directory_lose_nothing() {
        // Two terminals, both with the log on. Each append is one write to a
        // file opened for appending, so the kernel puts each whole at the end;
        // the test is that nothing interleaves inside an entry and nothing is
        // lost, however the two race.
        let dir = scratch("two-writers");
        let day = at(1_800_000_000);
        let writers: Vec<_> = (0..2)
            .map(|w| {
                let dir = dir.clone();
                std::thread::spawn(move || {
                    for i in 0..50 {
                        let cpu = (w * 100 + i) as f32;
                        append(
                            &dir,
                            day,
                            &[&sample(1_800_000_000 + i as u64, cpu)],
                            u64::MAX,
                        )
                        .unwrap();
                    }
                })
            })
            .collect();
        for w in writers {
            w.join().unwrap();
        }
        let (back, notes) = read_day(&dir, date_of(day).unwrap());
        assert_eq!(back.len(), 100, "{notes:?}");
        assert!(notes.is_empty(), "{notes:?}");
        let _ = fs::remove_dir_all(&dir);
    }

    /// What people type into the jump box, and the edges of each form.
    const WHEN: &[&str] = &[
        "-2h",
        "+5m",
        "-90s",
        "-1d",
        "03:00",
        "03:00:15",
        "24:00",
        "23:59:60",
        "2026-09-08 03:00",
        "2026-09-08",
        "0000-01-01 00:00",
        "9999-12-31 23:59:59",
        "1969-12-31 23:59",
        "2026-02-29",
        "-366d",
        "-367d",
        "-1e308h",
        "+nan s",
    ];

    #[test]
    fn nothing_typed_into_the_jump_box_can_panic() {
        // The C library is on the other side of `mktime`, and a year it cannot
        // place, or a span `Duration` cannot hold, has to come back as an
        // error rather than an abort.
        let nows = [
            at(0),
            at(1_800_000_000),
            UNIX_EPOCH + std::time::Duration::from_secs(253_402_300_799),
        ];
        for text in WHEN {
            for v in crate::mangle::text_variants(text, 200) {
                for now in nows {
                    let _ = parse_when(&v, now);
                }
            }
        }
    }

    #[test]
    fn a_jump_error_quotes_what_was_typed() {
        // Except the one about the machine's own clock, every refusal names
        // the part of the input it could not read.
        let now = at(1_800_000_000);
        for text in WHEN {
            for v in crate::mangle::text_variants(text, 200) {
                let Err(e) = parse_when(&v, now) else {
                    continue;
                };
                let Some((q, _)) = e.split_once('`').and_then(|(_, r)| r.split_once('`')) else {
                    // Unquoted, so it must say what it is about in words.
                    assert!(
                        e.contains(v.trim()) || e.contains("clock"),
                        "{v:?} was refused without saying why: {e}"
                    );
                    continue;
                };
                assert!(
                    v.contains(q),
                    "{v:?} was refused quoting `{q}`, not in it: {e}"
                );
            }
        }
    }

    #[test]
    fn a_torn_entry_after_a_whole_one_does_not_cost_the_whole_one() {
        // Found by `fuzz/log_torn`. A whole entry, then one torn after the
        // first byte of its length, then one torn right after its magic. The
        // entry after the whole one is not whole, so "the next entry starts
        // here" had to accept a torn start as well — it used to require a
        // whole one, and dropped the only entry that was.
        let block = |cpu: f32| {
            let b = store::encode(&[&sample(1_800_000_000, cpu)]);
            let mut framed = (b.len() as u32).to_le_bytes().to_vec();
            framed.extend(b);
            framed
        };
        let mut day = block(0.0);
        day.extend_from_slice(&block(1.0)[..1]);
        day.extend_from_slice(&block(2.0)[..LEN + store::MAGIC.len()]);
        let (back, notes) = read_blocks(&day, "poptop-test");
        let cpus: Vec<f32> = back.iter().map(|s| s.cpu_total).collect();
        assert_eq!(cpus, [0.0], "{notes:?}");
    }

    #[test]
    fn a_whole_entry_followed_by_many_crashes_is_still_read() {
        // Found by `fuzz/log_torn`: a whole entry, then five appends each cut
        // after one byte, then whole entries again. What follows an entry is no
        // evidence about it — any number of crashes can leave fragments there —
        // and requiring a clean start right after it dropped the entry.
        let block = |cpu: f32| {
            let b = store::encode(&[&sample(1_800_000_000, cpu)]);
            let mut framed = (b.len() as u32).to_le_bytes().to_vec();
            framed.extend(b);
            framed
        };
        let mut day = block(0.0);
        for i in 1..=5 {
            day.extend_from_slice(&block(i as f32)[..1]);
        }
        day.extend_from_slice(&block(6.0));
        day.extend_from_slice(&block(7.0));
        let (back, notes) = read_blocks(&day, "poptop-test");
        let cpus: Vec<f32> = back.iter().map(|s| s.cpu_total).collect();
        assert_eq!(cpus, [0.0, 6.0, 7.0], "{notes:?}");
    }

    /// What finding its place costs the reader on a day with nothing wrong
    /// in it, against decoding the same entries with their lengths trusted.
    /// `cargo test --release -- --ignored --nocapture measure_reading_a_day`.
    ///
    /// Measured when the resync went in: checking every entry up front for
    /// another entry starting inside it cost 28.5ms against 7.8ms of decoding
    /// for twelve megabytes. Checking only when a step fails, or the day ends,
    /// brought it to 9.3ms against 8.7ms. The checksum is summed on every
    /// entry, so it is in the reader's figure and not in the other: 11.3ms
    /// against 7.9ms when it went in, about four gigabytes a second.
    #[test]
    #[ignore = "measurement"]
    fn measure_reading_a_day() {
        let s = crate::store::tests_support::big_sample(5.0, 400);
        let mut day = Vec::new();
        for _ in 0..300 {
            day.extend(frame(&[&s]).unwrap());
        }
        let bare = |day: &[u8]| {
            let mut m = 0;
            let mut at = 0;
            while at + LEN <= day.len() {
                let len = u32::from_le_bytes(day[at..at + LEN].try_into().unwrap()) as usize;
                m += store::decode_reporting(&day[at + LEN..at + LEN + len - SUM])
                    .0
                    .unwrap()
                    .len();
                at += LEN + len;
            }
            m
        };
        // Alternated and the best of seven, because whichever runs second
        // gets the warm cache.
        let (mut reader, mut decode) = (std::time::Duration::MAX, std::time::Duration::MAX);
        for _ in 0..7 {
            let t = std::time::Instant::now();
            assert_eq!(read_blocks(&day, "x").0.len(), 300);
            reader = reader.min(t.elapsed());
            let t = std::time::Instant::now();
            assert_eq!(bare(&day), 300);
            decode = decode.min(t.elapsed());
        }
        eprintln!(
            "{} MB: reader {reader:?}, decoding alone {decode:?}",
            day.len() >> 20
        );
    }

    #[test]
    fn a_process_named_after_the_magic_cannot_erase_the_log() {
        // Entries are found by their magic, `poptophist`, when a length cannot
        // be trusted — and the store writes each string as a length and its
        // bytes. So a process named `poptophist` put "a length, then the
        // magic" inside a whole entry, the reader took it for an entry starting
        // there, and threw the real one away as torn. Anyone can name a
        // process; that was a way to erase a stretch of somebody's log.
        //
        // What follows the magic is the format version, whose top bytes are
        // zero. No string poptop stores can contain a zero byte — names,
        // command lines, mounts and users all come from C strings — so a
        // string cannot pass for an entry.
        let block = |cpu: f32| {
            let mut s = sample(1_800_000_000, cpu);
            s.procs = vec![crate::sample::ProcSample {
                name: Arc::from("poptophist"),
                cmd: Some(Arc::from("poptophist poptophist")),
                ..crate::store::tests_support::big_sample(1.0, 1).procs[0].clone()
            }];
            let b = store::encode(&[&s]);
            let mut framed = (b.len() as u32).to_le_bytes().to_vec();
            framed.extend(b);
            framed
        };
        let mut day = block(1.0);
        day.extend(block(2.0));
        let (back, notes) = read_blocks(&day, "poptop-test");
        let cpus: Vec<f32> = back.iter().map(|s| s.cpu_total).collect();
        assert_eq!(cpus, [1.0, 2.0], "{notes:?}");
        assert!(notes.is_empty(), "{notes:?}");
    }

    /// An entry framed as poptop framed them before checksums: a length and
    /// a store block, nothing after. Logs written then are still read.
    fn legacy(cpu: f32) -> Vec<u8> {
        let b = store::encode(&[&sample(1_800_000_000, cpu)]);
        let mut framed = (b.len() as u32).to_le_bytes().to_vec();
        framed.extend(b);
        framed
    }

    /// An entry framed as `append` frames it now.
    fn framed(cpu: f32) -> Vec<u8> {
        frame(&[&sample(1_800_000_000, cpu)]).unwrap()
    }

    #[test]
    fn a_torn_entry_filled_out_by_the_next_torn_write_is_caught_by_its_checksum() {
        // 0100, found by `fuzz/log_torn`: an entry cut one byte short, then a
        // later append cut one byte in. That byte completes the first entry's
        // length exactly, the bytes decode, and the whole entry after starts
        // where the first said it would end. Without a checksum nothing in the
        // file says the first was torn, and it was read with a stranger's byte
        // in its last field.
        for cut in [1, 2, 4, 5, 13, 14, 100] {
            let first = framed(0.0);
            let mut day = first[..first.len() - cut].to_vec();
            day.extend_from_slice(&framed(1.0)[..cut]);
            day.extend_from_slice(&framed(2.0));
            let (back, notes) = read_blocks(&day, "poptop-test");
            let cpus: Vec<f32> = back.iter().map(|s| s.cpu_total).collect();
            assert_eq!(cpus, [2.0], "{cut} bytes short: {notes:?}");
        }
    }

    #[test]
    fn a_torn_entry_from_before_checksums_is_still_a_known_limit() {
        // The same case in an entry written before checksums, which has none
        // to fail. Pinned so the limit stays a known one: what must hold is
        // the whole entry after it.
        let first = legacy(0.0);
        let mut day = first[..first.len() - 1].to_vec();
        day.extend_from_slice(&legacy(1.0)[..1]);
        day.extend_from_slice(&legacy(2.0));
        let (back, _) = read_blocks(&day, "poptop-test");
        let cpus: Vec<f32> = back.iter().map(|s| s.cpu_total).collect();
        assert!(cpus.ends_with(&[2.0]), "the whole entry was lost: {cpus:?}");
        assert!(
            !cpus.contains(&1.0),
            "a one-byte fragment was read: {cpus:?}"
        );
    }

    #[test]
    fn a_log_with_checksums_reads_in_a_poptop_from_before_them() {
        // The reader poptop shipped before checksums, verbatim in what matters:
        // trust the length, decode what it spans, ignore anything the decoder
        // did not reach. A downgrade must not lose the days the newer version
        // wrote.
        let old_reader = |day: &[u8]| {
            let mut out = Vec::new();
            let mut at = 0;
            while at + LEN <= day.len() {
                let len = u32::from_le_bytes(day[at..at + LEN].try_into().unwrap()) as usize;
                let (block, _) = store::decode_reporting(&day[at + LEN..at + LEN + len]);
                out.extend(block.expect("an older poptop could not read the entry"));
                at += LEN + len;
            }
            out
        };
        let mut day = framed(1.0);
        day.extend(framed(2.0));
        let cpus: Vec<f32> = old_reader(&day).iter().map(|s| s.cpu_total).collect();
        assert_eq!(cpus, [1.0, 2.0]);
    }

    #[test]
    fn a_day_written_across_the_upgrade_to_checksums_reads_whole() {
        // The morning in the old framing, the afternoon in the new.
        let mut day = legacy(1.0);
        day.extend(legacy(2.0));
        day.extend(framed(3.0));
        day.extend(framed(4.0));
        let (back, notes) = read_blocks(&day, "poptop-test");
        let cpus: Vec<f32> = back.iter().map(|s| s.cpu_total).collect();
        assert_eq!(cpus, [1.0, 2.0, 3.0, 4.0], "{notes:?}");
        assert!(notes.is_empty(), "{notes:?}");
    }

    #[test]
    fn every_byte_of_an_entry_is_in_its_checksum() {
        let block = store::encode(&[&sample(1_800_000_000, 1.0)]);
        let sum = checksum(&block);
        for i in 0..block.len() {
            for bit in [0x01, 0x80] {
                let mut bent = block.clone();
                bent[i] ^= bit;
                assert_ne!(checksum(&bent), sum, "byte {i} is not in the checksum");
            }
        }
        // And its length: a block that is a prefix of another sums apart.
        assert_ne!(checksum(&block[..block.len() - 1]), sum);
        assert_ne!(checksum(&[0; 7]), checksum(&[0; 8]));
    }

    #[test]
    fn entries_after_a_run_of_empty_bytes_are_still_read() {
        // A hole the filesystem left, with a later append after it. The zeroes
        // say nothing about what follows them.
        let dir = scratch("zeros-then-more");
        let day = at(1_800_000_000);
        let date = date_of(day).unwrap();
        append(&dir, day, &[&sample(1_800_000_000, 11.0)], u64::MAX).unwrap();
        let path = dir.join(file_name(date));
        let mut bytes = fs::read(&path).unwrap();
        bytes.extend_from_slice(&[0u8; 64]);
        fs::write(&path, &bytes).unwrap();
        append(&dir, day, &[&sample(1_800_000_001, 22.0)], u64::MAX).unwrap();

        let (back, notes) = read_day(&dir, date);
        let cpus: Vec<f32> = back.iter().map(|s| s.cpu_total).collect();
        assert_eq!(cpus, [11.0, 22.0], "{notes:?}");
        assert!(notes.iter().any(|n| n.contains("empty bytes")), "{notes:?}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_entry_from_another_version_costs_that_entry_and_not_the_day() {
        // The reason each append carries its own schema. An upgrade halfway
        // through a day must leave the morning readable — a log nobody can
        // read after upgrading is worse than no log.
        let dir = scratch("version");
        let day = at(1_800_000_000);
        let date = date_of(day).unwrap();
        append(&dir, day, &[&sample(1_800_000_000, 11.0)], u64::MAX).unwrap();
        let path = dir.join(file_name(date));
        let good = fs::read(&path).unwrap();

        // A second block whose version byte is not this one's.
        let mut alien = good.clone();
        alien[LEN + crate::store::MAGIC.len()] =
            alien[LEN + crate::store::MAGIC.len()].wrapping_add(1);
        let mut both = good.clone();
        both.extend_from_slice(&alien);
        both.extend_from_slice(&good);
        fs::write(&path, &both).unwrap();

        let (back, notes) = read_day(&dir, date);
        assert_eq!(
            back.len(),
            2,
            "a readable entry was lost with an unreadable one"
        );
        assert!(
            notes.iter().any(|n| n.contains("different version")),
            "an unreadable entry was skipped silently: {notes:?}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn retention_is_bounded_by_days_and_by_bytes() {
        let today = Date {
            year: 2026,
            month: 9,
            day: 10,
        };
        let d = |day| Date { day, ..today };
        let files: Vec<(Date, u64)> = (1..=10).map(|day| (d(day), 100)).collect();

        // Days alone: the newest three survive, and today is one of them.
        let dropped = to_prune(&files, 3, u64::MAX, today);
        assert_eq!(dropped, [d(7), d(6), d(5), d(4), d(3), d(2), d(1)]);

        // Days on the calendar, not files in a list. A machine that runs
        // poptop occasionally has gaps, and counting positions made
        // `log-days = 3` mean "the three newest files" — which keeps one from
        // last year and expires nothing.
        let year = |y| Date {
            year: y,
            month: 9,
            day: 10,
        };
        let sparse = [(today, 100), (year(2025), 100), (year(2024), 100)];
        assert_eq!(
            to_prune(&sparse, 3, u64::MAX, today),
            [year(2025), year(2024)],
            "files from previous years were kept because there were only three"
        );

        // And the rule is monotonic in age: once the budget is spent, every
        // older file goes. Advancing the running total only for the files that
        // fitted left retention with a hole in it — a large day dropped and a
        // small older one kept, and the older one told it was past a limit the
        // newer one had already broken.
        let lopsided = [(today, 1), (d(9), 300), (d(8), 10)];
        assert_eq!(
            to_prune(&lopsided, u32::MAX, 200, today),
            [d(9), d(8)],
            "an older file survived a newer one being dropped for size"
        );

        // Bytes alone: a rule in days is a different rule on every machine,
        // because a sample carries a whole process table. 250 bytes holds two
        // of these.
        let dropped = to_prune(&files, u32::MAX, 250, today);
        assert_eq!(dropped.len(), 8, "{dropped:?}");
        assert!(!dropped.contains(&d(10)) && !dropped.contains(&d(9)));

        // Today is never pruned, however large. It is the file being written,
        // and dropping it would take the history of the running session.
        let huge = vec![(today, u64::MAX), (d(9), 1)];
        assert_eq!(to_prune(&huge, u32::MAX, 10, today), [d(9)]);
        assert_eq!(to_prune(&huge, 0, 10, today), [d(9)]);

        // A file whose name is not poptop's is never a candidate, whatever the
        // rule says: this is the one function here that deletes a user's data.
        assert_eq!(date_in_name("poptop-2026090"), None);
        assert_eq!(date_in_name("poptop-2026091a"), None);
        assert_eq!(date_in_name("history"), None);
        assert_eq!(date_in_name("poptop-20261301"), None, "month 13");
    }

    #[test]
    fn pruning_deletes_only_what_the_rule_named() {
        let dir = scratch("prune");
        fs::create_dir_all(&dir).unwrap();
        let today = Date {
            year: 2026,
            month: 9,
            day: 10,
        };
        // The four days ending today, so the rule under test is the age rule
        // rather than "everything here is from last week".
        for day in 7..=10 {
            fs::write(dir.join(file_name(Date { day, ..today })), b"x".repeat(100)).unwrap();
        }
        // Files that are not poptop's, in poptop's directory. The second is
        // eight digits with no prefix — the shape the name check would let
        // through if it looked only at what follows a prefix it did not
        // require. This is the one place in poptop that deletes a user's data.
        fs::write(dir.join("notes.txt"), b"keep me").unwrap();
        fs::write(dir.join("20260901"), b"keep me too").unwrap();

        let notes = prune(&dir, 2, u64::MAX, today);
        assert_eq!(notes.len(), 2, "{notes:?}");
        assert!(notes.iter().all(|n| n.contains("retention limit")));
        assert_eq!(
            days(&dir),
            [Date { day: 10, ..today }, Date { day: 9, ..today }]
        );
        assert!(
            dir.join("notes.txt").exists() && dir.join("20260901").exists(),
            "a file poptop did not write was deleted"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn opening_a_day_says_what_it_could_not_promise() {
        let dir = scratch("open");
        let day = at(1_800_000_000);
        let date = date_of(day).unwrap();

        // A day nothing was recorded on is a reason, not an empty buffer: a
        // TUI opened on nothing has nothing to scrub and no way to say why.
        assert!(open_day(&dir, date).is_err());

        // One boot: whole, and nothing to say about it.
        let mut s = sample(1_800_000_000, 11.0);
        s.uptime = std::time::Duration::from_secs(1_000);
        append(&dir, day, &[&s], u64::MAX).unwrap();
        let mut s2 = sample(1_800_000_060, 22.0);
        s2.uptime = std::time::Duration::from_secs(1_060);
        append(&dir, day, &[&s2], u64::MAX).unwrap();
        let (back, notes) = open_day(&dir, date).unwrap();
        assert_eq!(back.len(), 2);
        assert!(notes.is_empty(), "{notes:?}");

        // A reboot inside the day is kept whole and said out loud — a process
        // is identified by pid and start time, and the pid on either side of a
        // restart is not the same process.
        let mut after = sample(1_800_000_120, 33.0);
        after.uptime = std::time::Duration::from_secs(5);
        append(&dir, day, &[&after], u64::MAX).unwrap();
        let (back, notes) = open_day(&dir, date).unwrap();
        assert_eq!(back.len(), 3, "the day was trimmed rather than explained");
        assert!(
            notes.iter().any(|n| n.contains("spans 2 boots")),
            "a reboot inside a recorded day passed unmentioned: {notes:?}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_moment_is_typed_as_a_distance_or_as_a_time() {
        // Both, because both are how the question is asked: an incident has a
        // time and it also has a distance.
        let now = at(1_800_003_600);
        let secs = |r: Result<SystemTime, String>| {
            r.unwrap().duration_since(UNIX_EPOCH).unwrap().as_secs()
        };

        assert_eq!(secs(parse_when("-1h", now)), 1_800_000_000);
        assert_eq!(secs(parse_when("-30m", now)), 1_800_001_800);
        assert_eq!(secs(parse_when("-90s", now)), 1_800_003_510);
        assert_eq!(secs(parse_when("-2d", now)), 1_800_003_600 - 172_800);
        assert_eq!(secs(parse_when("+5m", now)), 1_800_003_900);
        // A bare number is seconds, as everywhere else in poptop.
        assert_eq!(secs(parse_when("-45", now)), 1_800_003_555);

        // A clock time is today, in local time — whatever local is here.
        let today = date_of(now).unwrap();
        assert_eq!(
            parse_when("03:00", now).unwrap(),
            at_local(today, 3, 0, 0).unwrap()
        );
        assert_eq!(
            parse_when("03:00:15", now).unwrap(),
            at_local(today, 3, 0, 15).unwrap()
        );
        // With a date, that date.
        let other = Date {
            year: 2026,
            month: 3,
            day: 4,
        };
        assert_eq!(
            parse_when("2026-03-04 03:00", now).unwrap(),
            at_local(other, 3, 0, 0).unwrap()
        );
        // A date alone is its midnight, which is a moment somebody may well
        // mean — "the start of that day".
        assert_eq!(
            parse_when("2026-03-04", now).unwrap(),
            at_local(other, 0, 0, 0).unwrap()
        );

        // Local, not UTC, and through the C library: the offset on a date is
        // not a constant, and the two nights a year it changes are exactly the
        // ones somebody is most likely to be reading a log.
        let summer = at_local(
            Date {
                year: 2026,
                month: 7,
                day: 1,
            },
            12,
            0,
            0,
        )
        .unwrap();
        let winter = at_local(
            Date {
                year: 2026,
                month: 1,
                day: 1,
            },
            12,
            0,
            0,
        )
        .unwrap();
        let noon_to_noon = winter.duration_since(UNIX_EPOCH).unwrap().as_secs() as i64
            - summer.duration_since(UNIX_EPOCH).unwrap().as_secs() as i64;
        // Both are local noon, so the gap is a whole number of days give or
        // take one hour of summer time — never an arbitrary offset.
        let slack = (noon_to_noon.rem_euclid(86_400)).min(86_400 - noon_to_noon.rem_euclid(86_400));
        assert!(
            slack == 0 || slack == 3_600,
            "local noon was not local noon: {noon_to_noon}s apart"
        );
    }

    #[test]
    fn a_typed_time_is_the_time_the_clock_showed() {
        // `03:00` in July and `03:00` in January are different offsets from
        // UTC wherever summer time is observed, and `mktime` is the only thing
        // on either platform that knows which. Asserted as a round trip rather
        // than against a fixed offset, because the answer depends on the zone
        // this happens to run in.
        //
        // On a machine set to UTC — CI is — both branches agree and this
        // cannot fail. It is still the property, and it holds anywhere a zone
        // with summer time is configured.
        for month in 1..=12 {
            let d = Date {
                year: 2026,
                month,
                day: 15,
            };
            let at = at_local(d, 3, 0, 0).expect("a real moment");
            assert_eq!(
                super::local_clock(at),
                Some((3, 0, 0)),
                "03:00 in month {month} came back as another time"
            );
            assert_eq!(date_of(at), Some(d), "03:00 landed on another day");
        }
    }

    #[test]
    fn a_jump_box_that_rejects_says_what_it_takes() {
        // A one-line box has nowhere else to teach its forms, and one that
        // rejects what you typed without saying which forms it accepts is one
        // you type into twice.
        let now = at(1_800_003_600);
        for bad in [
            "",
            "  ",
            "tuesday",
            "3pm",
            "-2y",
            "25:00",
            "03:60",
            "1:2:3:4",
            "-",
            "03",
            // `mktime` normalises rather than rejects, so these used to answer
            // a question nobody asked: 30 February is 2 March and `24:30` is
            // half past midnight the next morning — reported against the text
            // that was typed, so the wrong answer read as a real one.
            "2026-02-30 03:00",
            "2025-02-29 03:00",
            "2026-04-31 03:00",
            "24:30",
        ] {
            let why = parse_when(bad, now).expect_err(&format!("`{bad}` was accepted"));
            assert!(
                why.contains("-2h") || why.contains("not a date on the calendar"),
                "`{bad}` was rejected without saying what is accepted: {why}"
            );
        }
        // …and the forms that do work are not rejected, including the leap day
        // of a year that has one and the last day of a thirty-one-day month.
        for good in [
            "-2h",
            "+5m",
            "03:00",
            "24:00",
            "2026-09-08 03:00",
            "2026-09-08",
            "2024-02-29 03:00",
            "2026-01-31 03:00",
        ] {
            assert!(parse_when(good, now).is_ok(), "`{good}` was rejected");
        }
    }

    #[test]
    fn the_calendar_is_the_real_calendar() {
        let d = |year, month, day| Date { year, month, day };
        assert_eq!(days_between(d(2026, 9, 8), d(2026, 9, 10)), 2);
        assert_eq!(days_between(d(2026, 9, 10), d(2026, 9, 10)), 0);
        // Backwards, for a file dated in the future — a clock that was wrong
        // when it was written is not a reason to delete it.
        assert_eq!(days_between(d(2026, 9, 12), d(2026, 9, 10)), -2);

        // Month lengths, and the ones that are the whole reason this is
        // Hinnant's algorithm rather than arithmetic worked out here.
        assert_eq!(days_between(d(2026, 1, 31), d(2026, 2, 1)), 1);
        assert_eq!(days_between(d(2025, 12, 31), d(2026, 1, 1)), 1);
        // 2024 is a leap year: February has 29 days.
        assert_eq!(days_between(d(2024, 2, 28), d(2024, 3, 1)), 2);
        // 2026 is not.
        assert_eq!(days_between(d(2026, 2, 28), d(2026, 3, 1)), 1);
        // 2100 is a century that is not a leap year; 2000 was.
        assert_eq!(days_between(d(2100, 2, 28), d(2100, 3, 1)), 1);
        assert_eq!(days_between(d(2000, 2, 28), d(2000, 3, 1)), 2);
        // A whole year, and a leap one.
        assert_eq!(days_between(d(2025, 1, 1), d(2026, 1, 1)), 365);
        assert_eq!(days_between(d(2024, 1, 1), d(2025, 1, 1)), 366);
    }

    #[test]
    fn a_run_of_empty_bytes_is_not_a_version_mismatch() {
        // A sparse region, or a file the filesystem extended and never filled.
        // Counted with the entries a different version wrote, it sends
        // somebody chasing an upgrade that never happened — the same
        // misdiagnosis the truncation case exists to avoid.
        let dir = scratch("zeros");
        let day = at(1_800_000_000);
        let date = date_of(day).unwrap();
        append(&dir, day, &[&sample(1_800_000_000, 11.0)], u64::MAX).unwrap();
        let path = dir.join(file_name(date));
        let mut bytes = fs::read(&path).unwrap();
        bytes.extend_from_slice(&[0u8; 64]);
        fs::write(&path, &bytes).unwrap();

        let (back, notes) = read_day(&dir, date);
        assert_eq!(back.len(), 1, "the entry before the zeroes was lost");
        assert!(
            notes.iter().any(|n| n.contains("empty bytes")),
            "a run of zeroes was reported as something else: {notes:?}"
        );
        assert!(
            !notes.iter().any(|n| n.contains("different version")),
            "a run of zeroes was blamed on an upgrade: {notes:?}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_recorded_days_spacing_is_the_median_and_not_the_mean() {
        // Everything downstream is scaled by the interval: the timeline draws
        // a seam past twice it, the growth column will not divide by an
        // unknown span, and the panel title says how much is buffered. A day
        // replayed at the live interval is drawn as nothing but seams.
        let ten = |n: u64| sample(1_800_000_000 + n * 600, 1.0);
        let day: Vec<Sample> = (0..6).map(ten).collect();
        assert_eq!(spacing(&day), Some(std::time::Duration::from_secs(600)));

        // A day with a four-hour hole in it — poptop was not running — is
        // still a ten-minute log. The mean would call it an hourly one.
        let mut gapped: Vec<Sample> = (0..5).map(ten).collect();
        gapped.push(sample(1_800_000_000 + 5 * 600 + 14_400, 1.0));
        assert_eq!(spacing(&gapped), Some(std::time::Duration::from_secs(600)));

        // One sample has no spacing, and neither has none.
        assert_eq!(spacing(&day[..1]), None);
        assert_eq!(spacing(&[]), None);
    }

    #[test]
    fn a_date_round_trips_through_a_file_name() {
        let d = Date {
            year: 2026,
            month: 9,
            day: 8,
        };
        assert_eq!(file_name(d), "poptop-20260908");
        assert_eq!(date_in_name("poptop-20260908"), Some(d));
        assert_eq!(d.to_string(), "2026-09-08");
    }
}
