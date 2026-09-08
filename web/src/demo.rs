//! The buffer behind the landing page's scrubbable frame.
//!
//! It is synthetic, and the page says so. What it is not is decorative: it is
//! a real incident with a real cause — a database process that walks up to
//! saturation over half a minute, drags the run queue with it, and comes back
//! down — because the claim the site is making is that you can scrub back to
//! the moment and name the process. A hero that shows noise proves nothing.
//!
//! Generated from a fixed seed on the server rather than sampled in the
//! browser, so the same incident is on the page for every visitor and a static
//! export is byte-identical between builds.

use std::fmt::Write;

const SAMPLES: usize = 320;
const INTERVAL: u32 = 1;
const CORES: u32 = 4;
/// Where the incident peaks. The page opens near here, because opening on
/// idle would be an honest demo of nothing.
const PEAK: usize = 232;

struct Proc {
    pid: u32,
    cmd: &'static str,
    state: &'static str,
    threads: u32,
    cpu: Vec<f64>,
    rss: Vec<u64>,
}

/// Deterministic noise. Not random, and not trying to be: a fixed sequence is
/// the point.
struct Wobble(u64);

impl Wobble {
    fn next(&mut self) -> f64 {
        // xorshift64*, which is small enough to read and good enough to look
        // like a machine under load.
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0.wrapping_mul(0x2545_f491_4f6c_dd1d) >> 11) as f64 / (1u64 << 53) as f64
    }

    fn spread(&mut self, width: f64) -> f64 {
        (self.next() - 0.5) * 2.0 * width
    }
}

/// A hump centred on `PEAK`: flat, then a climb, saturation, and a recovery
/// that is slower than the climb, the way real contention unwinds.
fn incident(i: usize) -> f64 {
    let t = i as f64 - PEAK as f64;
    if t < -70.0 || t > 55.0 {
        return 0.0;
    }
    if t < 0.0 {
        // Climb.
        let x = (t + 70.0) / 70.0;
        x.powf(2.6)
    } else {
        // Recovery, with a shoulder.
        let x = 1.0 - t / 55.0;
        (x.powf(0.7)).max(0.0)
    }
}

pub fn buffer_json() -> String {
    let mut w = Wobble(0x706f_7074_6f70_0001);

    let mut cpu = Vec::with_capacity(SAMPLES);
    let mut mem = Vec::with_capacity(SAMPLES);
    let mut wait = Vec::with_capacity(SAMPLES);
    let mut run = Vec::with_capacity(SAMPLES);
    let mut blocked = Vec::with_capacity(SAMPLES);
    // A separate series for the peak-versus-mean section, and the only one on
    // the page that is not part of the incident. The claim there is about
    // *brief* saturation — a second at 95% among idle seconds — which the
    // incident's broad hump cannot demonstrate, because a four-sample mean of a
    // slow climb looks much like its peak. This is the shape that actually
    // disappears when you average it, and machines really do this: a cron job,
    // a GC pause, a fsync.
    let mut spike = Vec::with_capacity(SAMPLES);

    let mut procs = [
        Proc {
            pid: 824,
            cmd: "postgres",
            state: "R",
            threads: 8,
            cpu: vec![],
            rss: vec![],
        },
        Proc {
            pid: 1190,
            cmd: "nginx",
            state: "S",
            threads: 4,
            cpu: vec![],
            rss: vec![],
        },
        Proc {
            pid: 2077,
            cmd: "node",
            state: "S",
            threads: 11,
            cpu: vec![],
            rss: vec![],
        },
        Proc {
            pid: 3310,
            cmd: "rustc",
            state: "S",
            threads: 6,
            cpu: vec![],
            rss: vec![],
        },
        Proc {
            pid: 1,
            cmd: "systemd",
            state: "S",
            threads: 1,
            cpu: vec![],
            rss: vec![],
        },
    ];

    for i in 0..SAMPLES {
        let surge = incident(i);

        // postgres is the cause: idle-ish, then saturating.
        let pg = (3.0 + surge * 71.0 + w.spread(2.5)).clamp(0.2, 92.0);
        // nginx follows it, because the requests are queueing behind the query.
        let ng = (4.0 + surge * 11.0 + w.spread(1.6)).clamp(0.2, 30.0);
        // node is doing its own unrelated work.
        let nd = (7.0 + w.spread(3.5) + (i as f64 / 21.0).sin() * 3.0).clamp(0.4, 24.0);
        // rustc is a second, unrelated burst — so the demo has a spike that is
        // *not* the incident, and scrubbing has to tell them apart.
        let rc = if (58..96).contains(&i) {
            (46.0 + w.spread(11.0)).clamp(5.0, 74.0)
        } else {
            w.next() * 0.8
        };
        let sd = 0.1 + w.next() * 0.4;

        let loads = [pg, ng, nd, rc, sd];
        for (p, value) in procs.iter_mut().zip(loads) {
            p.cpu.push(round1(value));
        }

        // Memory: a baseline, plus postgres's working set growing under load
        // and not shrinking back, which is what actually happens.
        let held = 31.0 + surge * 21.0 + (i as f64 / SAMPLES as f64) * 8.0;
        mem.push(round1((held + w.spread(0.8)).clamp(5.0, 96.0)));

        let rss_base = [512u64 * 1024, 32 * 1024, 148 * 1024, 96 * 1024, 12 * 1024];
        for (p, base) in procs.iter_mut().zip(rss_base) {
            let grow = if p.cmd == "postgres" {
                1.0 + surge * 0.55
            } else {
                1.0
            };
            p.rss.push((base as f64 * grow) as u64);
        }

        // The header's figure is the machine, and each row is that process's
        // share of the same machine — the way the README's own example reads,
        // where postgres at 88.4 sits under a total of 89.2.
        let total = (loads.iter().sum::<f64>() + w.spread(1.2)).clamp(0.5, 99.5);
        cpu.push(round1(total));
        // Wait rises with saturation, and lags it slightly.
        let lag = incident(i.saturating_sub(6));
        wait.push(round1((1.5 + lag * 31.0 + w.spread(1.2)).clamp(0.0, 60.0)));
        run.push((1.0 + surge * 3.4 + w.next()) as u32);
        spike.push(round1(if i % 37 == 11 {
            (92.0 + w.spread(6.0)).clamp(80.0, 99.0)
        } else {
            (6.0 + w.spread(4.0)).clamp(0.5, 14.0)
        }));
        blocked.push(if surge > 0.72 {
            (surge * 4.0) as u32
        } else {
            0
        });
    }

    let mut out = String::with_capacity(48 * 1024);
    out.push('{');
    write!(
        out,
        "\"interval\":{INTERVAL},\"cores\":{CORES},\"focus\":{PEAK},"
    )
    .unwrap();
    push_floats(&mut out, "cpu", &cpu);
    push_floats(&mut out, "mem", &mem);
    push_floats(&mut out, "wait", &wait);
    push_floats(&mut out, "spike", &spike);
    push_ints(&mut out, "run", &run);
    push_ints(&mut out, "blocked", &blocked);
    out.push_str("\"procs\":[");
    for (i, p) in procs.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        write!(
            out,
            "{{\"pid\":{},\"cmd\":\"{}\",\"state\":\"{}\",\"threads\":{},",
            p.pid, p.cmd, p.state, p.threads
        )
        .unwrap();
        push_floats(&mut out, "cpu", &p.cpu);
        out.push_str("\"rss\":[");
        for (j, v) in p.rss.iter().enumerate() {
            if j > 0 {
                out.push(',');
            }
            write!(out, "{v}").unwrap();
        }
        out.push_str("]}");
    }
    out.push_str("]}");
    out
}

