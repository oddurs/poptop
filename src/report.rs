//! What happened over a period, without having watched it.
//!
//! `atopsar` reads a logfile and prints reports at an interval you choose; it
//! is how atop is used from cron. poptop could show any instant and could not
//! describe a stretch of them — "what was the worst hour yesterday" is a
//! question the buffer contains the answer to and the interface could not ask.
//!
//! Two things make this more than atopsar's version.
//!
//! **It names what was responsible.** poptop retains whole process tables, so a
//! report can say which process owned the worst minute rather than only that
//! the minute was bad. atop cannot generate that from its own logs at default
//! settings, because its process records are per-interval.
//!
//! **Peak and sustained are separate answers.** They are different questions
//! and a mean is neither: a machine that hit 100% for one second and a machine
//! that sat at 60% for an hour both average to something unremarkable, and only
//! one of them was in trouble. The timeline already refuses to aggregate by
//! mean for the same reason.

use crate::sample::{ProcSample, Sample};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

/// The stretch a report covers.
pub struct Span {
    pub from: SystemTime,
    pub to: SystemTime,
    pub samples: usize,
    /// Intervals within the period that were not observed.
    ///
    /// Reported because a summary over a period with a hole in it is a summary
    /// of the parts that were watched, and saying "the worst hour was 03:00"
    /// about a machine that was switched off from 02:00 to 04:00 would be a
    /// claim about time nobody measured.
    pub gaps: usize,
    /// The spacing the samples were recorded at, as their median gap.
    pub every: Duration,
}

impl Span {
    pub fn of(samples: &[Sample], nominal: Duration) -> Option<Span> {
        let first = samples.first()?;
        let last = samples.last()?;
        let every = crate::log::spacing(samples).unwrap_or(nominal);
        let limit = crate::history::gap_limit(every);
        let gaps = samples
            .windows(2)
            .filter(|w| w[1].at.duration_since(w[0].at).is_ok_and(|d| d >= limit))
            .count();
        Some(Span {
            from: first.at,
            to: last.at,
            samples: samples.len(),
            gaps,
            every,
        })
    }
}

/// The highest a figure reached, when, and who was on top at that instant.
pub struct Peak {
    pub value: f32,
    pub at: SystemTime,
    pub who: Option<Arc<str>>,
}

/// How long a figure stayed high, and who was there while it did.
pub struct Sustained {
    /// Time spent at or above the threshold.
    ///
    /// A duration and not a share: the report prints it against the length of
    /// the period — "2h10m of 8h00m" — which is the same fact and says what it
    /// is a share *of*, on a line that may be describing a period with holes
    /// in it.
    pub above: Duration,
    /// The worst window of the report's window length: when it started, and the
    /// mean over it.
    pub window: Option<(SystemTime, f32)>,
    /// Who accounted for the most of the time above the threshold.
    ///
    /// Summed across every sample above it rather than taken from the worst
    /// one: a process that is briefly enormous is the peak's answer, and the
    /// process that was there for all of it is this one's.
    pub who: Option<Arc<str>>,
}

/// What a report says about one figure.
pub struct Finding {
    pub name: &'static str,
    pub unit: &'static str,
    pub peak: Option<Peak>,
    pub sustained: Sustained,
    pub threshold: f32,
    /// The window the run was measured over, which is not always the one
    /// asked for — see [`run_window`].
    pub window: Duration,
}

/// The window a run is actually measured over.
///
/// The one asked for, unless the samples are too far apart to describe it. A
/// run needs at least three samples to be a run rather than a pair, and at the
/// shipped defaults — a ten-minute log against a five-minute window — there
/// are not two. Widening is better than saying nothing, and the report prints
/// the length it used so the widening is visible rather than assumed.
pub fn run_window(asked: Duration, every: Duration) -> Duration {
    asked.max(every.saturating_mul(3))
}

/// The highest value of `f`, and the process `blame` names at that moment.
pub fn peak_of(
    samples: &[Sample],
    f: impl Fn(&Sample) -> Option<f32>,
    blame: impl Fn(&Sample) -> Option<Arc<str>>,
) -> Option<Peak> {
    let mut best: Option<Peak> = None;
    for s in samples {
        let Some(v) = f(s).filter(|v| v.is_finite()) else {
            continue;
        };
        // Strictly greater, so the *first* moment a figure reached its maximum
        // is the one reported. A machine pinned at 100% for an hour should name
        // when that started, not when it happened to stop.
        if best.as_ref().is_none_or(|b| v > b.value) {
            best = Some(Peak {
                value: v,
                at: s.at,
                who: blame(s),
            });
        }
    }
    best
}

