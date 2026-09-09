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
//! block behind a length: an upgrade halfway through a day leaves the morning
//! readable, because every block carries its own schema. That is what the
//! format in `persist` was for.

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
        ((1..=12).contains(&date.month) && (1..=31).contains(&date.day)).then_some(date)
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
/// reader skip one it cannot decode and keep the rest of the day.
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
    let block = store::encode(samples);
    let len = u32::try_from(block.len()).map_err(|_| io::Error::other("block too large"))?;
    let mut f = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;
    // One write, not two. A length that reached the file without its block —
    // the disk filled between the calls — is a file every later reader stops
    // at, and the block after it is lost with it.
    let mut framed = Vec::with_capacity(LEN + block.len());
    framed.extend_from_slice(&len.to_le_bytes());
    framed.extend_from_slice(&block);
    f.write_all(&framed)?;
    f.flush()?;
    Ok(true)
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
    let mut notes = Vec::new();
    let path = dir.join(file_name(date));
    let Ok(bytes) = fs::read(&path) else {
        return (Vec::new(), notes);
    };
    let mut out = Vec::new();
    let mut at = 0usize;
    let mut skipped = 0usize;
    while at + LEN <= bytes.len() {
        let len = u32::from_le_bytes(bytes[at..at + LEN].try_into().unwrap()) as usize;
        let from = at + LEN;
        if len == 0 {
            // A run of zeroes: a sparse region, or a file the filesystem
            // extended and never filled. Named as what it is rather than
            // counted with the entries a different version wrote — that
            // message sends somebody chasing an upgrade that never happened.
            notes.push(format!(
                "{}: a run of empty bytes; the file was read only as far as it",
                file_name(date)
            ));
            break;
        }
        let Some(to) = from.checked_add(len).filter(|to| *to <= bytes.len()) else {
            // A block whose length runs past the end of the file: the write was
            // cut short. Everything before it is still good.
            notes.push(format!(
                "{}: the last entry was cut short and was skipped",
                file_name(date)
            ));
            break;
        };
        // `decode_reporting`, not `decode`: a block this build cannot read has
        // a reason, and the reader that throws away what it noticed is the
        // thing the store's own tests exist to stop.
        let (block, said) = store::decode_reporting(&bytes[from..to]);
        match block {
            Some(mut s) => out.append(&mut s),
            None => skipped += 1,
        }
        // Once each, however many blocks say the same thing: a day written
        // across an upgrade would otherwise repeat one sentence a hundred and
        // forty-four times.
        for note in said {
            if !notes.contains(&note) {
                notes.push(note);
            }
        }
        at = to;
    }
    if skipped > 0 {
        notes.push(format!(
            "{}: {skipped} entries could not be read, most likely written by a different version",
            file_name(date)
        ));
    }
    // Written in order and read in order, so this is already sorted — but a
    // day assembled from two poptops running at once is not, and the cursor
    // has to move forwards in time whatever wrote the file.
    out.sort_by_key(|s| s.at);
    (out, notes)
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
    Some(gaps[gaps.len() / 2])
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
