//! What poptop may cost, and tests that fail when it costs more.
//!
//! 0038 was a milestone about being cheap enough to leave running, and until
//! these existed nothing failed if sampling, drawing or the store got twice as
//! slow. Each budget below is set from the figures in its comment, measured on
//! an M-series Mac and in a Linux arm64 container on the same machine, release
//! builds, on 2026-09-19. A time gets about three times the slower of the two,
//! since a budget exists to catch a regression and not to measure noise; a
//! size, which does not vary from run to run, gets a fifth more.
//!
//! Release builds only, and one at a time:
//!
//! ```text
//! ./check --perf
//! cargo test --release --bin poptop budget:: -- --ignored --test-threads=1 --nocapture
//! ```
//!
//! `POPTOP_BUDGET=report` prints every figure and fails none. CI runs them that
//! way: a shared runner's timing is noise, and a budget that fails on noise is
//! one people learn to rerun.

use crate::app::App;
use crate::collect::{Collector, Platform};
use crate::sample::Sample;
use crate::store::tests_support::big_sample;
use std::time::{Duration, Instant, UNIX_EPOCH};

/// Hold a measurement to its budget, or only say it, in report mode.
fn within<T: PartialOrd + std::fmt::Debug>(what: &str, measured: T, budget: T) {
    let ok = measured <= budget;
    println!(
        "  {:<44} {:>14} of {:>14}  {}",
        what,
        format!("{measured:?}"),
        format!("{budget:?}"),
        if ok { "ok" } else { "OVER" }
    );
    if std::env::var("POPTOP_BUDGET").as_deref() == Ok("report") {
        return;
    }
    if cfg!(debug_assertions) {
        panic!("budgets are for release builds: cargo test --release");
    }
    assert!(ok, "{what}: {measured:?} is over its budget of {budget:?}");
}

/// The best of `runs` timings of `f`, each over `per` calls. The best, not the
/// mean: what is being bounded is what the code costs, and a slower run is the
/// machine doing something else.
fn best<R>(runs: u32, per: u32, mut f: impl FnMut() -> R) -> Duration {
    (0..runs)
        .map(|_| {
            let t0 = Instant::now();
            for _ in 0..per {
                std::hint::black_box(f());
            }
            t0.elapsed() / per
        })
        .min()
        .unwrap_or_default()
}

/// A full buffer: the default window at one sample a second, each sample a
/// busy machine's process table.
fn full(procs: usize) -> Vec<Sample> {
    (0..600)
        .map(|i| {
            let mut s = big_sample((i % 100) as f32, procs);
            s.at = UNIX_EPOCH + Duration::from_secs(1_700_000_000 + i);
            s
        })
        .collect()
}

#[test]
#[ignore = "a budget: ./check --perf"]
fn collecting_a_sample() {
    // Per process, because the process table is what a sample's cost scales
    // with and every machine has a different one: 8.8µs a process on the Mac
    // (6.3ms for about 720). Only held where there are a hundred processes to
    // divide by: the container's five cost 0.17ms, all of it the fixed reads,
    // which is 34µs "a process" and means nothing. The whole sample has a
    // ceiling of its own, for a regression in the fixed part.
    let mut c = Platform::new().unwrap();
    let app = App::new(600);
    let needs = app.needs();
    c.sample(needs).unwrap();
    let mut procs = 0;
    let each = best(3, 10, || {
        let s = c.sample(needs).unwrap();
        procs = s.procs.len().max(1);
    });
    within(
        "collect: one sample, whole",
        each,
        Duration::from_millis(25),
    );
    if procs >= 100 {
        within(
            "collect: per process in the table",
            each / procs as u32,
            Duration::from_micros(30),
        );
    } else {
        println!("  collect: per process: {procs} processes is too few to divide by");
    }
}

#[test]
#[ignore = "a budget: ./check --perf"]
fn drawing_a_frame() {
    // 900 processes by 600 samples of history at 200x60, the case
    // `measure_render_with_sparklines` was written for. 4.4ms on the Mac,
    // 7.7ms in the container. A frame is drawn once a second and on every key.
    use ratatui::{Terminal, backend::TestBackend};
    let mut app = App::new(600);
    for s in full(900) {
        app.push(s);
    }
    let mut term = Terminal::new(TestBackend::new(200, 60)).unwrap();
    let each = best(3, 10, || {
        term.draw(|f| crate::ui::draw(f, &app)).map(|_| ())
    });
    within(
        "draw: a 200x60 frame, 900 procs x 600",
        each,
        Duration::from_millis(25),
    );
}

#[test]
#[ignore = "a budget: ./check --perf"]
fn writing_and_reading_a_full_store() {
    // 600 samples of 400 processes, the default window full. Encode 29ms on
    // the Mac and 30ms in the container, decode 7.7ms and 11.6ms; 25.0 MB,
    // the same on both. Written once, on the way out; read once, on the way
    // in.
    let all = full(400);
    let refs: Vec<&Sample> = all.iter().collect();
    let bytes = crate::store::encode(&refs);
    let write = best(3, 1, || crate::store::encode(&refs));
    let read = best(3, 1, || crate::store::decode(&bytes));
    within(
        "store: encode a full buffer",
        write,
        Duration::from_millis(90),
    );
    within(
        "store: decode a full buffer",
        read,
        Duration::from_millis(35),
    );
    within("store: bytes, full buffer", bytes.len(), 30_000_000);
}