/// The frame the page shows before its script runs, and the whole of it for a
/// reader without one: the peak of the incident, as plain text.
pub fn peak_summary() -> String {
    format!(
        "At the peak, {PEAK}s into the buffer: postgres at 74% of the machine, \
         the run queue at 4 of {CORES}, and memory climbing and not coming back."
    )
}

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

fn push_floats(out: &mut String, key: &str, values: &[f64]) {
    write!(out, "\"{key}\":[").unwrap();
    for (i, v) in values.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        write!(out, "{v}").unwrap();
    }
    out.push_str("],");
}

fn push_ints(out: &mut String, key: &str, values: &[u32]) {
    write!(out, "\"{key}\":[").unwrap();
    for (i, v) in values.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        write!(out, "{v}").unwrap();
    }
    out.push_str("],");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The buffer is embedded in a `<script>` element. A `</script>` inside it
    /// would end the element early, and a lone `<` is the only way that can
    /// happen — so there must not be one.
    #[test]
    fn the_buffer_is_safe_to_embed_in_a_script_element() {
        let json = buffer_json();
        assert!(!json.contains('<'));
        assert!(!json.contains('&'));
    }

    /// The section that draws mean against peak needs samples where the two
    /// genuinely differ. If the spikes ever became broad enough that averaging
    /// preserved them, that section would be showing two identical plots under
    /// captions claiming they differ.
    #[test]
    fn the_spike_series_is_brief_enough_to_average_away() {
        let json = buffer_json();
        let at = json.find("\"spike\":[").expect("spike series");
        let rest = &json[at + 9..];
        let end = rest.find(']').unwrap();
        let values: Vec<f64> = rest[..end].split(',').map(|v| v.parse().unwrap()).collect();

        let high = values.iter().filter(|v| **v > 50.0).count();
        assert!(high > 4, "only {high} spikes; the section needs several");
        assert!(
            high * 8 < values.len(),
            "{high} of {} samples are spikes — too many to average away",
            values.len()
        );

        // The thing the picture claims: four of these averaged is a quarter of
        // what the peak of them is.
        let window = &values[8..16];
        let mean = window.iter().sum::<f64>() / window.len() as f64;
        let peak = window.iter().cloned().fold(0.0, f64::max);
        assert!(
            peak > mean * 3.0,
            "peak {peak:.1} vs mean {mean:.1} is not a story"
        );
    }

    #[test]
    fn the_same_seed_gives_the_same_incident() {
        assert_eq!(buffer_json(), buffer_json());
    }

    #[test]
    fn the_incident_peaks_where_the_page_opens() {
        let json = buffer_json();
        // Cheap parse: the demo's own contract is that postgres is the busiest
        // process at the peak, which is what the landing page claims.
        assert!(json.contains("\"cmd\":\"postgres\""));
        assert!(incident(PEAK) > 0.9);
        assert_eq!(incident(0), 0.0);
    }
}
