//! Collection on its own thread, on its own schedule.
//!
//! The interactive loop used to call the collector between key polls, so for
//! as long as a sample took the keys queued unread and the frame on screen was
//! the last one drawn. That was already a visible stall on a machine with a few
//! thousand processes, and every source added to the collector lengthened it:
//! reading the temperature sensors alone costs 40ms on a ten-core Mac (0229).
//!
//! So the sampler owns the collector, the schedule and the log, and the
//! interface owns the screen. They meet at a channel: finished samples one
//! way, what to gather the other. Neither waits for the other.

use crate::collect::{Collector, Needs};
use crate::log;
use crate::sample::Sample;
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Where and how often the log is written, when it is written at all.
///
/// `None` is the default and the whole point: poptop logs if it is left running
/// and works if it was not, so the ordinary run writes nothing.
#[derive(Clone)]
pub struct Logging {
    pub dir: std::path::PathBuf,
    pub every: Duration,
    pub days: u32,
    pub bytes: u64,
}

/// One sample, and what taking it involved.
pub struct Tick {
    pub sample: io::Result<Sample>,
    /// How long collection took, for the budget. The log write is not in it:
    /// the budget decides which sources to give up, and no source can make an
    /// append faster.
    pub took: Duration,
    /// `Some` when this sample was due to be logged: `None` inside it if the
    /// write went as asked, or the sentence the reader needs if it did not.
    pub logged: Option<Option<String>>,
    /// What retention had to say, when the date changed and it ran.
    pub pruned: Vec<String>,
}

/// The handle the interface holds. Dropping it stops the thread.
pub struct Sampler {
    rx: Receiver<Tick>,
    needs: Arc<Mutex<Needs>>,
    due: Arc<Mutex<Instant>>,
    stop: Arc<AtomicBool>,
}

/// How often a wait between samples looks at the stop flag.
const STOP_CHECK: Duration = Duration::from_millis(100);

impl Sampler {
    /// Start sampling every `interval`, the first one `interval` from now.
    ///
    /// The collector moves to the sampler thread and does not come back. The
    /// thread is not joined on the way out: quitting while a slow sample is in
    /// flight should not have to wait for it, and a log entry cut off by the
    /// process ending is one the reader already recovers from (0145).
    pub fn start<C>(
        mut collector: C,
        interval: Duration,
        needs: Needs,
        logging: Option<Logging>,
    ) -> Self
    where
        C: Collector + Send + 'static,
    {
        let (tx, rx) = mpsc::channel();
        let needs = Arc::new(Mutex::new(needs));
        let stop = Arc::new(AtomicBool::new(false));
        let mut schedule = Schedule::new(interval, Instant::now(), SystemTime::now());
        let due = Arc::new(Mutex::new(schedule.next));
        let (t_needs, t_due, t_stop) = (needs.clone(), due.clone(), stop.clone());

        std::thread::Builder::new()
            .name("sampler".into())
            .spawn(move || {
                // The first sample reaches the log immediately rather than one
                // logging interval in. A poptop left running for nine minutes
                // and killed would otherwise have recorded nothing at all,
                // which is the case somebody who asked for a log is least
                // willing to forgive.
                let mut next_log = Instant::now();
                let mut last_pruned: Option<log::Date> = None;
                loop {
                    set(&t_due, schedule.next);
                    while let Some(left) = schedule.next.checked_duration_since(Instant::now()) {
                        if t_stop.load(Ordering::Relaxed) {
                            return;
                        }
                        std::thread::sleep(left.min(STOP_CHECK));
                    }
                    if t_stop.load(Ordering::Relaxed) {
                        return;
                    }
                    let needs = *t_needs.lock().unwrap_or_else(|p| p.into_inner());
                    let t0 = Instant::now();
                    let sample = collector.sample(needs);
                    let took = t0.elapsed();

                    let mut logged = None;
                    let mut pruned = Vec::new();
                    if let (Ok(s), Some(cfg)) = (&sample, &logging)
                        && Instant::now() >= next_log
                    {
                        logged = Some(append(cfg, s));
                        next_log = Instant::now() + cfg.every;
                        // Retention is applied when the date changes, not on a
                        // timer: the rule is about days, and a poptop left
                        // running over midnight is exactly the one that needs
                        // it applied.
                        let today = log::date_of(s.at);
                        if today.is_some() && today != last_pruned {
                            last_pruned = today;
                            if let Some(d) = today {
                                pruned = log::prune(&cfg.dir, cfg.days, cfg.bytes, d);
                            }
                        }
                    }
                    let failed = sample.is_err();
                    let tick = Tick {
                        sample,
                        took,
                        logged,
                        pruned,
                    };
                    // Nobody listening: the interface has gone, so this has too.
                    if tx.send(tick).is_err() || failed {
                        return;
                    }
                    schedule.advance(Instant::now(), SystemTime::now());
                }
            })
            .expect("the sampler thread could not be started");

        Sampler {
            rx,
            needs,
            due,
            stop,
        }
    }