#[test]
#[ignore = "a budget: ./check --perf"]
fn starting_with_a_full_store() {
    // From the file's bytes to the first frame: decode, fill the buffer, draw.
    // What `show_startup_cost_with_a_full_store` measured the first part of.
    // 10.2ms on the Mac, 10.5ms in the container.
    use ratatui::{Terminal, backend::TestBackend};
    let all = full(400);
    let refs: Vec<&Sample> = all.iter().collect();
    let bytes = crate::store::encode(&refs);
    let each = best(3, 1, || {
        let mut app = App::new(600);
        for s in crate::store::decode(&bytes).unwrap() {
            app.push(s);
        }
        let mut term = Terminal::new(TestBackend::new(200, 60)).unwrap();
        term.draw(|f| crate::ui::draw(f, &app)).map(|_| ())
    });
    within(
        "start: full store to first frame",
        each,
        Duration::from_millis(35),
    );
}

/// This process's resident memory.
#[cfg(target_os = "linux")]
pub fn rss() -> u64 {
    let status = std::fs::read_to_string("/proc/self/status").unwrap();
    let kb = status
        .lines()
        .find_map(|l| l.strip_prefix("VmRSS:"))
        .and_then(|v| v.trim().trim_end_matches("kB").trim().parse::<u64>().ok())
        .unwrap();
    kb * 1024
}

/// This process's resident memory, from `ps`.
#[cfg(not(target_os = "linux"))]
pub fn rss() -> u64 {
    let out = std::process::Command::new("ps")
        .args(["-o", "rss=", "-p", &std::process::id().to_string()])
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout)
        .trim()
        .parse::<u64>()
        .unwrap()
        * 1024
}

#[test]
#[ignore = "a budget: ./check --perf"]
fn memory_after_an_hour() {
    // An hour at one sample a second, 400 processes each, into the default
    // buffer. The buffer holds ten minutes, so everything past that must be
    // given back: what is being bounded is the ring and what it interns, not
    // an hour of samples. 66 MB on the Mac and 62 MB in the container, and
    // after the buffer filled at ten minutes, 96 KB and nothing: flat.
    //
    // In a process of its own: every other test here builds and frees a
    // full buffer, and memory an allocator has been given back is reused
    // rather than counted, so measured beside them an hour cost nothing.
    if std::env::var_os("POPTOP_BUDGET_ALONE").is_none() {
        let out = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "budget::memory_after_an_hour",
                "--ignored",
                "--nocapture",
            ])
            .env("POPTOP_BUDGET_ALONE", "1")
            .output()
            .unwrap();
        print!("{}", String::from_utf8_lossy(&out.stdout));
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        return;
    }
    let before = rss();
    let mut app = App::new(600);
    let mut at_ten_minutes = 0;
    for i in 0..3600u64 {
        let mut s = big_sample((i % 100) as f32, 400);
        s.at = UNIX_EPOCH + Duration::from_secs(1_700_000_000 + i);
        app.push(s);
        if i == 600 {
            at_ten_minutes = rss();
        }
    }
    let after = rss();
    std::hint::black_box(&app);
    within(
        "memory: growth over an hour",
        after.saturating_sub(before),
        100 << 20,
    );
    // Flat after warm-up: the fifty minutes after the buffer filled may not
    // cost what the first ten did.
    within(
        "memory: growth after the buffer is full",
        after.saturating_sub(at_ten_minutes),
        8 << 20,
    );
}

#[test]
#[ignore = "a budget: ./check --perf"]
fn appending_one_entry_to_the_log() {
    // An entry, written and synced. The sync is the whole cost — 4.2ms on
    // APFS and 2.8ms on ext4 against 0.09ms for the write — and it is what
    // makes the log survive the machine rather than only the process. Paid
    // once a `log-interval`, which is ten minutes by default.
    let dir = std::env::temp_dir().join(format!("poptop-budget-log-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let all = full(400);
    let one: Vec<&Sample> = all.first().into_iter().collect();
    let at = one[0].at;
    let each = best(3, 5, || crate::log::append(&dir, at, &one, u64::MAX));
    let _ = std::fs::remove_dir_all(&dir);
    within(
        "log: append one entry, synced",
        each,
        Duration::from_millis(40),
    );
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "a budget: ./check --perf"]
fn parsing_a_hundred_nfs_mounts() {
    // What `cost_of_nfs` measured: a machine with a hundred NFSv4.2 mounts.
    // Built with seventy-one per-op lines a mount, as 4.2 writes them; a
    // fixture with three would make the parse look twenty times cheaper than
    // it is. 0.5ms in the container. Read every sample on a machine that
    // mounts NFS, so it is a real share of one.
    let mut text = String::new();
    for m in 0..100 {
        text.push_str(&format!(
            "device 10.0.0.{m}:/export mounted on /mnt/{m} with fstype nfs4 statvers=1.1\n\
             \topts:\trw,vers=4.2,rsize=1048576,wsize=1048576,proto=tcp\n\
             \tage:\t12345\n\
             \tbytes:\t900 800 0 0 4096 8192 1 2\n\
             \tRPC iostats version: 1.1  p/v: 100003/4 (nfs)\n\
             \txprt:\ttcp 0 0 1 0 5 1000 1000 0 1000 0 2 3 4\n\
             \tper-op statistics\n"
        ));
        for op in 0..71 {
            text.push_str(&format!(
                "\t    OP{op:02}: 100 104 0 1000 2000 0 296 400 0\n"
            ));
        }
    }
    let each = best(3, 20, || crate::collect::nfs::parse_mountstats(&text));
    within(
        "nfs: parse 100 NFSv4.2 mounts",
        each,
        Duration::from_micros(1500),
    );
}