/// How long `f` stayed at or above `threshold`, and who was there for it.
pub fn sustained_of(
    samples: &[Sample],
    every: Duration,
    threshold: f32,
    window: Duration,
    f: impl Fn(&Sample) -> Option<f32>,
    blame: impl Fn(&Sample) -> Option<Arc<str>>,
) -> Sustained {
    // Counted in samples and multiplied by the spacing, not by subtracting
    // timestamps: a period with a hole in it would otherwise count the hole as
    // time spent above the threshold, which is time nobody watched.
    let mut hot = 0usize;
    let mut culprits: Vec<(Arc<str>, u32)> = Vec::new();
    for s in samples {
        let Some(v) = f(s).filter(|v| v.is_finite()) else {
            continue;
        };
        if v < threshold {
            continue;
        }
        hot += 1;
        if let Some(who) = blame(s) {
            match culprits.iter_mut().find(|(n, _)| *n == who) {
                Some((_, n)) => *n += 1,
                None => culprits.push((who, 1)),
            }
        }
    }
    // Ties to the earlier, as everywhere else in poptop: a stable answer beats
    // one that changes with the order the kernel happened to list processes in.
    let who = culprits
        .into_iter()
        .fold(None::<(Arc<str>, u32)>, |best, c| match best {
            Some(b) if b.1 >= c.1 => Some(b),
            _ => Some(c),
        })
        .map(|(n, _)| n);
    Sustained {
        above: every.saturating_mul(hot as u32),
        window: worst_window(samples, every, window, f),
        who,
    }
}

/// The window of `len` with the highest mean, and where it starts.
///
/// The answer to "was this sustained": a single spike moves a five-minute mean
/// hardly at all, and half an hour at sixty percent moves it a long way.
pub fn worst_window(
    samples: &[Sample],
    every: Duration,
    len: Duration,
    f: impl Fn(&Sample) -> Option<f32>,
) -> Option<(SystemTime, f32)> {
    if every.is_zero() {
        return None;
    }
    let n = (len.as_secs_f64() / every.as_secs_f64()).round() as usize;
    // At least two samples, or this is not a run. `round` gave one for any
    // spacing at or above the window length — and at the shipped defaults, a
    // ten-minute log against a five-minute window, that made every report's
    // "worst run" the single maximum sample, printing the peak line twice
    // under a different name. A run shorter than one sample cannot be
    // described by those samples, and saying nothing is the honest answer.
    if n < 2 {
        return None;
    }
    // A period shorter than the window has no window in it. Reporting the
    // whole period as "the worst five minutes" of a two-minute recording would
    // be a mean wearing a sustained figure's name.
    if samples.len() < n {
        return None;
    }
    let limit = crate::history::gap_limit(every);
    let mut best: Option<(SystemTime, f32)> = None;
    for w in samples.windows(n) {
        // A window with a hole in it is not a run of `len`. `sustained_of`
        // takes care never to credit a hole as time spent busy; sliding over
        // indices here would have asserted a five-minute run whose last sample
        // was two hours after its first, on the same report that says at the
        // top which stretches were not recorded.
        if w.windows(2)
            .any(|p| p[1].at.duration_since(p[0].at).is_ok_and(|d| d >= limit))
        {
            continue;
        }
        let vals: Vec<f32> = w.iter().filter_map(&f).filter(|v| v.is_finite()).collect();
        if vals.len() < n {
            // A window that is not fully measured is not a window. Averaging
            // over the samples that happen to carry the figure would report a
            // quiet hour as busy because only its busy minutes were readable.
            continue;
        }
        let mean = vals.iter().sum::<f32>() / n as f32;
        if best.is_none_or(|b| mean > b.1) {
            best = Some((w[0].at, mean));
        }
    }
    best
}