    /// What the next sample should gather. Read when that sample starts, so a
    /// key that opens a panel changes the next sample rather than this one.
    pub fn set_needs(&self, needs: Needs) {
        set(&self.needs, needs);
    }

    /// When the next sample starts. In the past means one is being taken now.
    pub fn due(&self) -> Instant {
        *self.due.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// A finished sample, if one is waiting. Never blocks.
    ///
    /// A collection that fails arrives as a tick holding the error, and is the
    /// last one: the thread stops after sending it.
    pub fn try_recv(&self) -> Option<Tick> {
        self.rx.try_recv().ok()
    }
}

impl Drop for Sampler {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

fn set<T>(m: &Mutex<T>, v: T) {
    *m.lock().unwrap_or_else(|p| p.into_inner()) = v;
}

/// Write one sample to the log, and say what the reader needs to hear if it
/// did not go as asked.
fn append(cfg: &Logging, s: &Sample) -> Option<String> {
    match log::append(&cfg.dir, s.at, &[s], cfg.bytes) {
        Ok(log::Appended::Wrote) => None,
        // Said while it is true, as the old "no longer being written to" was:
        // the log is still being written, and what the reader needs to know is
        // that history is now being given up at the other end.
        Ok(log::Appended::Trimmed(said)) => Some(said),
        Ok(log::Appended::Full) => {
            Some("log-bytes will not hold one entry, so the log is not being written".to_string())
        }
        Err(e) => Some(format!("could not write the log: {e}")),
    }
}

/// When samples are taken.
///
/// A fixed cadence, not "one interval after the last sample finished".
/// Restarting the clock after collection adds the collect time to every
/// period, so the timestamps drift steadily away from the rate they claim — on
/// a box with thousands of processes, far enough that the gap detector would
/// see a missed tick on every single cell and paint the whole graph as seams.
/// The interval is a schedule, so schedule against it.
///
/// **On the clock, not on the start.** Ticks fall on wall-clock multiples of
/// the interval — at one second, on the second. Started from whenever the
/// process did, two poptops on one machine never sampled the same moment, a day
/// and the one after it were offset by however long startup took, and the time
/// in the header was always some fraction past the second it showed (0230).
///
/// The wall clock sets the phase and the monotonic clock keeps time. Each tick
/// is nudged to the nearest boundary, never by more than half an interval, so
/// NTP's slewing is absorbed a millisecond at a time and a wall clock stepped
/// by an hour costs one short or long interval rather than a stall or a burst.
#[derive(Clone, Copy, Debug)]
pub struct Schedule {
    interval: Duration,
    next: Instant,
}

impl Schedule {
    /// The first boundary at least an interval away. Not the nearest: the
    /// sample before it was taken just now, and a rate over the few
    /// milliseconds to the next boundary is noise — on macOS, below the floor
    /// where sysinfo's CPU figures are simply wrong.
    pub fn new(interval: Duration, now: Instant, wall: SystemTime) -> Self {
        let early = past_boundary(wall + interval, interval);
        let wait = if early > 0 {
            interval + interval.saturating_sub(Duration::from_nanos(early as u64))
        } else {
            interval + Duration::from_nanos(early.unsigned_abs() as u64)
        };
        Schedule {
            interval,
            next: now + wait,
        }
    }

    /// When the next tick is.
    pub fn next(&self) -> Instant {
        self.next
    }

    /// Move to the next tick after the one just taken.
    ///
    /// Falling a whole interval behind means the host cannot sustain the rate.
    /// Resync rather than catch up: catching up would sample flat out until
    /// the backlog cleared, which is the worst thing to do to the loaded box
    /// that caused the backlog. The samples really are further apart than the
    /// nominal rate, and the timeline says so — that is what the seam is for.
    pub fn advance(&mut self, now: Instant, wall: SystemTime) {
        let mut next = self.next + self.interval;
        if next <= now {
            next = now + self.interval;
        }
        // Where the wall clock will read at `next`, and how far that is past
        // the boundary it should be on.
        let off = past_boundary(wall + (next - now), self.interval);
        let by = Duration::from_nanos(off.unsigned_abs() as u64);
        self.next = if off > 0 { next - by } else { next + by };
        // A correction can bring the tick back to before now only when the
        // wall clock is well ahead of where the schedule thought: take the
        // next boundary instead of sampling immediately.
        if self.next <= now {
            self.next += self.interval;
        }
    }
}

/// How far `wall` is past the nearest multiple of `interval`, in nanoseconds:
/// negative when it is short of one. Within half an interval either way.
fn past_boundary(wall: SystemTime, interval: Duration) -> i128 {
    let iv = interval.as_nanos() as i128;
    if iv == 0 {
        return 0;
    }
    let since = match wall.duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_nanos() as i128,
        Err(e) => -(e.duration().as_nanos() as i128),
    };
    let r = since.rem_euclid(iv);
    if r * 2 >= iv { r - iv } else { r }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collect::Source;
    use std::sync::mpsc::Sender;

    /// A collector that takes as long as it is told to, and reports what it
    /// was asked for.
    struct Slow {
        took: Duration,
        asked: Sender<Needs>,
    }

    impl Collector for Slow {
        fn collect(&mut self, needs: Needs) -> io::Result<Sample> {
            let _ = self.asked.send(needs);
            std::thread::sleep(self.took);
            Ok(Sample::empty())
        }
    }