/// The process using the most of something in one sample.
fn top_by(s: &Sample, by: impl Fn(&ProcSample) -> u64) -> Option<Arc<str>> {
    s.procs
        .iter()
        .fold(None::<(&ProcSample, u64)>, |best, p| {
            let v = by(p);
            match best {
                Some(b) if b.1 >= v => Some(b),
                _ => Some((p, v)),
            }
        })
        .filter(|(_, v)| *v > 0)
        .map(|(p, _)| p.name.clone())
}

/// Everything a report says, as text.
///
/// Plain lines rather than a table: this is read in a mail from cron as often
/// as on a terminal, and it has to survive both.
pub fn render(samples: &[Sample], warn: f32, window: Duration, nominal: Duration) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    let Some(span) = Span::of(samples, nominal) else {
        return "nothing recorded\n".into();
    };

    let _ = writeln!(
        out,
        "period  {} to {}, {} samples every {}",
        crate::log::clock_string(span.from),
        crate::log::clock_string(span.to),
        span.samples,
        crate::ui::fmt_lag(span.every),
    );
    if span.gaps > 0 {
        // Said before anything else it qualifies. Every figure below is about
        // the parts that were watched, and a report that did not say so would
        // be making claims about time nobody measured.
        let _ = writeln!(
            out,
            "gaps    {} — figures below cover only what was recorded",
            span.gaps
        );
    }

    for f in findings(samples, &span, warn, window) {
        let _ = writeln!(out, "{}", one(&f, &span));
    }
    out
}

/// Every figure a report covers, in the order "why was this slow" walks
/// through — the same order the header's groups are in.
fn findings(samples: &[Sample], span: &Span, warn: f32, asked: Duration) -> Vec<Finding> {
    let window = run_window(asked, span.every);
    let mem_pct = |s: &Sample| Some(s.mem.used_pct());
    let mut out = vec![
        Finding {
            name: "cpu",
            unit: "%",
            peak: peak_of(
                samples,
                |s| Some(s.cpu_total),
                |s| top_by(s, |p| (p.cpu * 1000.0) as u64),
            ),
            sustained: sustained_of(
                samples,
                span.every,
                warn,
                window,
                |s| Some(s.cpu_total),
                |s| top_by(s, |p| (p.cpu * 1000.0) as u64),
            ),
            threshold: warn,
            window,
        },
        Finding {
            name: "memory",
            unit: "%",
            peak: peak_of(samples, mem_pct, |s| top_by(s, |p| p.rss)),
            sustained: sustained_of(samples, span.every, warn, window, mem_pct, |s| {
                top_by(s, |p| p.rss)
            }),
            threshold: warn,
            window,
        },
    ];
    // Only where the platform reports them. A report is read by somebody who
    // was not there, so a line of zeroes about a figure that was never
    // collected is worse here than anywhere else in the tool.
    if samples.iter().any(|s| s.iowait.is_some()) {
        out.push(Finding {
            name: "iowait",
            unit: "%",
            peak: peak_of(
                samples,
                |s| s.iowait,
                |s| top_by(s, |p| p.io.map_or(0, |i| i.read + i.write)),
            ),
            sustained: sustained_of(
                samples,
                span.every,
                warn,
                window,
                |s| s.iowait,
                |s| top_by(s, |p| p.io.map_or(0, |i| i.read + i.write)),
            ),
            threshold: warn,
            window,
        });
    }
    if samples.iter().any(|s| s.pressure.is_some()) {
        let stall = |s: &Sample| s.pressure.map(|p| p.worst().1);
        out.push(Finding {
            name: "stall",
            unit: "%",
            peak: peak_of(samples, stall, |s| top_by(s, |p| u64::from(p.state == 'D'))),
            sustained: sustained_of(samples, span.every, warn, window, stall, |s| {
                top_by(s, |p| u64::from(p.state == 'D'))
            }),
            threshold: warn,
            window,
        });
    }
    out
}

/// One figure, as a line.
fn one(f: &Finding, span: &Span) -> String {
    let who = |w: &Option<Arc<str>>| match w {
        Some(n) => format!(" ({n})"),
        // Nothing at all, rather than a guess or a placeholder. A figure with
        // no process that could account for it — `iowait` on a machine whose
        // per-process IO could not be read — has nothing to name, and a `(—)`
        // on the end of every such line would be punctuation pretending to be
        // information.
        None => String::new(),
    };
    let peak = match &f.peak {
        Some(p) => format!(
            "peak {:.1}{} at {}{}",
            p.value,
            f.unit,
            crate::log::clock_string(p.at),
            who(&p.who)
        ),
        None => "not reported".to_string(),
    };
    let s = &f.sustained;
    // Peak and sustained on one line and never merged. A machine that hit 100%
    // for a second and one that sat at 60% for an hour are different machines,
    // and a mean describes neither.
    let sustained = if s.above.is_zero() {
        format!("never above {:.0}{}", f.threshold, f.unit)
    } else {
        let win = match s.window {
            // The window's actual length, not the one that was asked for. It
            // is widened where the samples are too far apart to describe five
            // minutes, and a line that said "worst run" without saying a run
            // of what would be describing half an hour as five minutes.
            Some((at, mean)) => format!(
                ", worst {} run {:.1}{} from {}",
                crate::ui::fmt_lag(f.window),
                mean,
                f.unit,
                crate::log::clock_string(at)
            ),
            None => String::new(),
        };
        format!(
            "above {:.0}{} for {} of {}{}{}",
            f.threshold,
            f.unit,
            crate::ui::fmt_lag(s.above),
            crate::ui::fmt_lag(span.every.saturating_mul(span.samples as u32)),
            win,
            who(&s.who)
        )
    };
    format!("{:<8}{peak}\n        {sustained}", f.name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sample::MemStat;
    use std::time::UNIX_EPOCH;

    fn at(secs: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(secs)
    }

    /// The instant of `s(n, ..)`, so a test can name one without repeating the
    /// base offset and getting it wrong.
    fn at_secs(n: u64) -> SystemTime {
        at(1_800_000_000 + n)
    }

    /// A sample at second `n` with a given CPU total, and one process holding
    /// all of it.
    fn s(n: u64, cpu: f32, who: &str) -> Sample {
        let mut s = Sample::unknown();
        s.at = at(1_800_000_000 + n);
        s.cpu_total = cpu;
        s.mem = MemStat {
            total: 100,
            used: 10,
            available: 90,
            ..MemStat::default()
        };
        s.procs = vec![ProcSample {
            pid: 1,
            name: Arc::from(who),
            user: Arc::from("root"),
            cpu,
            rss: 1,
            ..ProcSample::default()
        }];
        s
    }

    /// A day with a real shape in it, for reading rather than asserting.
    ///
    /// `cargo test -- --ignored --nocapture show_report`.
    #[test]
    #[ignore]
    fn show_report() {
        // Eight hours at ten-minute samples: a quiet morning, a long build
        // through the afternoon, and one spike in the evening.
        let day: Vec<Sample> = (0..48)
            .map(|i| {
                let (cpu, who) = match i {
                    20..32 => (74.0, "cc1plus"),
                    40 => (99.0, "backup"),
                    _ => (6.0, "launchd"),
                };
                let mut s = s(i * 600, cpu, who);
                s.iowait = Some(if i == 40 { 61.0 } else { 1.0 });
                s.mem.used = if i >= 20 { 78 } else { 30 };
                s
            })
            .collect();
        print!(
            "{}",
            render(
                &day,
                50.0,
                Duration::from_secs(1800),
                Duration::from_secs(600)
            )
        );
    }

    #[test]
    fn a_peak_names_the_moment_and_the_process() {
        // The thing atop cannot do from its own logs: the report says *which*
        // process owned the worst instant, because the sample kept the table.
        let day = [s(0, 5.0, "idle"), s(1, 99.0, "compiler"), s(2, 7.0, "idle")];
        let p = peak_of(
            &day,
            |s| Some(s.cpu_total),
            |s| top_by(s, |p| (p.cpu * 1000.0) as u64),
        )
        .expect("a peak");
        assert_eq!(p.value, 99.0);
        assert_eq!(p.at, at(1_800_000_001));
        assert_eq!(p.who.as_deref(), Some("compiler"));
    }

    #[test]
    fn the_first_moment_of_a_plateau_is_the_one_reported() {
        // A machine pinned at 100% for an hour should say when that started,
        // not when it happened to stop.
        let day: Vec<Sample> = (0..10).map(|i| s(i, 100.0, "compiler")).collect();
        let p = peak_of(&day, |s| Some(s.cpu_total), |_| None).unwrap();
        assert_eq!(p.at, at(1_800_000_000));
    }

    #[test]
    fn peak_and_sustained_are_different_answers() {
        // The whole reason this is not a mean. Both of these average to
        // something unremarkable and only one of them was in trouble.
        let every = Duration::from_secs(1);
        let window = Duration::from_secs(4);
        let spike: Vec<Sample> = (0..20)
            .map(|i| s(i, if i == 10 { 100.0 } else { 5.0 }, "x"))
            .collect();
        let grind: Vec<Sample> = (0..20).map(|i| s(i, 62.0, "y")).collect();

        let a = sustained_of(&spike, every, 50.0, window, |s| Some(s.cpu_total), |_| None);
        let b = sustained_of(&grind, every, 50.0, window, |s| Some(s.cpu_total), |_| None);

        // The peak says the same thing about both, near enough.
        assert!(
            peak_of(&spike, |s| Some(s.cpu_total), |_| None)
                .unwrap()
                .value
                > 50.0
        );
        assert!(
            peak_of(&grind, |s| Some(s.cpu_total), |_| None)
                .unwrap()
                .value
                > 50.0
        );
        // The sustained figure does not.
        assert_eq!(a.above, Duration::from_secs(1));
        assert_eq!(b.above, Duration::from_secs(20));
        // One second of twenty against all twenty of them.
        assert!(a.above * 19 < b.above, "{:?} {:?}", a.above, b.above);
        // And the window mean separates them, which a period mean would not:
        // the spike's four-second window is dragged down by its three quiet
        // seconds where the grind's is not.
        assert!(a.window.unwrap().1 < b.window.unwrap().1);
    }

    #[test]
    fn the_process_that_was_there_for_all_of_it_is_the_sustained_answer() {
        // Not the one that was briefly enormous — that is the peak's answer,
        // and conflating them would name a one-second process as the cause of
        // an hour.
        // `early` first and `brief` last, so an implementation that takes
        // whichever it saw first or last names one of them instead of the one
        // that was there throughout.
        let mut day: Vec<Sample> = (0..20).map(|i| s(i, 80.0, "steady")).collect();
        day[0] = s(0, 80.0, "early");
        day[19] = s(19, 99.0, "brief");
        let out = sustained_of(
            &day,
            Duration::from_secs(1),
            50.0,
            Duration::from_secs(5),
            |s| Some(s.cpu_total),
            |s| top_by(s, |p| (p.cpu * 1000.0) as u64),
        );
        assert_eq!(out.who.as_deref(), Some("steady"));
        assert_eq!(
            peak_of(
                &day,
                |s| Some(s.cpu_total),
                |s| top_by(s, |p| (p.cpu * 1000.0) as u64)
            )
            .unwrap()
            .who
            .as_deref(),
            Some("brief")
        );
    }

    #[test]
    fn a_gap_is_not_time_spent_above_the_threshold() {
        // Counted in samples and multiplied by the spacing, not by subtracting
        // timestamps: a machine that was switched off for two hours between a
        // busy sample and the next one did not spend those hours busy.
        // Twenty seconds of samples, then a two-hour hole, then one more.
        let mut day: Vec<Sample> = (0..20).map(|i| s(i, 90.0, "x")).collect();
        day.push(s(7_200, 90.0, "x"));
        let out = sustained_of(
            &day,
            Duration::from_secs(1),
            50.0,
            Duration::from_secs(2),
            |s| Some(s.cpu_total),
            |_| None,
        );
        assert_eq!(out.above, Duration::from_secs(21), "the gap was counted");

        // And the gap is reported, because every figure is about the parts
        // that were watched.
        let span = Span::of(&day, Duration::from_secs(1)).unwrap();
        assert_eq!(span.gaps, 1);
        assert!(
            render(&day, 50.0, Duration::from_secs(2), Duration::from_secs(1)).contains("gaps")
        );
    }

    #[test]
    fn a_run_needs_more_than_one_sample_to_be_a_run() {
        // `round` gave one sample for any spacing at or above the window, so
        // at the shipped defaults — a ten-minute log against a five-minute
        // window — every report's "worst run" was the single maximum sample,
        // printing the peak line twice under a different name.
        let day: Vec<Sample> = (0..20).map(|i| s(i * 600, 50.0 + i as f32, "x")).collect();
        assert!(
            worst_window(
                &day,
                Duration::from_secs(600),
                Duration::from_secs(300),
                |s| Some(s.cpu_total)
            )
            .is_none(),
            "a ten-minute log described a five-minute run"
        );

        // And the window a report actually uses is widened rather than lost,
        // with its real length printed — a line saying "worst run" without
        // saying a run of *what* would call half an hour five minutes.
        assert_eq!(
            run_window(Duration::from_secs(300), Duration::from_secs(600)),
            Duration::from_secs(1800)
        );
        assert_eq!(
            run_window(Duration::from_secs(300), Duration::from_secs(1)),
            Duration::from_secs(300)
        );
        let out = render(
            &day,
            50.0,
            Duration::from_secs(300),
            Duration::from_secs(600),
        );
        assert!(out.contains("worst 30m00s run"), "{out}");
    }

    #[test]
    fn a_run_does_not_span_a_hole() {
        // `sustained_of` takes care never to credit a hole as time spent busy.
        // Sliding over indices here asserted a three-second run whose last
        // sample was two hours after its first — on the same report that says
        // at the top which stretches were not recorded.
        let mut day: Vec<Sample> = (0..3).map(|i| s(i, 95.0, "x")).collect();
        day.extend((0..3).map(|i| s(7_200 + i, 95.0, "x")));
        // Quieter samples on each side of the hole, so a window that refuses
        // to cross it still has somewhere honest to land.
        day.insert(3, s(3, 10.0, "x"));

        let every = Duration::from_secs(1);
        let run = worst_window(&day, every, Duration::from_secs(3), |s| Some(s.cpu_total));
        let (at, mean) = run.expect("there are three adjacent busy samples");
        assert_eq!(mean, 95.0);
        assert_eq!(
            at,
            at_secs(0),
            "the run started across the hole rather than before it"
        );

        // With nothing three-adjacent anywhere, there is no run at all.
        let sparse = [s(0, 95.0, "x"), s(7_200, 95.0, "x"), s(14_400, 95.0, "x")];
        assert!(
            worst_window(&sparse, every, Duration::from_secs(3), |s| Some(
                s.cpu_total
            ))
            .is_none(),
            "a run was found across two holes"
        );
    }

    #[test]
    fn a_period_shorter_than_the_window_has_no_window() {
        // Reporting a two-minute recording's whole length as "the worst five
        // minutes" would be a period mean wearing a sustained figure's name.
        let day: Vec<Sample> = (0..3).map(|i| s(i, 90.0, "x")).collect();
        assert!(
            worst_window(
                &day,
                Duration::from_secs(1),
                Duration::from_secs(300),
                |s| Some(s.cpu_total)
            )
            .is_none()
        );
        assert!(
            worst_window(&day, Duration::from_secs(1), Duration::from_secs(2), |s| {
                Some(s.cpu_total)
            })
            .is_some()
        );
    }

    #[test]
    fn a_figure_the_platform_never_reported_is_not_a_line_of_zeroes() {
        // A report is read by somebody who was not there, so an invented zero
        // costs more here than anywhere else in the tool.
        let day: Vec<Sample> = (0..5).map(|i| s(i, 10.0, "x")).collect();
        let out = render(&day, 50.0, Duration::from_secs(2), Duration::from_secs(1));
        assert!(!out.contains("iowait"), "{out}");
        assert!(!out.contains("stall"), "{out}");
        assert!(out.contains("cpu"), "{out}");

        let mut with = day.clone();
        for s in &mut with {
            s.iowait = Some(4.0);
        }
        assert!(
            render(&with, 50.0, Duration::from_secs(2), Duration::from_secs(1)).contains("iowait")
        );
    }

    #[test]
    fn a_quiet_period_says_it_was_quiet() {
        let day: Vec<Sample> = (0..10).map(|i| s(i, 3.0, "x")).collect();
        let out = render(&day, 50.0, Duration::from_secs(2), Duration::from_secs(1));
        assert!(out.contains("never above 50%"), "{out}");
        // …and still names its peak, which is the honest one-line summary of a
        // machine that did nothing.
        assert!(out.contains("peak 3.0%"), "{out}");
    }
}