    fn wait_for(s: &Sampler, within: Duration) -> Option<Tick> {
        let end = Instant::now() + within;
        while Instant::now() < end {
            if let Some(t) = s.try_recv() {
                return Some(t);
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        None
    }

    #[test]
    fn a_slow_sample_does_not_hold_up_the_caller() {
        let (asked, _) = mpsc::channel();
        let slow = Slow {
            took: Duration::from_millis(300),
            asked,
        };
        let s = Sampler::start(slow, Duration::from_millis(20), Needs::at(0), None);
        // Past the start of the first sample and well inside its 300ms.
        std::thread::sleep(Duration::from_millis(60));
        assert!(s.due() <= Instant::now(), "the sample should be under way");
        let t0 = Instant::now();
        assert!(s.try_recv().is_none());
        s.set_needs(Needs::at(1));
        assert!(
            t0.elapsed() < Duration::from_millis(50),
            "asking about a sample in flight waited for it: {:?}",
            t0.elapsed()
        );
        let tick = wait_for(&s, Duration::from_secs(2)).expect("no sample arrived");
        assert!(tick.sample.is_ok());
        assert!(tick.took >= Duration::from_millis(300));
    }

    #[test]
    fn the_next_sample_gathers_what_was_asked_for_since() {
        let (asked, heard) = mpsc::channel();
        let slow = Slow {
            took: Duration::ZERO,
            asked,
        };
        let s = Sampler::start(slow, Duration::from_millis(30), Needs::at(0), None);
        let first = heard
            .recv_timeout(Duration::from_secs(2))
            .expect("no sample");
        assert!(!first.wants(Source::Io));
        s.set_needs(Needs::at(1).with(Source::Io));
        let later = (0..5)
            .filter_map(|_| heard.recv_timeout(Duration::from_secs(2)).ok())
            .any(|n| n.wants(Source::Io));
        assert!(
            later,
            "the needs set after a sample never reached a later one"
        );
    }

    #[test]
    fn dropping_the_handle_does_not_wait_for_a_sample_in_flight() {
        let (asked, heard) = mpsc::channel();
        let slow = Slow {
            took: Duration::from_secs(2),
            asked,
        };
        let s = Sampler::start(slow, Duration::from_millis(10), Needs::at(0), None);
        heard
            .recv_timeout(Duration::from_secs(2))
            .expect("never started");
        let t0 = Instant::now();
        drop(s);
        assert!(t0.elapsed() < Duration::from_millis(50));
    }

    /// Wall-clock second `secs` plus `ms`.
    fn wall(secs: u64, ms: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(secs) + Duration::from_millis(ms)
    }

    /// Where the wall clock reads at the schedule's next tick.
    fn wall_at(s: &Schedule, now: Instant, w: SystemTime) -> SystemTime {
        w + (s.next - now)
    }

    #[test]
    fn ticks_fall_on_the_second() {
        let t = Instant::now();
        let s = Schedule::new(Duration::from_secs(1), t, wall(1000, 618));
        // At least an interval away, then onto the next whole second.
        assert_eq!(wall_at(&s, t, wall(1000, 618)), wall(1002, 0));

        let mut s = s;
        let later = s.next + Duration::from_millis(40);
        s.advance(later, wall(1002, 40));
        assert_eq!(wall_at(&s, later, wall(1002, 40)), wall(1003, 0));
    }

    #[test]
    fn an_interval_that_does_not_divide_a_minute_still_ticks_evenly() {
        let t = Instant::now();
        let iv = Duration::from_secs(7);
        let mut s = Schedule::new(iv, t, wall(1000, 0));
        let mut prev = s.next;
        for _ in 0..20 {
            let now = s.next + Duration::from_millis(5);
            let w = wall(0, 0) + (now - t) + Duration::from_secs(1000);
            s.advance(now, w);
            assert_eq!(s.next - prev, iv);
            prev = s.next;
        }
    }

    #[test]
    fn slew_is_absorbed_a_little_at_a_time() {
        // The wall clock running 2ms fast since the last tick: the next tick
        // comes 2ms early, not an interval off.
        let t = Instant::now();
        let mut s = Schedule::new(Duration::from_secs(1), t, wall(1000, 0));
        let now = s.next;
        s.advance(now, wall(1002, 2));
        assert_eq!(s.next - now, Duration::from_millis(998));
    }

    #[test]
    fn a_stepped_clock_neither_stalls_nor_bursts() {
        let iv = Duration::from_secs(1);
        for step_ms in [3_600_000i64, -3_600_000, 400, -400, 600, -600, 999, -999] {
            let t = Instant::now();
            let mut s = Schedule::new(iv, t, wall(10_000, 0));
            let now = s.next;
            let w = if step_ms >= 0 {
                wall(10_002, 0) + Duration::from_millis(step_ms as u64)
            } else {
                wall(10_002, 0) - Duration::from_millis(step_ms.unsigned_abs())
            };
            s.advance(now, w);
            let gap = s.next - now;
            assert!(
                gap >= iv / 2 && gap <= iv * 3 / 2,
                "a clock stepped {step_ms}ms moved the next tick {gap:?} away"
            );
            // And on a boundary of the clock as it now reads.
            assert_eq!(past_boundary(w + gap, iv), 0, "step {step_ms}ms");
        }
    }

    #[test]
    fn a_schedule_that_fell_behind_resyncs_rather_than_bursting() {
        let t = Instant::now();
        let iv = Duration::from_secs(1);
        let mut s = Schedule::new(iv, t, wall(1000, 0));
        // Five seconds late: the next tick is about an interval from now, not
        // five in a row.
        let now = s.next + Duration::from_secs(5);
        let w = wall(1007, 0);
        s.advance(now, w);
        let gap = s.next - now;
        assert!(gap >= iv / 2 && gap <= iv * 3 / 2, "{gap:?}");
    }
}
