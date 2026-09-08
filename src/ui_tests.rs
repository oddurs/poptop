//! Headless render tests.
//!
//! A TUI that panics on a 20-column terminal or an empty buffer is worse than
//! no TUI, and neither case shows up in normal use. `TestBackend` renders into
//! a plain buffer so both are cheap to exercise.

use crate::app::App;
use crate::sample::{MemStat, ProcSample, Sample, ThreadSample};
use crate::theme::{Palette, Theme, Tier};
use crate::ui;
use ratatui::Terminal;
use ratatui::backend::TestBackend;

/// Walk the selection down the table until `want` matches, or fail.
///
/// Bounded on purpose. The first version of these tests used
/// `while ... { app.select_delta(1) }`, which turns a wrong answer into an
/// infinite loop — under one mutation the whole suite hung instead of
/// reporting a failure, which is strictly worse than a red test.
fn select_until(app: &mut App, what: &str, want: impl Fn(&str) -> bool) {
    for _ in 0..500 {
        if app.selected.as_ref().is_some_and(|w| want(w.name())) {
            return;
        }
        app.select_delta(1);
    }
    panic!(
        "walked the whole table without finding {what}; last selection was {:?}",
        app.selected.as_ref().map(|w| w.name().to_string())
    );
}

/// A process for a fixture.
///
/// Keep pids and ppids away from 2: on Linux that is `kthreadd`, so a fixture
/// using it builds a kernel thread by accident and the table hides it by
/// default. Three tests here did exactly that with incidental low pids, and
/// passed on macOS while failing in the container.
fn proc_named(pid: i32, name: &str, cpu: f32, rss: u64) -> ProcSample {
    ProcSample {
        pid,
        ppid: 1,
        name: std::sync::Arc::from(name),
        user: std::sync::Arc::from("root"),
        cpu,
        rss,
        threads: Some(1),
        state: 'S',
        started: Some(0),
        cmd: None,
        io: None,
    }
}

fn sample(cpu: f32) -> Sample {
    sample_at(cpu, 0)
}

/// `age_secs` back-dates the sample, so tests can exercise anything that reads
/// the clock rather than counting rows.
fn sample_at(cpu: f32, age_secs: u64) -> Sample {
    Sample {
        at: std::time::SystemTime::now() - std::time::Duration::from_secs(age_secs),
        cpu_total: cpu,
        cpu_per_core: vec![cpu, cpu / 2.0, 0.0, 99.0],
        disks: None,
        clock_ceiling: None,
        tasks: None,
        pressure: None,
        net: None,
        filesystems: None,
        iowait: None,
        running: None,
        blocked: None,
        mem: MemStat {
            total: 16 << 30,
            used: 8 << 30,
            available: 8 << 30,
            free: Some(5 << 30),
            swap_total: 2 << 30,
            swap_used: 1 << 30,
        },
        load: [1.0, 2.0, 3.0],
        procs: vec![
            proc_named(1, "init", 0.1, 1 << 20),
            proc_named(42, "postgres", 88.0, 512 << 20),
            proc_named(99, "nginx", 12.5, 32 << 20),
        ],
        uptime: std::time::Duration::from_secs(90_000),
        forks: None,
        io_supported: true,
        io_collected: false,
        io_denied: 0,
    }
}

fn render(app: &App, w: u16, h: u16) -> String {
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    term.draw(|f| ui::draw(f, app)).unwrap();
    term.backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect::<String>()
}

/// The frame split into terminal rows.
///
/// `render` concatenates every cell with no line breaks, so `.lines()` on its
/// output yields one enormous line — which quietly turns "is this in the
/// timeline block" into "is this anywhere on screen". Anything asking where
/// something is has to chunk by width first.
fn rows(app: &App, w: u16, h: u16) -> Vec<String> {
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    term.draw(|f| ui::draw(f, app)).unwrap();
    let cells: Vec<String> = term
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol().to_string())
        .collect();
    cells.chunks(w as usize).map(|row| row.concat()).collect()
}

#[test]
fn renders_without_panicking() {
    let mut app = App::new(60);
    app.push(sample(42.0));
    let out = render(&app, 100, 30);
    assert!(out.contains("postgres"));
    assert!(out.contains("LIVE"));
}

#[test]
fn empty_history_shows_placeholder_instead_of_panicking() {
    let app = App::new(60);
    let out = render(&app, 80, 24);
    assert!(out.contains("collecting"));
}

#[test]
fn survives_absurdly_small_terminal() {
    let mut app = App::new(60);
    app.push(sample(50.0));
    // Every panel is narrower than its own title here.
    for (w, h) in [(20, 10), (10, 6), (4, 3), (1, 1)] {
        render(&app, w, h);
    }
}

#[test]
fn paused_state_is_visibly_marked() {
    let mut app = App::new(60);
    // Oldest first, one second apart, newest last.
    for i in (0..10).rev() {
        app.push(sample_at((9 - i) as f32 * 10.0, i));
    }
    app.history.scrub(-4);
    let out = render(&app, 100, 30);
    assert!(out.contains("PAUSED"), "paused view must not look live");
    assert!(
        out.contains("-4s"),
        "paused badge must report real elapsed lag"
    );
    assert!(
        out.contains('▌') || out.contains('▐'),
        "scrub cursor must be visible without colour"
    );
}

#[test]
fn filter_narrows_the_table() {
    let mut app = App::new(60);
    app.push(sample(10.0));
    app.filter = "postgres".into();
    let out = render(&app, 100, 30);
    assert!(out.contains("postgres"));
    assert!(!out.contains("nginx"));
}

#[test]
fn sort_by_mem_puts_the_biggest_process_first() {
    let mut app = App::new(60);
    app.push(sample(10.0));
    app.sort = crate::app::Sort::Mem;
    // Bound, not chained: a row's process is a `Cow` now, so borrowing through
    // a temporary `Vec` does not outlive it.
    let rows = app.visible_rows();
    let names: Vec<&str> = rows.iter().map(|r| r.proc.name.as_ref()).collect();
    assert_eq!(names, vec!["postgres", "nginx", "init"]);
}

#[test]
fn a_filter_that_hides_the_watched_process_does_not_claim_it_stopped() {
    // Not in the rows is not the same as not in the sample. This test used to
    // assert the opposite — that a filtered-out process was announced as `nginx
    // not running here` — which is the panel making a claim that is false about
    // the machine, above a row the reader can see by clearing the filter.
    // Nothing needs saying: they typed the filter, and it is on screen.
    let mut app = App::new(60);
    app.push(sample(10.0));
    app.select_delta(2);
    let watched = app.selected.clone().expect("nothing selected");

    app.filter = "nginx".into();
    let rows = app.visible_rows();
    assert_eq!(
        app.selected.as_ref(),
        Some(&watched),
        "the filter moved the selection to a different process"
    );
    assert!(
        app.row_of(&rows).is_none(),
        "the watched process is filtered out and still highlighted"
    );
    assert!(
        app.watched_but_absent(&rows).is_none(),
        "a filtered process was reported as not running"
    );
    let frame = render(&app, 100, 30);
    assert!(
        !frame.contains("not running here"),
        "the panel says a running process stopped"
    );

    // Clearing the filter brings it back, still selected.
    app.filter.clear();
    let rows = app.visible_rows();
    let i = app
        .row_of(&rows)
        .expect("the selection did not survive the filter");
    assert_eq!(rows[i].proc.command(), &**watched.name());
}

#[test]
#[ignore = "visual check: cargo test -- --ignored --nocapture show_frame"]
fn show_frame() {
    let mut app = App::new(600);
    for i in 0..300 {
        let t = i as f32;
        let mut s = sample_at((t * 0.7).sin().abs() * 95.0, 300 - i);
        s.mem.used = ((8.0 + (t * 0.2).sin() * 3.0) as u64) << 30;
        app.push(s);
    }
    app.history.scrub(-18);

    for (label, zoom_steps, set) in [
        ("braille, zoom 1", 0, crate::glyphs::GlyphSet::Braille),
        ("braille, zoomed out", 3, crate::glyphs::GlyphSet::Braille),
        ("ascii fallback", 0, crate::glyphs::GlyphSet::Ascii),
    ] {
        let mut a = App::new(600);
        for i in 0..300 {
            let t = i as f32;
            let mut s = sample_at((t * 0.7).sin().abs() * 95.0, 300 - i);
            s.mem.used = ((8.0 + (t * 0.2).sin() * 3.0) as u64) << 30;
            a.push(s);
        }
        a.history.scrub(-18);
        a.glyphs = set;
        for _ in 0..zoom_steps {
            a.zoom_out();
        }
        println!("\n=== {label} ===");
        let mut term = Terminal::new(TestBackend::new(100, 12)).unwrap();
        term.draw(|f| ui::draw_timeline_for_test(f, f.area(), &a))
            .unwrap();
        let buf = term.backend().buffer();
        for y in 0..buf.area.height {
            let row: String = (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect();
            println!("{}", row.trim_end());
        }
    }
}

#[test]
fn cursor_marker_picks_the_correct_half_of_a_cell() {
    // Braille packs two samples per cell, so the marker has to distinguish
    // them or scrubbing loses half its precision.
    let mut app = App::new(60);
    for i in (0..8).rev() {
        app.push(sample_at(10.0, i));
    }
    app.glyphs = crate::glyphs::GlyphSet::Braille;

    // Newest is slot 7 (right half of cell 3); one back is slot 6 (left half).
    app.history.scrub(-1);
    assert!(
        render(&app, 100, 30).contains('▌'),
        "odd offset is a left half"
    );
    app.history.scrub(-1);
    assert!(
        render(&app, 100, 30).contains('▐'),
        "even offset is a right half"
    );
}

#[test]
fn ascii_glyphs_render_without_any_unicode() {
    let mut app = App::new(60);
    for i in (0..20).rev() {
        app.push(sample_at(80.0, i));
    }
    app.glyphs = crate::glyphs::GlyphSet::Ascii;
    app.history.scrub(-3);
    let out = render(&app, 100, 30);
    // Box-drawing borders are ratatui's; the graph area itself must be plain.
    assert!(out.contains('#'), "ascii set must draw a filled bar");
    assert!(!out.contains('⣿'), "ascii set must not emit braille");
    assert!(out.contains('^'), "ascii cursor marker");
}

#[test]
fn zooming_out_widens_the_time_span_shown() {
    let mut app = App::new(600);
    for i in (0..400).rev() {
        app.push(sample_at(50.0, i));
    }
    // At zoom 1 a 40-column terminal cannot show 400 samples; zoomed out it can.
    let narrow = 40;
    let at_zoom_1 = render(&app, narrow, 30);
    for _ in 0..4 {
        app.zoom_out();
    }
    let at_max_zoom = render(&app, narrow, 30);
    assert!(app.zoom() > 1);
    assert_ne!(at_zoom_1, at_max_zoom, "zoom must change what is drawn");
}

#[test]
fn zoom_is_clamped_at_both_ends() {
    let mut app = App::new(60);
    for _ in 0..20 {
        app.zoom_in();
    }
    assert_eq!(app.zoom(), crate::app::ZOOM_LEVELS[0]);
    for _ in 0..20 {
        app.zoom_out();
    }
    assert_eq!(app.zoom(), *crate::app::ZOOM_LEVELS.last().unwrap());
}

#[test]
fn a_spike_survives_aggregation_at_every_zoom_level() {
    // The core promise: zooming out must never hide a spike.
    for &z in crate::app::ZOOM_LEVELS.iter() {
        let mut app = App::new(600);
        for i in (0..120).rev() {
            // One 100% sample buried in otherwise idle history.
            app.push(sample_at(if i == 60 { 100.0 } else { 0.0 }, i));
        }
        while app.zoom() < z {
            app.zoom_out();
        }
        let out = render(&app, 100, 30);
        assert!(
            out.contains('⣿') || out.contains('⡇') || out.contains('⢸'),
            "spike vanished at zoom {z}"
        );
    }
}

#[test]
fn zoom_is_clamped_to_what_the_buffer_can_fill() {
    use crate::app::effective_zoom;
    // 300 samples over 196 slots needs 2 per slot; asking for 8 would shrink
    // the graph into a corner and leave most of the width blank.
    assert_eq!(effective_zoom(8, 300, 196), 2);
    assert_eq!(effective_zoom(8, 600, 196), 4);
    // A narrow terminal genuinely needs the higher levels.
    assert_eq!(effective_zoom(8, 600, 80), 8);
    // Never below 1, and never divides by zero.
    assert_eq!(effective_zoom(1, 0, 196), 1);
    assert_eq!(effective_zoom(4, 600, 0), 4);
}

#[test]
fn timeline_fills_its_panel_with_no_blank_rows() {
    // The acceptance test for L1. Its bounds previously skipped the border
    // rows and column — which after L1 are exactly the space the change
    // reclaimed, so a regression leaving the last row blank would have passed.
    let (w, h) = (60u16, 10u16);
    let mut app = App::new(600);
    for i in (0..200).rev() {
        app.push(sample_at(50.0, i));
    }
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);

    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    term.draw(|f| ui::draw_timeline_for_test(f, f.area(), &app))
        .unwrap();
    let buf = term.backend().buffer();

    // Every row, including the first (the section rule) and the last.
    for y in 0..h {
        let row: String = (0..w).map(|x| buf[(x, y)].symbol()).collect();
        assert!(
            row.chars().any(|c| c != ' '),
            "row {y} of the timeline is blank:\n{row:?}"
        );
    }
    // And column 0, which used to be a border, now carries content.
    let col0: String = (1..h).map(|y| buf[(0, y)].symbol()).collect();
    assert!(
        col0.chars().any(|c| c != ' '),
        "column 0 is unused: {col0:?}"
    );
}

#[test]
fn tree_mode_renders_nesting_in_the_table() {
    let mut app = App::new(60);
    let mut s = sample(10.0);
    // postgres(42) parents nginx(99); init(1) parents postgres.
    s.procs = vec![
        proc_named(1, "init", 0.1, 1 << 20),
        proc_named(42, "postgres", 88.0, 512 << 20),
        proc_named(99, "nginx", 12.5, 32 << 20),
    ];
    s.procs[1].ppid = 1;
    s.procs[2].ppid = 42;
    app.push(s);
    app.tree = true;

    let out = render(&app, 100, 30);
    assert!(out.contains("tree"), "title should say the tree is on");
    assert!(out.contains("└─ postgres") || out.contains("├─ postgres"));
    assert!(out.contains("nginx"));
}

#[test]
fn tree_mode_keeps_every_process_visible() {
    let mut app = App::new(60);
    app.push(sample(10.0));
    let flat = app.visible_rows().len();
    app.tree = true;
    assert_eq!(app.visible_rows().len(), flat, "tree must not drop rows");
}

#[test]
fn tree_survives_a_tiny_terminal() {
    let mut app = App::new(60);
    app.push(sample(10.0));
    app.tree = true;
    for (w, h) in [(20, 10), (4, 3), (1, 1)] {
        render(&app, w, h);
    }
}

#[test]
#[ignore = "visual check: cargo test -- --ignored --nocapture show_real_tree"]
fn show_real_tree() {
    use crate::collect::{Collector, Platform};
    let mut c = Platform::new().unwrap();
    let mut app = App::new(60);
    app.push(c.sample(Default::default()).unwrap());
    app.tree = true;
    app.sort = crate::app::Sort::Pid;

    let rows = app.visible_rows();
    println!(
        "{} processes, {} rows",
        app.history.current().unwrap().procs.len(),
        rows.len()
    );
    let depth = |r: &crate::tree::TreeRow| r.prefix.chars().count() / 3;
    println!("max depth: {}", rows.iter().map(depth).max().unwrap_or(0));
    println!(
        "roots: {}",
        rows.iter().filter(|r| r.prefix.is_empty()).count()
    );
    for r in rows.iter().take(28) {
        println!("{:>7} {}{}", r.proc.pid, r.prefix, r.proc.name);
    }
}

#[test]
fn io_columns_are_there_before_anyone_asks() {
    // The header may have just said the machine is blocked on IO, and the
    // table is where the culprit is named. A default that hides it makes the
    // default view unable to answer the question the default view raised.
    let mut app = App::new(60);
    app.push(sample(10.0));
    assert!(
        render(&app, 120, 30).contains("DISK"),
        "the columns are hidden by default"
    );
    app.toggle_io();
    assert!(
        !render(&app, 120, 30).contains("DISK"),
        "the key cannot hide them"
    );
}

#[test]
fn io_collection_is_a_ratchet() {
    use crate::collect::Source;
    // Collection starts with the columns, which are on by default.
    let mut app = App::new(60);
    assert!(app.needs().asked(Source::Io));

    // Hiding the columns must NOT stop collection: resuming later would leave
    // a hole in the middle of history rather than one clean boundary.
    app.toggle_io();
    assert!(!app.show_io);
    assert!(app.needs().asked(Source::Io), "collection must not stop");

    // …and showing them again changes nothing, because it never stopped.
    app.toggle_io();
    assert!(app.needs().asked(Source::Io));

    // The one thing that does stop it is the probe deciding the column is
    // unreadable — a boundary at the very start rather than in the middle.
    let mut probed = App::new(60);
    probed.probe_io(&with_denied(100, 90));
    assert!(!probed.needs().asked(Source::Io));
}

#[test]
fn history_without_io_says_so_rather_than_showing_zero() {
    let mut app = App::new(60);
    let mut old = sample(10.0);
    old.io_collected = false; // recorded before the column was switched on
    app.push(old);

    let out = render(&app, 120, 30);
    assert!(out.contains("not collected"), "must explain the blank");
    assert!(out.contains('·'), "blank marker, never a fabricated 0");
}

#[test]
fn unreadable_processes_are_blank_not_zero() {
    let mut app = App::new(60);
    let mut s = sample(10.0);
    s.io_collected = true;
    s.procs[0].io = Some(crate::sample::IoRates {
        read: 2048,
        write: 0,
    });
    // procs[1] and [2] stay None: readable by root only.
    s.io_denied = 2;
    app.push(s);

    let out = render(&app, 120, 30);
    assert!(out.contains("2.0K/s"), "a real rate renders as a rate");
    assert!(out.contains('—'), "unreadable renders as a dash");
    assert!(
        out.contains("2/3 need root"),
        "title should explain the dashes"
    );
}

#[test]
fn only_unreadable_processes_prompt_for_root() {
    // A process merely awaiting its second reading also shows a dash, but it
    // resolves on its own; blaming permissions for that case would be wrong.
    let mut app = App::new(60);
    let mut s = sample(10.0);
    s.io_collected = true;
    s.io_denied = 0; // all readable, none has a prior counter yet
    app.push(s);
    app.toggle_io();

    let out = render(&app, 120, 30);
    assert!(out.contains('—'), "no prior reading still shows a dash");
    assert!(!out.contains("need root"), "but must not blame permissions");
}

#[test]
#[ignore = "digest for cross-branch comparison: cargo test -- --ignored --nocapture render_digest"]
fn render_digest() {
    // Symbol *and* style per cell, so a colour change shows up. Used to prove
    // a refactor is visually a no-op.
    //
    // Deliberately hermetic: timestamps come from a fixed epoch rather than
    // `now()`, because the header renders elapsed lag and a scheduler stall
    // mid-loop would tick it over a second and change the hash — a false
    // "rendering changed" verdict during exactly the comparison this exists
    // for.
    //
    // Also renders every state that owns a distinct token. A digest that never
    // enters the live branch or the filter prompt would stay unchanged if
    // someone repainted them, and would then be quietly lying.
    fn fixed_sample(cpu: f32, age_secs: u64) -> Sample {
        let mut s = sample(cpu);
        s.at = std::time::SystemTime::UNIX_EPOCH
            + std::time::Duration::from_secs(1_700_000_000 - age_secs);
        s.io_collected = true;
        s.io_denied = 1;
        s
    }

    let build = || {
        let mut app = App::new(600);
        for i in (0..120).rev() {
            app.push(fixed_sample((i as f32 * 0.7).sin().abs() * 95.0, i));
        }
        app
    };

    let mut out = String::new();
    for (label, prep) in [("paused+tree+io", 0u8), ("live", 1), ("filter-prompt", 2)] {
        let mut app = build();
        match prep {
            0 => {
                app.history.scrub(-9);
                app.toggle_io();
                app.tree = true;
            }
            1 => {} // stays live: exercises theme.live
            _ => {
                app.editing_filter = true;
                app.filter = "pg".into();
            }
        }
        let mut term = Terminal::new(TestBackend::new(110, 34)).unwrap();
        term.draw(|f| ui::draw(f, &app)).unwrap();
        let buf = term.backend().buffer();
        out.push_str(label);
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                let c = &buf[(x, y)];
                out.push_str(&format!(
                    "{}|{:?}|{:?}|{:?};",
                    c.symbol(),
                    c.fg,
                    c.bg,
                    c.modifier
                ));
            }
        }
    }

    // A hash keeps the output to one line; any cell difference changes it.
    let mut h: u64 = 1469598103934665603;
    for b in out.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(1099511628211);
    }
    println!("RENDER_DIGEST {h:016x}");
}

#[test]
fn mono_tier_emits_no_colour_anywhere_on_screen() {
    // The acceptance criterion for the tier: not "mostly grey", but that no
    // rendered cell carries a colour at all.
    let mut app = App::new(60);
    let mut s = sample(90.0);
    s.io_collected = true;
    s.io_denied = 1;
    app.push(s);
    app.theme = Theme::new(Palette::Classic, Tier::Mono);
    app.toggle_io();
    app.tree = true;
    app.history.scrub(-1);

    let mut term = Terminal::new(TestBackend::new(110, 30)).unwrap();
    term.draw(|f| ui::draw(f, &app)).unwrap();
    let buf = term.backend().buffer();
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            let c = &buf[(x, y)];
            assert_eq!(
                (c.fg, c.bg),
                (ratatui::style::Color::Reset, ratatui::style::Color::Reset),
                "cell ({x},{y}) {:?} carries colour at the mono tier",
                c.symbol()
            );
        }
    }
}

#[test]
fn mono_tier_still_marks_the_paused_state() {
    // Losing colour must not lose the loudest warning in the UI.
    let mut app = App::new(60);
    for i in (0..6).rev() {
        app.push(sample_at(50.0, i));
    }
    app.theme = Theme::new(Palette::Classic, Tier::Mono);
    app.history.scrub(-3);

    let mut term = Terminal::new(TestBackend::new(100, 30)).unwrap();
    term.draw(|f| ui::draw(f, &app)).unwrap();
    let buf = term.backend().buffer();
    let reversed = (0..buf.area.width).any(|x| {
        buf[(x, 0)]
            .modifier
            .contains(ratatui::style::Modifier::REVERSED)
    });
    assert!(reversed, "PAUSED badge must stay loud without colour");
}

#[test]
fn every_tier_renders() {
    for tier in [Tier::Mono, Tier::Ansi16, Tier::Ansi256, Tier::TrueColor] {
        let mut app = App::new(60);
        app.push(sample(75.0));
        app.theme = Theme::new(Palette::Classic, tier);
        let out = render(&app, 100, 30);
        assert!(out.contains("postgres"), "{tier:?} failed to render");
        // And at a size where everything is fighting for room.
        render(&app, 20, 8);
    }
}

#[test]
fn coloured_tiers_actually_differ_from_mono() {
    // Guards against a tier that silently resolves to the same styling, which
    // would make the mono test above pass for the wrong reason.
    let styled = |tier| {
        let mut app = App::new(60);
        app.push(sample(90.0));
        app.theme = Theme::new(Palette::Classic, tier);
        let mut term = Terminal::new(TestBackend::new(100, 30)).unwrap();
        term.draw(|f| ui::draw(f, &app)).unwrap();
        let buf = term.backend().buffer();
        (0..buf.area.height)
            .flat_map(|y| (0..buf.area.width).map(move |x| (x, y)))
            .map(|(x, y)| format!("{:?}{:?}", buf[(x, y)].fg, buf[(x, y)].bg))
            .collect::<String>()
    };
    let mono = styled(Tier::Mono);
    assert_ne!(mono, styled(Tier::Ansi16));
    assert_ne!(mono, styled(Tier::TrueColor));
    assert_ne!(styled(Tier::Ansi16), styled(Tier::TrueColor));
}

#[test]
#[ignore = "visual check: cargo test -- --ignored --nocapture show_tiers"]
fn show_tiers() {
    // Prints the frame with a modifier map under the header, so the monochrome
    // tier can be eyeballed for whether meaning survives without colour.
    use ratatui::style::Modifier;
    for tier in [Tier::Mono, Tier::TrueColor] {
        let mut app = App::new(600);
        for i in (0..200).rev() {
            app.push(sample_at((i as f32 * 0.7).sin().abs() * 95.0, i));
        }
        app.theme = Theme::new(Palette::Classic, tier);
        app.history.scrub(-8);
        let mut term = Terminal::new(TestBackend::new(96, 26)).unwrap();
        term.draw(|f| ui::draw(f, &app)).unwrap();
        let buf = term.backend().buffer();
        println!("\n=== {tier:?} ===");
        for y in 0..buf.area.height {
            let row: String = (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect();
            println!("{}", row.trim_end());
            if y < 2 {
                let mods: String = (0..buf.area.width)
                    .map(|x| {
                        let m = buf[(x, y)].modifier;
                        if m.contains(Modifier::REVERSED) {
                            'R'
                        } else if m.contains(Modifier::BOLD) {
                            'B'
                        } else if m.contains(Modifier::DIM) {
                            'd'
                        } else {
                            ' '
                        }
                    })
                    .collect();
                if !mods.trim().is_empty() {
                    println!("{}", mods.trim_end());
                }
            }
        }
    }
}

#[test]
fn the_default_theme_is_the_colour_vision_safe_one() {
    // The point of C3: safe is the default, classic is the escape hatch, not
    // the other way round.
    let app = App::new(60);
    assert_eq!(app.theme, Theme::default());
    assert_eq!(Palette::default(), Palette::Safe);
    assert_ne!(app.theme.ok, ratatui::style::Color::Green);
}

#[test]
fn every_section_rule_uses_the_chrome_token() {
    // Replaces the panel-border test: L1 removed the boxes. Identifies a rule
    // by its shape — a row that is mostly `─` — rather than by hardcoded row
    // numbers, so it keeps working as sections move. The tree spine also draws
    // `─`, but only a glyph or two per row, so the majority test excludes it.
    //
    // Tree mode is on for exactly that reason.
    let mut app = App::new(60);
    app.push(sample(50.0));
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);
    app.tree = true;
    // The IO columns push the table past a hundred columns, and a clipped
    // panel has no rule to find. This test is about the rules, not the width.
    app.toggle_io();

    let (w, h) = (100u16, 30u16);
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    term.draw(|f| ui::draw(f, &app)).unwrap();
    let buf = term.backend().buffer();

    // A rule is a row that *begins* with the rule glyph, not one that is
    // mostly made of it. The majority test was a proxy, and it was marginal:
    // adding eleven characters to the processes title pushed that row under
    // half and the rule stopped being counted while it was still being drawn.
    // The tree spine draws `─` too, but indented and never at column zero.
    let mut rules = 0;
    for y in 0..h {
        if buf[(0, y)].symbol() != "─" {
            continue;
        }
        rules += 1;
        for x in 0..w {
            let c = &buf[(x, y)];
            if c.symbol() == "─" {
                assert_eq!(
                    c.fg, app.theme.chrome,
                    "section rule at ({x},{y}) is not chrome-coloured"
                );
            }
        }
    }
    // Timeline and processes. The cores section folded into the header in L2,
    // so it has no rule of its own any more.
    assert!(rules >= 2, "expected a rule per section, saw {rules}");
}

#[test]
fn the_tree_spine_recedes_like_a_gridline() {
    // The spine is structure, not data. It shares a cell with the process
    // name, so this checks the two are styled differently rather than the
    // whole cell inheriting one style.
    let mut app = App::new(60);
    let mut s = sample(10.0);
    s.procs[1].ppid = 1;
    s.procs[2].ppid = 42;
    app.push(s);
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);
    app.tree = true;

    let mut term = Terminal::new(TestBackend::new(100, 30)).unwrap();
    term.draw(|f| ui::draw(f, &app)).unwrap();
    let buf = term.backend().buffer();

    let mut spine = 0;
    for y in 0..30 {
        // Column 0 is the panel edge; the spine lives inside the COMMAND cell.
        for x in 1..99u16 {
            let c = &buf[(x, y)];
            // Both halves of the prefix. They share a span today, but the
            // test should state what its name claims rather than rely on that.
            if matches!(c.symbol(), "├" | "└" | "─") {
                // Only the spine inside the COMMAND column, not section rules.
                if x > 40 {
                    spine += 1;
                    assert_eq!(
                        c.fg, app.theme.chrome,
                        "tree spine at ({x},{y}) is not chrome-coloured"
                    );
                }
            }
        }
    }
    assert!(spine > 0, "expected a tree spine to be drawn, saw none");
}

/// Rows of the timeline panel that contain a chrome-styled glyph, excluding the
/// panel edges. Used to locate the threshold rules precisely.
fn rule_rows(app: &App, w: u16, h: u16) -> Vec<usize> {
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    term.draw(|f| ui::draw_timeline_for_test(f, f.area(), app))
        .unwrap();
    let buf = term.backend().buffer();
    (1..h)
        .filter(|&y| {
            (0..w).any(|x| {
                let c = &buf[(x, y)];
                c.fg == app.theme.chrome && c.symbol() != " "
            })
        })
        .map(|y| (y - 1) as usize)
        .collect()
}

#[test]
fn the_rules_land_on_exactly_the_threshold_rows() {
    // An earlier version asserted only that *some* cell was chrome-coloured —
    // which the panel border satisfies, so it passed with the rule removed
    // entirely. This pins the exact rows, so it cannot.
    //
    // Both graphs scale to their own peak, so the expectation has to use the
    // same ceiling the renderer picks: a threshold above the ceiling draws no
    // rule at all, which is the point of the scaling.
    let (w, h) = (100u16, 12u16);
    // Memory stays low so its ceiling puts both thresholds off its scale and
    // only the CPU graph contributes rules. At 50% its 50 threshold would sit
    // exactly on the ceiling *and* under the data, which data correctly
    // occludes — a real behaviour, but not the one this test is about.
    let (cpu_pct, mem_frac) = (95.0_f32, 0.05_f32);
    let mut app = App::new(600);
    for i in (0..200).rev() {
        // A spike lifts the CPU ceiling to 100 so both thresholds are on its
        // scale; memory sits flat at half the machine.
        let mut s = sample_at(if i == 100 { cpu_pct } else { 5.0 }, i);
        s.mem.used = ((s.mem.total as f32) * mem_frac) as u64;
        app.push(s);
    }
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);

    // Same arithmetic the renderer uses, so the expectation tracks the layout.
    let graph_rows = (h as usize - 1).saturating_sub(2).max(1);
    let cpu_rows = (graph_rows * 3 / 5).max(1);
    let mem_rows = graph_rows - cpu_rows;
    let cpu_ceiling = crate::glyphs::ceiling_for(cpu_pct);
    let mem_ceiling = crate::glyphs::ceiling_for(mem_frac * 100.0);

    let mut expected: Vec<usize> = Vec::new();
    for pct in [app.theme.warn_pct, app.theme.critical_pct] {
        if let Some((r, _)) = crate::glyphs::rule_position_scaled(pct, cpu_rows, cpu_ceiling) {
            expected.push(r);
        }
        if let Some((r, _)) = crate::glyphs::rule_position_scaled(pct, mem_rows, mem_ceiling) {
            expected.push(cpu_rows + r);
        }
    }
    expected.sort_unstable();
    expected.dedup();

    assert!(
        !expected.is_empty(),
        "no rules expected — test proves nothing"
    );
    assert_eq!(rule_rows(&app, w, h), expected);
}

#[test]
fn both_thresholds_get_a_rule_not_just_critical() {
    // The warn boundary is the one the roadmap asked for; it was hue-only.
    let (w, h) = (100u16, 16u16);
    let mut app = App::new(600);
    for i in (0..200).rev() {
        // One spike lifts the ceiling to 100 so both thresholds are on the
        // visible scale; the rest stays low so there is empty space to draw
        // the rules into.
        let mut s = sample_at(if i == 100 { 95.0 } else { 5.0 }, i);
        s.mem.used = if i == 100 { 15 << 30 } else { 1 << 30 };
        app.push(s);
    }
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);
    let rows = rule_rows(&app, w, h);
    assert!(
        rows.len() >= 4,
        "expected warn and critical rules in both graphs, found rows {rows:?}"
    );
}

#[test]
fn the_rule_is_dashed_so_it_cannot_be_read_as_data() {
    // At the mono tier chrome and a low bar are both dim, and a solid rule row
    // renders the same glyph a level-1 bar does. Dashing is what separates a
    // reference line from a row of samples when colour is unavailable.
    let (w, h) = (100u16, 12u16);
    let mut app = App::new(600);
    for i in (0..200).rev() {
        // A spike puts the ceiling at 100 so the critical rule is on-scale.
        app.push(sample_at(if i == 100 { 95.0 } else { 10.0 }, i));
    }
    app.theme = Theme::new(Palette::Safe, Tier::Mono);

    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    term.draw(|f| ui::draw_timeline_for_test(f, f.area(), &app))
        .unwrap();
    let buf = term.backend().buffer();

    let rule_y = 1 + crate::glyphs::rule_position_scaled(
        app.theme.critical_pct,
        (((h as usize - 1).saturating_sub(2)).max(1) * 3 / 5).max(1),
        100.0,
    )
    .unwrap()
    .0 as u16;

    // U+2800 is the braille blank: visually empty, but not an ASCII space, so
    // it must be counted as a gap or every braille row looks solid.
    let blank = |s: &str| s == " " || s == "\u{2800}";
    let row: Vec<String> = (1..w - 1)
        .map(|x| buf[(x, rule_y)].symbol().to_string())
        .collect();
    let marks = row.iter().filter(|s| !blank(s)).count();
    let blanks = row.iter().filter(|s| blank(s)).count();
    assert!(marks > 10, "rule row has no marks: {marks}");
    assert!(
        blanks > 10,
        "rule row is solid, indistinguishable from a bar at the mono tier"
    );
}

#[test]
fn data_always_wins_the_cell_over_the_rule() {
    // An earlier version OR'd the rule into the bar glyph, so a cell holding a
    // spike and an idle sample lit a dot at the rule height in the data
    // colour — identical to the idle sample having crossed the threshold.
    let (w, h) = (100u16, 12u16);
    let mut app = App::new(600);
    for i in (0..200).rev() {
        let mut s = sample_at(100.0, i);
        s.mem.used = s.mem.total; // both graphs full, so no cell is empty
        app.push(s);
    }
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);
    assert!(
        rule_rows(&app, w, h).is_empty(),
        "rule drew over cells that contain data"
    );
}

/// Column of the scrub cursor marker in a rendered timeline, if drawn.
fn cursor_column(app: &App, w: u16, h: u16) -> Option<u16> {
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    term.draw(|f| ui::draw_timeline_for_test(f, f.area(), app))
        .unwrap();
    let buf = term.backend().buffer();
    for y in 0..h {
        for x in 0..w {
            if matches!(buf[(x, y)].symbol(), "▌" | "▐" | "^") {
                return Some(x);
            }
        }
    }
    None
}

#[test]
fn the_cursor_stays_over_its_own_column_once_the_gutter_exists() {
    // The gutter shifts the graph right; if the cursor row is not padded by
    // the same amount the marker points four columns off the sample it claims.
    //
    // Asserted as an exact column, computed the way the renderer computes it.
    // An approximate assertion ("right of the gutter", "past halfway") passed
    // happily with the padding removed — verified — which is no test at all.
    let (w, h) = (100u16, 12u16);
    let n = 40usize;
    let mut app = App::new(600);
    for i in (0..n).rev() {
        app.push(sample_at(50.0, i as u64));
    }
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);
    app.history.scrub(-7);

    let gutter = 4usize;
    let graph_w = w as usize - gutter;
    let spc = app.glyphs.samples_per_cell();
    let slots = graph_w * spc;
    let zoom = crate::app::effective_zoom(app.zoom(), n, slots);
    let shown = (slots * zoom).min(n);
    let dropped = app.history.len() - shown;
    let idx = app.history.cursor_index() - dropped;
    let slot = crate::history::slot_of_index(idx, shown, zoom, slots);
    let expected = gutter as u16 + (slot / spc) as u16;

    assert_eq!(
        cursor_column(&app, w, h),
        Some(expected),
        "cursor marker is not over the sample it points at"
    );
}

/// The gutter columns of every graph row, as one string.
///
/// Scoped to the gutter rather than the whole frame: asserting on the full
/// render made the narrow case depend on the panel title never containing the
/// digits "100", which is unrelated to what the test is about.
fn gutter_text(app: &App, w: u16, h: u16) -> String {
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    term.draw(|f| ui::draw_timeline_for_test(f, f.area(), app))
        .unwrap();
    let buf = term.backend().buffer();
    let graph_rows = (h as usize - 1).saturating_sub(2).max(1);
    (0..graph_rows)
        .map(|row| {
            // The gutter's own width, not a literal. Hardcoding four meant
            // widening the gutter for a longer series name broke four tests
            // that were not about the gutter's width at all.
            (0..(ui::GUTTER_W as u16 - 1).min(w))
                .map(|x| buf[(x, 1 + row as u16)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn the_gutter_carries_the_scale_and_yields_on_a_narrow_panel() {
    let mut app = App::new(600);
    for i in (0..50).rev() {
        app.push(sample_at(50.0, i));
    }
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);

    let wide = gutter_text(&app, 100, 12);
    // The top anchor is the axis ceiling, which scales to the data.
    assert!(
        wide.chars().any(|c| c.is_ascii_digit()),
        "wide panel lost the top anchor:\n{wide}"
    );
    assert!(
        wide.contains('0'),
        "wide panel lost the zero anchor:\n{wide}"
    );

    // Four columns of axis is a poor trade against four columns of history
    // when there is barely any room.
    let narrow = gutter_text(&app, 24, 12);
    assert!(
        !narrow.chars().any(|c| c.is_ascii_digit()),
        "narrow panel should drop the gutter, got:\n{narrow}"
    );
}

#[test]
fn a_section_too_short_for_both_ends_carries_no_axis_at_all() {
    // A one-row section spans the whole 0..100 range. Labelling its top `100`
    // implies the bottom is not zero, and `0` never appears anywhere — the
    // axis states something false rather than merely being absent.
    //
    // Checked per section: the two graphs are sized independently, so a short
    // CPU section can sit above a MEM section that legitimately has a scale.
    let mut app = App::new(600);
    for i in (0..50).rev() {
        app.push(sample_at(50.0, i));
    }
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);

    for h in [5u16, 6, 7, 8, 12] {
        let rows: Vec<String> = gutter_text(&app, 60, h)
            .lines()
            .map(str::to_string)
            .collect();
        let graph_rows = (h as usize - 1).saturating_sub(2).max(1);
        // Asked of the renderer rather than recomputed. A second copy of the
        // split is a second thing to keep in step, and it was already wrong
        // once the series stopped being a fixed pair.
        let split = ui::sections(graph_rows, 2, ui::GUTTER_W);
        let cpu_rows = split[0];
        let mem_rows = split.get(1).copied().unwrap_or(0);

        for (name, range, n) in [
            ("cpu", 0..cpu_rows, cpu_rows),
            ("mem", cpu_rows..graph_rows, mem_rows),
        ] {
            let section: String = rows.get(range).unwrap_or_default().join("");
            let labelled = section.chars().any(|c| c.is_ascii_digit());
            assert_eq!(
                labelled,
                n >= 2,
                "h={h}: {name} section of {n} row(s) labelled={labelled}, in:\n{section}"
            );
        }
    }
}

#[test]
fn the_gutter_never_overlaps_the_graph() {
    // Every graph row must start with the gutter, so no glyph can be drawn
    // under the axis labels.
    let (w, h) = (100u16, 12u16);
    let mut app = App::new(600);
    for i in (0..200).rev() {
        app.push(sample_at(100.0, i)); // saturated: bars everywhere
    }
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);

    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    term.draw(|f| ui::draw_timeline_for_test(f, f.area(), &app))
        .unwrap();
    let buf = term.backend().buffer();

    let graph_rows = (h as usize - 1).saturating_sub(2).max(1);
    for row in 0..graph_rows {
        let y = 1 + row as u16;
        for x in 0..4u16 {
            let s = buf[(x, y)].symbol();
            // The concrete allowed set, not "any alphanumeric": a future glyph
            // set that used a letter would otherwise pass this silently.
            let ok = s == " "
                || s.chars().all(|c| c.is_ascii_digit())
                || matches!(s, "C" | "P" | "U" | "M" | "E");
            assert!(ok, "unexpected glyph {s:?} inside the gutter at ({x},{y})");
        }
    }
}

#[test]
fn the_gutter_names_each_series_directly() {
    // A direct label beats a legend: the reader stops having to hold
    // "top is cpu" in their head while reading the graph.
    let mut app = App::new(600);
    for i in (0..50).rev() {
        app.push(sample_at(50.0, i));
    }
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);

    let g = gutter_text(&app, 100, 12);
    assert!(g.contains("CPU"), "cpu graph is unlabelled:\n{g}");
    assert!(g.contains("MEM"), "mem graph is unlabelled:\n{g}");
}

#[test]
fn the_legend_keeps_identifying_the_series_when_the_gutter_cannot() {
    // The identification has to live somewhere. When a section is too short to
    // carry a label, dropping the legend line too would leave the reader with
    // two anonymous graphs.
    let mut app = App::new(600);
    for i in (0..50).rev() {
        app.push(sample_at(50.0, i));
    }
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);

    let whole = |w: u16, h: u16| {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| ui::draw_timeline_for_test(f, f.area(), &app))
            .unwrap();
        let buf = term.backend().buffer();
        (0..h)
            .map(|y| (0..w).map(|x| buf[(x, y)].symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    };

    // Tall: gutter labels present, legend sheds its identification half.
    let tall = whole(100, 14);
    assert!(tall.contains("CPU"));
    assert!(
        !tall.contains("cpu · mem"),
        "legend duplicates the gutter label"
    );

    // Narrow: no gutter at all, so the legend must still say which is which.
    let narrow = whole(24, 14);
    assert!(
        !narrow.contains("CPU"),
        "narrow panel should have no gutter"
    );
    assert!(
        narrow.contains("cpu · mem"),
        "narrow panel dropped both the label and the legend:\n{narrow}"
    );
}

#[test]
#[ignore = "regenerates the README sample frame"]
fn readme_frame() {
    let mut app = App::new(600);
    for i in 0..300 {
        let x = i as f32;
        let mut s = sample_at((x * 0.7).sin().abs() * 95.0, 300 - i);
        s.mem.used = ((8.0 + (x * 0.2).sin() * 3.0) as u64) << 30;
        // A box that is stalled as well as busy, because a screenshot of a
        // monitor should show the thing the monitor is for.
        s.iowait = Some((x * 0.11).sin().abs() * 55.0);
        s.running = Some(if i % 7 == 0 { 3 } else { 1 });
        s.blocked = Some(if i % 5 == 0 { 4 } else { 0 });
        s.procs = vec![
            proc_named(1, "systemd", 0.1, 12 << 20),
            proc_named(824, "postgres", 88.4, 512 << 20),
            proc_named(1190, "nginx", 12.5, 32 << 20),
            proc_named(2077, "node", 4.2, 148 << 20),
        ];
        app.push(s);
    }
    app.history.scrub(-18);
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);
    let mut term = Terminal::new(TestBackend::new(78, 20)).unwrap();
    term.draw(|f| ui::draw(f, &app)).unwrap();
    let buf = term.backend().buffer();
    for y in 0..buf.area.height {
        let row: String = (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect();
        println!("{}", row.trim_end());
    }
}

/// The whole timeline panel as text, one string per row.
fn timeline_rows(app: &App, w: u16, h: u16) -> Vec<String> {
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    term.draw(|f| ui::draw_timeline_for_test(f, f.area(), app))
        .unwrap();
    let buf = term.backend().buffer();
    (0..h)
        .map(|y| (0..w).map(|x| buf[(x, y)].symbol()).collect::<String>())
        .collect()
}

#[test]
fn the_header_reports_the_values_at_the_cursor_not_the_live_ones() {
    // This used to be about the timeline's own readout, which repeated what the
    // header was already showing two centimetres above it. The readout is gone;
    // the claim it was making is the header's, and still worth pinning — a
    // header showing the newest sample would contradict the process table
    // beside it, which does follow the cursor.
    let mut app = App::new(600);
    // Oldest 90%, newest 10%, so the two are impossible to confuse.
    for i in (0..40).rev() {
        app.push(sample_at(if i > 20 { 90.0 } else { 10.0 }, i));
    }
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);
    app.history.scrub(-35); // back into the 90% region

    let header = rows(&app, 100, 24)
        .into_iter()
        .find(|l| l.contains("CPU"))
        .expect("no header");
    assert!(
        header.contains("90.0%"),
        "the header shows the live value, not the cursor's: {header:?}"
    );
    assert!(!header.contains("10.0%"), "{header:?}");
}

#[test]
fn the_cursor_row_states_the_scale_and_repeats_no_figure() {
    // The row is positional. It used to carry `CPU 50.0%` as well — a copy of
    // what the header shows while scrubbing, two centimetres away — while the
    // slot size, which nothing else states, was dropped in that mode because
    // this row replaced the caption that used to carry it.
    let mut app = App::new(600);
    for i in (0..40).rev() {
        app.push(sample_at(50.0, i));
    }
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);

    for scrubbed in [false, true] {
        if scrubbed {
            app.history.scrub(-5);
        }
        let text = timeline_rows(&app, 100, 12).join("\n");
        assert!(
            !text.contains("CPU 50.0%"),
            "the cursor row repeats a figure the header is showing (scrubbed: {scrubbed}):\n{text}"
        );
        assert!(
            text.contains("/slot"),
            "the slot size is not stated (scrubbed: {scrubbed}):\n{text}"
        );
        // The anchors are what make the marker's position mean anything. An
        // anchor the marker lands in is dropped rather than half-overwritten —
        // a marker at the right edge already says `now` — so each is required
        // unless the cursor is standing in it.
        let marker = cursor_column(&app, 100, 12);
        if marker.is_none_or(|c| c >= 4) {
            assert!(text.contains("past"), "the past anchor is gone:\n{text}");
        }
        if marker.is_none_or(|c| c + 3 <= 100 - 3) {
            assert!(text.contains("now"), "the now anchor is gone:\n{text}");
        }
        // …and never a fragment of one, which names nothing at all.
        for fragment in ["▌ow", "▐ow", "pas▌", "pas▐", " ow ", " as "] {
            assert!(
                !text.contains(fragment),
                "an anchor was written through (scrubbed: {scrubbed}): {fragment:?}\n{text}"
            );
        }
    }
}

#[test]
fn the_readout_never_pushes_the_marker_off_its_column() {
    // Asserting `row.len() == w` cannot fail: TestBackend is a fixed grid
    // pre-filled with spaces and ratatui truncates an over-wide line, so an
    // overflowing row measures `w` either way. The observable consequence of
    // overflow is the marker sliding, so assert the marker's column directly.
    for w in [40u16, 60, 80, 100, 140] {
        for back in [1usize, 5, 20, 60] {
            let n = 80usize;
            let mut app = App::new(600);
            for i in (0..n).rev() {
                app.push(sample_at(50.0, i as u64));
            }
            app.theme = Theme::new(Palette::Safe, Tier::TrueColor);
            app.history.scrub(-(back as isize));

            let gutter = if w as usize >= 30 { 4 } else { 0 };
            let graph_w = w as usize - gutter;
            let spc = app.glyphs.samples_per_cell();
            let slots = graph_w * spc;
            let zoom = crate::app::effective_zoom(app.zoom(), n, slots);
            let shown = (slots * zoom).min(n);
            let dropped = app.history.len() - shown;
            if app.history.cursor_index() < dropped {
                continue; // off-window: covered by its own test
            }
            let idx = app.history.cursor_index() - dropped;
            let slot = crate::history::slot_of_index(idx, shown, zoom, slots);
            let expected = gutter as u16 + (slot / spc) as u16;

            assert_eq!(
                cursor_column(&app, w, 12),
                Some(expected),
                "w={w} back={back}: readout displaced the marker"
            );
        }
    }
}

#[test]
fn scrubbing_past_the_left_edge_scrolls_the_window() {
    // G4 made the off-window case honest — an explicit marker and no figures.
    // G7 removes the case: Home now scrolls the graph to the oldest samples, so
    // the cursor can be drawn where it actually is.
    let mut app = App::new(600);
    for i in (0..500).rev() {
        app.push(sample_at(if i > 400 { 11.0 } else { 88.0 }, i as u64));
    }
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);
    app.history.goto_oldest();

    let text = timeline_rows(&app, 100, 12).join("\n");
    assert!(
        !text.contains('◀'),
        "off-window marker shown when the window can reach the cursor:\n{text}"
    );
    // Checked by where the marker is, not by a figure beside it: the cursor row
    // no longer repeats the header's values. At the oldest sample the marker
    // belongs at the left edge of the graph, which is only true if the window
    // followed.
    let col = cursor_column(&app, 100, 12).expect("no cursor marker drawn");
    assert!(
        col <= ui::GUTTER_W as u16 + 1,
        "the window did not follow the cursor to the oldest sample: marker at {col}"
    );
}

#[test]
fn the_live_view_does_not_shuffle_while_the_cursor_is_inside_it() {
    // Scrolling on every keypress would make the graph slide sideways under
    // the reader. The window only moves once the cursor would leave it.
    let mut app = App::new(600);
    for i in (0..500).rev() {
        app.push(sample_at((i as f32 * 1.7) % 100.0, i as u64));
    }
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);

    let graph = |a: &App| timeline_rows(a, 100, 12)[1..5].join("\n");
    let live = graph(&app);
    // A few steps back: still inside the live-anchored window.
    app.history.scrub(-3);
    assert_eq!(
        graph(&app),
        live,
        "graph moved while the cursor was still on it"
    );

    // Far enough back to leave it: now it must follow.
    app.history.scrub(-400);
    assert_ne!(graph(&app), live, "graph failed to follow the cursor");
}

#[test]
fn zoom_still_works_at_any_scroll_position() {
    let mut app = App::new(600);
    for i in (0..500).rev() {
        app.push(sample_at((i as f32 * 1.7) % 100.0, i as u64));
    }
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);
    app.history.goto_oldest();

    let mut seen = std::collections::HashSet::new();
    for _ in 0..crate::app::ZOOM_LEVELS.len() {
        let rows = timeline_rows(&app, 100, 12);
        assert_eq!(rows.len(), 12);
        // The cursor must remain visible at every zoom level.
        assert!(
            rows.iter().any(|r| r.contains('▌') || r.contains('▐')),
            "cursor lost at zoom {}",
            app.zoom()
        );
        seen.insert(rows[1].clone());
        app.zoom_out();
    }
    assert!(seen.len() > 1, "zoom had no effect while scrolled back");
}

#[test]
fn the_caption_moves_aside_rather_than_being_written_through() {
    // The marker is placed last and wins its cell outright — a marker a caption
    // can overwrite is a marker that sometimes lies about where the cursor is.
    // So the caption has to go to whichever side of it has room.
    let mut app = App::new(600);
    for i in (0..20).rev() {
        app.push(sample_at(77.0, i));
    }
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);

    // Sweep the cursor across the whole graph: at every position the caption
    // must be intact and the marker must still be drawn.
    for back in 1..19 {
        app.history.goto_live();
        app.history.scrub(-back);
        let rows = timeline_rows(&app, 100, 12);
        let row = rows
            .iter()
            .find(|r| r.contains('▌') || r.contains('▐'))
            .unwrap_or_else(|| panic!("no cursor row at -{back}: {rows:?}"));
        assert!(
            row.contains("/slot"),
            "the caption was written through at -{back}: {row:?}"
        );
        let marker = row.find(['▌', '▐']).unwrap();
        let caption = row.find("shown,").expect("caption missing");
        assert_ne!(marker, caption, "the marker landed inside the caption");
    }
}

/// The distinct foreground colours used by the graph rows of the timeline,
/// excluding chrome (borders, gutter, threshold rules).
fn graph_colours(app: &App, w: u16, h: u16) -> std::collections::HashSet<String> {
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    term.draw(|f| ui::draw_timeline_for_test(f, f.area(), app))
        .unwrap();
    let buf = term.backend().buffer();
    let graph_rows = (h as usize - 1).saturating_sub(2).max(1);
    let mut out = std::collections::HashSet::new();
    for row in 0..graph_rows {
        for x in 4..w {
            let c = &buf[(x, 1 + row as u16)];
            if c.fg != app.theme.chrome && c.symbol() != " " {
                out.insert(format!("{:?}", c.fg));
            }
        }
    }
    out
}

#[test]
fn timeline_colour_carries_identity_not_magnitude() {
    // Bar height already encodes the value. Colouring by the same number is
    // double-encoding: it spends the one free channel on information the chart
    // is already showing. An idle machine and a dying one must therefore draw
    // in the same hues, differing only in bar height.
    let build = |cpu: f32| {
        let mut app = App::new(600);
        for i in (0..60).rev() {
            let mut s = sample_at(cpu, i);
            s.mem.used = ((cpu / 100.0 * 16.0) as u64) << 30;
            app.push(s);
        }
        app.theme = Theme::new(Palette::Safe, Tier::TrueColor);
        app
    };
    let idle = graph_colours(&build(5.0), 100, 14);
    let busy = graph_colours(&build(95.0), 100, 14);
    assert_eq!(
        idle, busy,
        "timeline recolours with magnitude; colour should mean which series"
    );
    assert!(
        !idle.is_empty(),
        "no graph colours found — test proves nothing"
    );
}

#[test]
fn the_two_series_are_told_apart_by_colour() {
    let mut app = App::new(600);
    for i in (0..60).rev() {
        app.push(sample_at(50.0, i));
    }
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);
    let seen = graph_colours(&app, 100, 14);
    assert!(
        seen.len() >= 2,
        "cpu and mem render in the same colour: {seen:?}"
    );
    assert!(seen.contains(&format!("{:?}", app.theme.series_cpu)));
    assert!(seen.contains(&format!("{:?}", app.theme.series_mem)));
}

#[test]
fn status_colour_is_kept_where_it_answers_is_this_bad() {
    // The header figures and the core meters are where a reader asks "is this
    // bad right now", not "what shape was this" — so heat earns its place
    // there and only there.
    let styles_at = |cpu: f32| {
        let mut app = App::new(60);
        app.push(sample(cpu));
        app.theme = Theme::new(Palette::Safe, Tier::TrueColor);
        let mut term = Terminal::new(TestBackend::new(100, 30)).unwrap();
        term.draw(|f| ui::draw(f, &app)).unwrap();
        let buf = term.backend().buffer();
        // The header is rows 0-2: title, figures, then the core meters that
        // L2 folded in from their own section.
        let row = |y: u16| {
            (0..100u16)
                .map(|x| format!("{:?}", buf[(x, y)].fg))
                .collect::<String>()
        };
        // Figures then cores. Taken from `HEADER_H` rather than written down:
        // the header lost a row and every hardcoded index moved with it.
        (row(0), row(ui::HEADER_H - 1))
    };
    let (h_idle, c_idle) = styles_at(5.0);
    let (h_busy, c_busy) = styles_at(95.0);
    assert_ne!(h_idle, h_busy, "header figures lost their status colour");
    assert_ne!(c_idle, c_busy, "core meters lost their status colour");
}

#[test]
fn status_and_identity_hues_stay_in_their_own_panels() {
    // The C6 rule, enforced against a rendered frame rather than the palette
    // definition — a palette-level test passes happily through a wiring bug,
    // which is exactly how G5 shipped `ok` green into the timeline.
    //
    // Both states are rendered. The timeline's cursor readout only exists while
    // scrubbed, and it prints the same figures the header colours with
    // `figure_style`, so it is the likeliest place for a status hue to leak
    // into the timeline — and a live-only render never draws it.
    for scrubbed in [false, true] {
        let mut app = App::new(600);
        for i in (0..80).rev() {
            // Sweep the range so every status band is actually reached.
            app.push(sample_at((i as f32 * 1.3) % 100.0, i));
        }
        app.theme = Theme::new(Palette::Safe, Tier::TrueColor);
        if scrubbed {
            app.history.scrub(-6);
        }

        let (w, h) = (110u16, 30u16);
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| ui::draw(f, &app)).unwrap();
        let buf = term.backend().buffer();

        let status = [app.theme.ok, app.theme.warn, app.theme.critical];
        let identity = [app.theme.series_cpu, app.theme.series_mem];
        let timeline = ui::timeline_rows_range(h);

        let mut identity_marks = 0;
        let mut status_hues = std::collections::HashSet::new();
        for y in 0..h {
            for x in 0..w {
                let c = &buf[(x, y)];
                let blank = c.symbol() == " " || c.symbol() == "\u{2800}";
                if timeline.contains(&y) {
                    assert!(
                        !status.contains(&c.fg),
                        "scrubbed={scrubbed}: status hue {:?} in the timeline at ({x},{y})",
                        c.fg
                    );
                    // Count only drawn bars. Every cell in a graph row carries
                    // the series fg, blank ones included, so counting cells
                    // would still pass if the timeline drew nothing at all.
                    if identity.contains(&c.fg) && !blank {
                        identity_marks += 1;
                    }
                } else {
                    assert!(
                        !identity.contains(&c.fg),
                        "scrubbed={scrubbed}: identity hue {:?} outside the timeline at ({x},{y})",
                        c.fg
                    );
                    // Only count outside the header's LIVE badge: `live` is the
                    // `ok` hue by design, so it alone would satisfy a naive
                    // "some status colour appeared" check even with every
                    // figure, meter and table cell stripped of status colour.
                    if status.contains(&c.fg) && !blank && y >= ui::HEADER_H {
                        status_hues.insert(format!("{:?}", c.fg));
                    }
                }
            }
        }
        assert!(
            identity_marks > 20,
            "scrubbed={scrubbed}: timeline drew no data ({identity_marks} marks)"
        );
        assert!(
            !status_hues.is_empty(),
            "scrubbed={scrubbed}: no status colour outside the header at all"
        );
    }
}

#[test]
fn the_heat_ramp_states_its_scale() {
    // The 50/80 thresholds drove every colour decision in the UI and were
    // written down nowhere in it.
    let mut app = App::new(60);
    app.push(sample(50.0));
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);

    let render = |w: u16| {
        let mut term = Terminal::new(TestBackend::new(w, 30)).unwrap();
        term.draw(|f| ui::draw(f, &app)).unwrap();
        let buf = term.backend().buffer();
        (0..30u16)
            .map(|y| (0..w).map(|x| buf[(x, y)].symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    };

    // Wide enough that the figures leave room for it. The scale shares a row
    // with them now — it used to have one of its own, which is a whole row of a
    // thirty-row terminal spent on a reference that never changes.
    let wide = render(160);
    assert!(wide.contains("warn 50"), "heat ramp has no scale:\n{wide}");
    assert!(wide.contains("crit 80"));

    // A reference yields before the data it refers to.
    assert!(
        !render(30).contains("warn 50"),
        "scale did not yield when narrow"
    );
}

#[test]
fn the_scale_survives_a_host_with_no_per_core_data() {
    // It used to live on the cores panel, which is not drawn at all when the
    // platform reports no per-core figures — leaving the ramp that still
    // colours the header and the process table with no stated thresholds.
    let mut app = App::new(60);
    let mut s = sample(50.0);
    s.cpu_per_core.clear();
    app.push(s);
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);

    let mut term = Terminal::new(TestBackend::new(160, 30)).unwrap();
    term.draw(|f| ui::draw(f, &app)).unwrap();
    let buf = term.backend().buffer();
    let text: String = (0..30u16)
        .flat_map(|y| (0..160u16).map(move |x| (x, y)))
        .map(|(x, y)| buf[(x, y)].symbol())
        .collect();
    assert!(
        !text.contains("cores ("),
        "fixture should have no cores panel"
    );
    assert!(
        text.contains("warn 50"),
        "scale vanished with the cores panel"
    );
}

#[test]
fn the_stated_scale_matches_the_colouring_it_describes() {
    // The previous version built its expectation from the same constants and
    // format literal as the code under test, so it pinned the title's shape
    // rather than its agreement with anything. This ties the printed numbers
    // to where `heat` actually changes colour.
    //
    // Run at the defaults *and* at configured thresholds. Now that the numbers
    // can move, a legend that agrees with the colouring only at 50/80 is a
    // legend that agrees by coincidence — this is the test that stops the two
    // drifting apart, so it has to see them move.
    for (warn, critical) in [
        (Theme::DEFAULT_WARN_PCT, Theme::DEFAULT_CRITICAL_PCT),
        (30.0, 65.0),
        (0.1, 100.0),
        // The case that used to slip through: with three integer thresholds
        // the legend could round and the test would never notice. `warn 62.5`
        // printed as `warn 62` claimed the colour changes half a point from
        // where it does, and `warn = 49.6` printed as `50` — indistinguishable
        // from the default the user was trying to move off.
        (62.5, 87.5),
        (49.6, 80.0),
    ] {
        let th = Theme::new(Palette::Safe, Tier::TrueColor).with_thresholds(warn, critical);
        assert_ne!(
            th.heat(th.warn_pct - 0.1),
            th.heat(th.warn_pct),
            "at {warn}/{critical} the printed warn threshold is not where the colour changes"
        );
        assert_ne!(
            th.heat(th.critical_pct - 0.1),
            th.heat(th.critical_pct),
            "at {warn}/{critical} the printed critical threshold is not where the colour changes"
        );

        let mut app = App::new(60);
        app.push(sample(50.0));
        app.theme = th;
        // Wide enough for the legend, which now shares the figures' row and
        // appears only when every figure fits beside it.
        let mut term = Terminal::new(TestBackend::new(170, 30)).unwrap();
        term.draw(|f| ui::draw(f, &app)).unwrap();
        let buf = term.backend().buffer();
        let row: String = (0..170u16).map(|x| buf[(x, 0)].symbol()).collect();
        assert!(
            row.contains(&format!("warn {warn}")),
            "the header does not print warn {warn}: {row:?}"
        );
        assert!(
            row.contains(&format!("crit {critical}")),
            "the header does not print critical {critical}: {row:?}"
        );
    }
}

#[test]
fn the_timeline_rules_move_with_the_thresholds() {
    // The third reader of the pair. A rule drawn at a compiled-in 50 while the
    // header says 30 would be the graph and its own legend disagreeing about
    // where the boundary is.
    //
    // Compared over the timeline rows alone: comparing whole frames would pass
    // on the header legend changing, which is a different reader and proves
    // nothing about the rules. A 2% signal is used so the graph has headroom
    // — the rule yields wherever data is present, so a full graph shows none
    // whatever the thresholds are.
    let mut app = App::new(600);
    for i in (0..120).rev() {
        app.push(sample_at(2.0, i as u64));
    }
    let graph = |app: &App| {
        let rows = ui::timeline_rows_range(40);
        render_lines(app, 100, 40)[rows.start as usize..rows.end as usize].join("\n")
    };
    // The glyphs the rule is actually drawn with, asked of the same glyph set
    // that draws it rather than transcribed.
    let rule_glyphs: Vec<char> = (1..=4).map(|k| app.glyphs.rule_glyph(k)).collect();
    let rules_in = |s: &str| s.chars().filter(|c| rule_glyphs.contains(c)).count();

    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);
    let at_default = graph(&app);
    app.theme = app.theme.with_thresholds(3.0, 6.0);
    let at_low = graph(&app);

    // At 50/80 against an auto-scaled 10% ceiling both rules are off the scale
    // and correctly suppressed; at 3/6 both fall inside it.
    assert_eq!(
        rules_in(&at_default),
        0,
        "a rule was drawn above the top of the axis"
    );
    assert!(
        rules_in(&at_low) > 0,
        "lowering the thresholds onto the visible scale drew no rule at all"
    );
}

#[test]
fn the_graph_does_not_slide_on_a_single_keypress_while_scrolled_back() {
    // The bug this guards: deriving the window directly from the cursor drags
    // it one sample sideways on every keypress, so the graph slides under the
    // reader — and at zoom > 1 the buckets re-form and bar heights change too.
    // Paging keeps the window still until the cursor crosses a page boundary.
    let mut app = App::new(600);
    for i in (0..500).rev() {
        app.push(sample_at((i as f32 * 1.7) % 100.0, i as u64));
    }
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);
    app.history.goto_oldest();

    let graph = |a: &App| timeline_rows(a, 100, 12)[1..5].join("\n");
    let before = graph(&app);
    app.history.scrub(1);
    assert_eq!(
        graph(&app),
        before,
        "graph slid sideways on a keypress that should only move the marker"
    );
}

#[test]
fn the_cursor_is_not_glued_to_the_left_edge_while_scrolled_back() {
    // Pinning the cursor to column 0 means never seeing anything older than
    // where you are — the very behaviour G7 exists to remove.
    let mut app = App::new(600);
    for i in (0..500).rev() {
        app.push(sample_at((i as f32 * 1.7) % 100.0, i as u64));
    }
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);

    // Somewhere deep in the buffer, but not at its very start.
    app.history.goto_oldest();
    app.history.scrub(60);

    let x = cursor_column(&app, 100, 12).expect("cursor should be drawn");
    assert!(
        x > 6,
        "cursor is pinned near the left edge at column {x}; history older than \
         the cursor is unreachable"
    );
}

#[test]
fn every_scrub_position_keeps_the_cursor_inside_the_window() {
    // Paging must contain the cursor at every position and zoom, or the
    // off-window fallback becomes reachable again.
    for zoom_steps in 0..crate::app::ZOOM_LEVELS.len() {
        let mut app = App::new(600);
        for i in (0..500).rev() {
            app.push(sample_at(50.0, i as u64));
        }
        app.theme = Theme::new(Palette::Safe, Tier::TrueColor);
        for _ in 0..zoom_steps {
            app.zoom_out();
        }
        app.history.goto_oldest();
        for step in 0..40 {
            assert!(
                cursor_column(&app, 100, 12).is_some(),
                "zoom step {zoom_steps}, scrub {step}: cursor left the window"
            );
            app.history.scrub(11);
        }
    }
}

#[test]
fn a_many_core_machine_summarises_rather_than_clipping() {
    // Cores that do not fit are counted, not dropped, so the number on screen
    // is never quietly wrong.
    //
    // The assertion is on the marker's *content*, not the row's length: every
    // TestBackend row is exactly `w` cells whatever was clipped, so a length
    // check is a tautology and passed the very bug this now catches — at 13
    // columns a 16-core machine rendered `16 cores  +1`, the marker sized from
    // the core count and then clipped by ratatui.
    for cores in [16usize, 128, 1024] {
        let mut app = App::new(60);
        let mut s = sample(50.0);
        s.cpu_per_core = (0..cores).map(|i| (i as f32 * 0.78) % 100.0).collect();
        app.push(s);
        app.theme = Theme::new(Palette::Safe, Tier::TrueColor);

        for w in [13u16, 16, 24, 80, 100, 200] {
            let mut term = Terminal::new(TestBackend::new(w, 30)).unwrap();
            term.draw(|f| ui::draw(f, &app)).unwrap();
            let buf = term.backend().buffer();
            let row: String = (0..w)
                .map(|x| buf[(x, ui::HEADER_H - 1)].symbol())
                .collect();
            let drawn = row.chars().filter(|c| BAR_GLYPHS.contains(c)).count();
            // Whatever it degrades to, the count itself is always stated.
            assert!(
                row.contains(&cores.to_string()),
                "cores={cores} w={w}: core count missing from {row:?}"
            );

            match row.split_once('+') {
                Some((_, tail)) => {
                    let hidden: usize = tail.trim().parse().unwrap_or_else(|_| {
                        panic!("cores={cores} w={w}: unreadable marker {row:?}")
                    });
                    assert_eq!(
                        drawn + hidden,
                        cores,
                        "cores={cores} w={w}: {drawn} drawn + {hidden} hidden != {cores}, in {row:?}"
                    );
                }
                // No marker is honest in exactly two cases: everything is
                // drawn, or nothing is and the line states the count alone.
                // A partial draw with no marker is the silent lie.
                None => assert!(
                    drawn == cores || drawn == 0,
                    "cores={cores} w={w}: {drawn} of {cores} drawn with no marker, in {row:?}"
                ),
            }
        }
    }
}

/// The eighth-block glyphs the meters draw with.
const BAR_GLYPHS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

#[test]
fn a_host_with_no_per_core_data_says_so() {
    // The line is part of the header now, so it is always drawn — silence
    // would read as "zero cores" rather than "not reported".
    let mut app = App::new(60);
    let mut s = sample(50.0);
    s.cpu_per_core.clear();
    app.push(s);
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);
    let mut term = Terminal::new(TestBackend::new(100, 30)).unwrap();
    term.draw(|f| ui::draw(f, &app)).unwrap();
    let buf = term.backend().buffer();
    let row: String = (0..100u16)
        .map(|x| buf[(x, ui::HEADER_H - 1)].symbol())
        .collect();
    assert!(
        row.contains("not reported"),
        "silent about missing cores: {row:?}"
    );
}

#[test]
#[ignore = "visual"]
fn show_core_overflow() {
    for cores in [16usize, 128, 1024] {
        for w in [13u16, 16, 24, 40, 80] {
            let mut app = App::new(60);
            let mut s = sample(50.0);
            s.cpu_per_core = (0..cores).map(|i| (i as f32 * 0.78) % 100.0).collect();
            app.push(s);
            app.theme = Theme::new(Palette::Safe, Tier::TrueColor);
            let mut term = Terminal::new(TestBackend::new(w, 30)).unwrap();
            term.draw(|f| ui::draw(f, &app)).unwrap();
            let buf = term.backend().buffer();
            let row: String = (0..w)
                .map(|x| buf[(x, ui::HEADER_H - 1)].symbol())
                .collect();
            println!("  {cores:>4} cores, w={w:<4} |{}|", row);
        }
    }
}

#[test]
fn growing_the_timeline_never_shrinks_it() {
    // The bug this guards: a purely proportional height gave an 80x24 terminal
    // five rows where nine were fixed before — a quarter of the CPU resolution,
    // on the commonest terminal size, from a change justified by *more*
    // resolution. Wherever the old fixed height fits, it is the floor.
    //
    // "Wherever it fits" now means *after* the table's own floor is met. Those
    // two floors compete below seventeen rows, and the table wins: a nine-row
    // graph on a fourteen-row terminal was bought with a table showing no
    // processes at all, which is not a trade between resolutions.
    let smallest_that_fits = ui::HEADER_H + 1 + ui::PROCS_FLOOR_H + ui::TIMELINE_MIN_H;
    for total in smallest_that_fits..=200u16 {
        assert!(
            ui::timeline_height(total) >= ui::TIMELINE_MIN_H,
            "total={total}: {} rows, below the {} it had when fixed",
            ui::timeline_height(total),
            ui::TIMELINE_MIN_H
        );
    }
}

#[test]
fn the_timeline_grows_above_the_floor_and_stops() {
    let h = |t| ui::timeline_height(t);
    assert_eq!(h(24), ui::TIMELINE_MIN_H, "should still be at the floor");
    assert!(h(40) > h(24), "did not grow when there was room");
    for total in [80u16, 200, 500] {
        assert_eq!(h(total), ui::TIMELINE_MAX_H, "total={total}: unbounded");
    }
}

#[test]
fn a_column_of_figures_shares_a_right_edge() {
    // Scanning a column for the largest value is the commonest thing anyone
    // does here, and right alignment is what makes magnitude visual instead of
    // something to parse. Left-aligned, `103.4`, `21.3` and `6.1` share no
    // decimal point and `6.1G`, `59.9M` and `5.1M` share no unit position.
    let mut app = App::new(60);
    let mut s = sample(10.0);
    s.procs = vec![
        ProcSample {
            cpu: 103.4,
            rss: 6_500_000_000,
            threads: Some(33),
            ..proc_named(81977, "aaa", 0.0, 0)
        },
        ProcSample {
            cpu: 21.3,
            rss: 62_800_000,
            threads: Some(4),
            ..proc_named(5531, "bbb", 0.0, 0)
        },
        ProcSample {
            cpu: 6.1,
            rss: 5_400_000,
            threads: Some(139),
            ..proc_named(1, "ccc", 0.0, 0)
        },
    ];
    app.push(s);
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);
    let drawn = rows(&app, 100, 20);
    let head = drawn
        .iter()
        .find(|l| l.contains("CPU%"))
        .expect("no header row")
        .clone();
    let body: Vec<String> = drawn
        .into_iter()
        .filter(|l| l.contains("aaa") || l.contains("bbb") || l.contains("ccc"))
        .collect();
    assert_eq!(body.len(), 3, "expected three rows: {body:?}");

    // Positions counted in chars, not bytes: the bars beside these figures are
    // three bytes to the column, so a byte offset says the wide row is further
    // right than it is.
    let at = |l: &str, p: &dyn Fn(char) -> bool, last: bool| -> usize {
        let hits = l.chars().enumerate().filter(|(_, c)| p(*c));
        if last {
            hits.last()
        } else {
            hits.into_iter().next()
        }
        .unwrap_or_else(|| panic!("no such column in {l:?}"))
        .0
    };
    let shared = |name: &str, cols: Vec<usize>| {
        assert!(
            cols.windows(2).all(|w| w[0] == w[1]),
            "the {name} do not share a right edge: {cols:?} in {body:?}"
        );
    };

    // One decimal place throughout, so a shared decimal point is a shared right
    // edge. Every magnitude here differs in digit count, which is the case that
    // exposes it.
    shared(
        "cpu figures",
        body.iter().map(|l| at(l, &|c| c == '.', false)).collect(),
    );
    // Bytes carry their unit as the last character, so a shared right edge puts
    // the `G` under the `M`.
    shared(
        "byte figures",
        body.iter()
            .map(|l| at(l, &|c| c == 'G' || c == 'M', true))
            .collect(),
    );
    // And the pids, which have no punctuation to give it away: the last digit
    // before the first space that follows them.
    shared(
        "pids",
        body.iter()
            .map(|l| {
                at(l, &|c: char| c.is_ascii_digit(), false) + l.trim_start().find(' ').unwrap() - 1
            })
            .collect(),
    );

    // A header aligned the other way from its column is worse than none: it
    // reads as the edge the eye then scans against. `CPU%` ends at its `%`,
    // and the figures below it end one char past the decimal point.
    let head_pct = at(&head, &|c| c == '%', false);
    let last_digit = at(&body[0], &|c| c == '.', false) + 1;
    assert_eq!(
        head_pct, last_digit,
        "the CPU% header does not share its column's right edge: {head:?} over {:?}",
        body[0]
    );
}

#[test]
fn a_short_terminal_draws_processes_not_just_a_header() {
    // The frame, not the arithmetic. `PROCS_FLOOR_H` was two panel rows and a
    // table spends two on chrome, so the floor was honoured and no process was
    // ever drawn.
    let mut app = App::new(600);
    for i in (0..20).rev() {
        let mut s = sample_at(50.0, i);
        s.procs = (0..30)
            .map(|n| ProcSample {
                cpu: 30.0 - n as f32,
                ..proc_named(n + 1, &format!("worker{n}"), 0.0, 1 << 20)
            })
            .collect();
        app.push(s);
    }
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);

    for filter in ["", "worker1"] {
        app.filter = filter.into();
        for h in 12..=24u16 {
            let drawn = rows(&app, 120, h)
                .iter()
                .filter(|l| l.contains("worker"))
                .count();
            // Both claims: the floor is honoured, *and* the floor is not zero.
            // A table that draws no process is not a process table however
            // faithfully it obeys its own constant.
            assert!(
                drawn >= ui::PROCS_FLOOR_ROWS as usize && drawn > 0,
                "at {h} rows with filter {filter:?} the table drew {drawn} processes"
            );
        }
    }
}

#[test]
fn a_short_terminal_shows_processes_rather_than_a_taller_graph() {
    // At 120x14 a filter matching eleven processes drew none of them: the
    // table's floor was two *panel* rows, and a table spends two on chrome
    // before any data.
    for total in 12..=17u16 {
        let table = total - ui::HEADER_H - 1 - ui::timeline_height(total);
        assert!(
            table >= ui::PROCS_FLOOR_H,
            "total={total}: the table got {table} rows, below its floor of {}",
            ui::PROCS_FLOOR_H
        );
        assert!(
            table - ui::PROCS_CHROME_H >= ui::PROCS_FLOOR_ROWS,
            "total={total}: {} process rows",
            table - ui::PROCS_CHROME_H
        );
    }
}

#[test]
fn the_process_table_always_keeps_some_rows() {
    // Including on terminals too small for the timeline's own floor, where the
    // timeline takes what is left rather than the height it would prefer.
    for total in 6..=80u16 {
        let left = total.saturating_sub(ui::HEADER_H + ui::timeline_height(total) + 1);
        assert!(left >= 1, "total={total}: process table got {left} rows");
    }
}

#[test]
fn a_taller_window_draws_more_graph_rows() {
    // The previous version of this test joined the *whole screen* at two
    // heights and asserted the strings differed — which a 24-line and a 50-line
    // string always do, so it held even for a constant height. Count the
    // timeline's own non-blank graph rows instead.
    let mut app = App::new(600);
    for i in (0..200).rev() {
        app.push(sample_at((i as f32 * 1.7) % 100.0, i as u64));
    }
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);

    let graph_rows = |total: u16| {
        let mut term = Terminal::new(TestBackend::new(100, total)).unwrap();
        term.draw(|f| ui::draw(f, &app)).unwrap();
        let buf = term.backend().buffer();
        let range = ui::timeline_rows_range(total);
        range
            .filter(|&y| {
                (0..100u16).any(|x| {
                    let s = buf[(x, y)].symbol();
                    s != " " && s != "\u{2800}" && s != "─"
                })
            })
            .count()
    };
    let small = graph_rows(24);
    let large = graph_rows(50);
    assert!(
        large > small,
        "a 50-row window drew {large} timeline rows, a 24-row one {small}"
    );
}

#[test]
fn a_gutter_is_only_reserved_when_something_can_fill_it() {
    // Four columns of padding with no anchors and no label is four columns of
    // history thrown away. Only reachable now that the panel can be squeezed
    // below the height at which a section can carry a scale.
    let mut app = App::new(600);
    for i in (0..200).rev() {
        app.push(sample_at(50.0, i));
    }
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);

    for total in 8..=40u16 {
        let mut term = Terminal::new(TestBackend::new(100, total)).unwrap();
        term.draw(|f| ui::draw(f, &app)).unwrap();
        let buf = term.backend().buffer();
        let range = ui::timeline_rows_range(total);
        // Skip the section rule; look at the graph rows only.
        let gutter_blank = range
            .clone()
            .skip(1)
            .all(|y| (0..4u16).all(|x| buf[(x, y)].symbol() == " "));
        let graph_drawn = range.skip(1).any(|y| {
            (4..100u16).any(|x| buf[(x, y)].symbol() != " " && buf[(x, y)].symbol() != "\u{2800}")
        });
        assert!(
            !(gutter_blank && graph_drawn),
            "total={total}: four gutter columns reserved and left empty"
        );
    }
}

/// What a rendered frame contains, for walking the degradation ladder.
#[derive(Debug, PartialEq)]
struct Present {
    heat_scale: bool,
    core_meters: bool,
    axis_anchors: bool,
    series_labels: bool,
    legend: bool,
    graph: bool,
    table_rows: bool,
}

fn present_at(app: &App, w: u16, h: u16) -> Present {
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    term.draw(|f| ui::draw(f, app)).unwrap();
    let buf = term.backend().buffer();
    let row = |y: u16| -> String {
        if y >= h {
            return String::new();
        }
        (0..w).map(|x| buf[(x, y)].symbol()).collect()
    };
    let all: String = (0..h).map(row).collect::<Vec<_>>().join("\n");
    let timeline = ui::timeline_rows_range(h);
    Present {
        heat_scale: all.contains("warn 50"),
        // The last header row, whichever that is. Written down as `2` it kept
        // pointing at the timeline the moment the header lost a row.
        core_meters: {
            let r = row(ui::HEADER_H - 1);
            r.contains('▇') || r.contains('▄') || r.contains('▁')
        },
        // Scoped to the timeline's gutter columns. Matching "CPU " anywhere
        // finds the header figures, which are always drawn — a false positive
        // that made the gutter look like it never yielded.
        // Read from the gutter and trimmed, rather than anchored to column
        // zero: the label is right-aligned inside `GUTTER_W`, so hardcoding
        // either the width or the alignment breaks the moment the constant
        // moves — which it did the moment a series was called `WAIT`.
        axis_anchors: timeline.clone().any(|y| {
            let g: String = row(y).chars().take(ui::GUTTER_W).collect();
            g.trim().chars().next().is_some_and(|c| c.is_ascii_digit())
        }),
        series_labels: timeline.clone().any(|y| {
            let g: String = row(y).chars().take(ui::GUTTER_W).collect();
            matches!(g.trim(), "CPU" | "WAIT" | "MEM")
        }),
        legend: all.contains("s/slot"),
        graph: timeline.clone().any(|y| {
            (0..w).any(|x| {
                let s = buf[(x, y.min(h - 1))].symbol();
                s.starts_with('⠀')
                    || (s
                        .chars()
                        .next()
                        .is_some_and(|c| ('\u{2800}'..='\u{28ff}').contains(&c))
                        && s != "⠀")
            })
        }),
        table_rows: all.contains("postgres") || all.contains("nginx"),
    }
}

#[test]
fn the_degradation_ladder_holds_at_every_size() {
    // Each element yields in a fixed order as the window shrinks, and each is
    // present above its threshold and absent below it. Individually every one
    // of these calls was defensible; the point of writing the order down is
    // that together they are a design rather than eight separate decisions.
    let mut app = App::new(600);
    for i in (0..200).rev() {
        app.push(sample_at(50.0, i));
    }
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);

    // Widest: everything on. Wider than it was, because the heat legend shares
    // the figures' row now — it appears only when there is room for every
    // figure beside it, which is what makes it strictly the first thing given
    // up rather than something that comes and goes as the figures shuffle.
    let full = present_at(&app, 170, 40);
    assert!(full.heat_scale && full.core_meters && full.axis_anchors);
    assert!(full.series_labels && full.legend && full.graph && full.table_rows);

    // The scale is a reference for the figures, so it goes before them.
    assert!(
        !present_at(&app, 40, 40).heat_scale,
        "scale outlived its room"
    );
    assert!(
        present_at(&app, 40, 40).core_meters,
        "meters went before the scale"
    );

    // The gutter — anchors and labels with it — goes before the graph.
    let narrow = present_at(&app, 28, 40);
    assert!(
        !narrow.axis_anchors && !narrow.series_labels,
        "gutter outlived its room"
    );
    assert!(narrow.graph, "graph went before its own axis");

    // The graph outlives the process table's rows, because the graph is the
    // thing this tool is for.
    let short = present_at(&app, 100, 12);
    assert!(short.graph, "graph went before the table");

    // And nothing panics anywhere on the way down.
    for w in (10..=120).step_by(7) {
        for h in (4..=40).step_by(3) {
            let _ = present_at(&app, w, h);
        }
    }
}

#[test]
fn every_element_yields_monotonically() {
    // An element that reappears as the window shrinks is a bug in the ladder,
    // not a feature. Walk the width down and require each flag to fall at most
    // once.
    let mut app = App::new(600);
    for i in (0..200).rev() {
        app.push(sample_at(50.0, i));
    }
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);

    let flags = |p: &Present| [p.heat_scale, p.axis_anchors, p.series_labels];
    let names = ["heat scale", "axis anchors", "series labels"];
    let mut prev = flags(&present_at(&app, 140, 40));
    for w in (20..=140).rev().step_by(2) {
        let now = flags(&present_at(&app, w, 40));
        for i in 0..prev.len() {
            assert!(
                !now[i] || prev[i],
                "{} reappeared at width {w} after yielding",
                names[i]
            );
        }
        prev = now;
    }
}

#[test]
fn an_idle_machine_still_fills_its_graph() {
    // A fixed 0..100 axis left the largest panel on screen almost entirely
    // blank on a machine doing ordinary work. The axis scales to the peak, and
    // says so.
    let build = |cpu: f32| {
        let mut app = App::new(600);
        for i in (0..200).rev() {
            let mut s = sample_at(cpu, i);
            s.mem.used = ((s.mem.total as f32) * 0.1) as u64;
            app.push(s);
        }
        app.theme = Theme::new(Palette::Safe, Tier::TrueColor);
        app
    };
    // Rows of the CPU graph carrying a drawn bar.
    let filled = |app: &App, h: u16| {
        let mut term = Terminal::new(TestBackend::new(94, h)).unwrap();
        term.draw(|f| ui::draw(f, app)).unwrap();
        let buf = term.backend().buffer();
        ui::timeline_rows_range(h)
            .skip(1)
            .filter(|&y| {
                (4..94u16).any(|x| {
                    let s = buf[(x, y)].symbol();
                    s != " " && s != "\u{2800}"
                })
            })
            .count()
    };
    // An idle machine and a saturated one should light a comparable number of
    // rows, because each is drawn against its own ceiling.
    let idle = filled(&build(9.0), 16);
    let busy = filled(&build(95.0), 16);
    assert!(
        idle * 2 >= busy,
        "idle machine lit {idle} rows against a busy machine's {busy}"
    );
}

#[test]
fn the_axis_states_the_ceiling_it_scaled_to() {
    // Scaling without saying so would be the misleading kind of clever.
    let gutter_top = |cpu: f32| {
        let mut app = App::new(600);
        for i in (0..200).rev() {
            app.push(sample_at(cpu, i));
        }
        app.theme = Theme::new(Palette::Safe, Tier::TrueColor);
        gutter_text(&app, 100, 14)
            .lines()
            .next()
            .unwrap()
            .trim()
            .to_string()
    };
    assert_eq!(gutter_top(9.0), "10");
    assert_eq!(gutter_top(22.0), "25");
    assert_eq!(gutter_top(44.0), "50");
    assert_eq!(gutter_top(95.0), "100");
}

#[test]
fn the_rules_do_not_mark_a_buffer_that_has_no_data_yet() {
    // Dashing a reference line across the part of the window that has never
    // been sampled is noise about a region with nothing to reference.
    let mut app = App::new(600);
    for i in (0..20).rev() {
        app.push(sample_at(95.0, i)); // few samples, wide panel
    }
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);

    let mut term = Terminal::new(TestBackend::new(100, 14)).unwrap();
    term.draw(|f| ui::draw(f, &app)).unwrap();
    let buf = term.backend().buffer();
    // Graph rows only — the range also covers the cursor and legend rows.
    let range = ui::timeline_rows_range(14);
    let graph_rows = (range.len() - 1).saturating_sub(2).max(1);
    // The left third of the graph holds no samples at all. Starting past the
    // gutter, whose width is derived from the series names rather than fixed —
    // an axis figure sitting in column four is a label, not a sample.
    for y in range.start + 1..range.start + 1 + graph_rows as u16 {
        for x in ui::GUTTER_W as u16..25u16 {
            let s = buf[(x, y)].symbol();
            assert!(
                s == " " || s == "\u{2800}",
                "glyph {s:?} drawn at ({x},{y}) where no sample exists"
            );
        }
    }
}

#[test]
fn core_meters_are_countable_in_groups() {
    let mut app = App::new(60);
    let mut s = sample(50.0);
    s.cpu_per_core = (0..14).map(|i| (i as f32 * 6.0) % 100.0).collect();
    app.push(s);
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);
    let mut term = Terminal::new(TestBackend::new(100, 30)).unwrap();
    term.draw(|f| ui::draw(f, &app)).unwrap();
    let buf = term.backend().buffer();
    let row: String = (0..100u16)
        .map(|x| buf[(x, ui::HEADER_H - 1)].symbol())
        .collect();
    let meters = row.trim_end().split_once("cores ").unwrap().1;
    // Fourteen cores in groups of four: three gaps.
    assert_eq!(
        meters.matches(' ').count(),
        3,
        "cores not grouped: {meters:?}"
    );
}

#[test]
fn the_cpu_bar_marks_a_process_using_more_than_one_core() {
    // Clipping 400% to a full bar would make it indistinguishable from a
    // process using exactly 100%.
    let mut app = App::new(60);
    let mut s = sample(50.0);
    s.procs = vec![
        proc_named(101, "single", 100.0, 1 << 20),
        proc_named(102, "threaded", 400.0, 1 << 20),
    ];
    app.push(s);
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);

    let mut term = Terminal::new(TestBackend::new(120, 30)).unwrap();
    term.draw(|f| ui::draw(f, &app)).unwrap();
    let buf = term.backend().buffer();
    let rows: Vec<String> = (0..30u16)
        .map(|y| (0..120u16).map(|x| buf[(x, y)].symbol()).collect())
        .collect();
    let threaded = rows.iter().find(|r| r.contains("threaded")).unwrap();
    let single = rows.iter().find(|r| r.contains("single")).unwrap();
    assert!(
        threaded.contains('+'),
        "over-one-core not marked: {threaded:?}"
    );
    assert!(
        !single.contains('+'),
        "exactly one core wrongly marked: {single:?}"
    );
}

#[test]
fn the_memory_bar_is_scaled_to_the_displayed_sample() {
    // Everything else in the table follows the cursor; a bar scaled against
    // the live total would contradict the row it sits in.
    let mut app = App::new(60);
    // Oldest: a small machine, so 8G is most of it. Newest: a large one.
    let mut old = sample(10.0);
    old.mem.total = 16 << 30;
    old.procs = vec![proc_named(1, "hog", 1.0, 8 << 30)];
    let mut new = sample(10.0);
    new.mem.total = 256 << 30;
    new.procs = vec![proc_named(1, "hog", 1.0, 8 << 30)];
    app.push(old);
    app.push(new);
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);

    let ink = |app: &App| {
        let mut term = Terminal::new(TestBackend::new(120, 30)).unwrap();
        term.draw(|f| ui::draw(f, app)).unwrap();
        let buf = term.backend().buffer();
        (0..30u16)
            .map(|y| {
                (0..120u16)
                    .map(|x| buf[(x, y)].symbol())
                    .collect::<String>()
            })
            .find(|r| r.contains("hog"))
            .unwrap()
            .chars()
            .filter(|c| "▏▎▍▌▋▊▉█".contains(*c))
            .count()
    };
    let on_big_machine = ink(&app);
    app.history.scrub(-1);
    let on_small_machine = ink(&app);
    assert!(
        on_small_machine > on_big_machine,
        "8G of 16G drew {on_small_machine} cells, 8G of 256G drew {on_big_machine}"
    );
}

/// A history where each process has a distinct, recognisable CPU shape.
fn app_with_shapes() -> App {
    let mut app = App::new(600);
    for i in (0..120).rev() {
        let x = (120 - i) as f32;
        let mut s = sample_at(30.0, i as u64);
        s.procs = vec![
            ProcSample {
                cpu: 45.0 + 35.0 * (x * 0.3).sin(),
                ..proc_named(824, "wave", 0.0, 512 << 20)
            },
            ProcSample {
                cpu: 2.0,
                ..proc_named(2077, "flat", 0.0, 148 << 20)
            },
            ProcSample {
                cpu: x.min(80.0),
                ..proc_named(3001, "ramping", 0.0, 64 << 20)
            },
        ];
        app.push(s);
    }
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);
    app
}

/// The sparkline drawn for a named process.
fn spark_for(app: &App, name: &str) -> String {
    let mut term = Terminal::new(TestBackend::new(110, 24)).unwrap();
    term.draw(|f| ui::draw(f, app)).unwrap();
    let buf = term.backend().buffer();
    let row = (0..24u16)
        .map(|y| {
            (0..110u16)
                .map(|x| buf[(x, y)].symbol())
                .collect::<String>()
        })
        .find(|r| r.contains(name))
        .unwrap_or_else(|| panic!("{name} not on screen"));
    row.chars()
        .filter(|c| ('\u{2800}'..='\u{28ff}').contains(c))
        .collect()
}

#[test]
fn each_process_gets_its_own_history() {
    // The differentiator: every retained sample holds its whole process list,
    // so "what has *this* process been doing" is already in the buffer. No
    // other monitor keeps per-process history to answer it from.
    let app = app_with_shapes();
    let wave = spark_for(&app, "wave");
    let flat = spark_for(&app, "flat");
    let ramping = spark_for(&app, "ramping");

    assert_eq!(wave.chars().count(), 10);
    assert_ne!(
        wave, flat,
        "two different histories drew the same sparkline"
    );
    assert_ne!(wave, ramping);
    assert_ne!(flat, ramping);
}

#[test]
fn sparklines_share_one_scale_so_rows_can_be_compared() {
    // Scaled per row, a flat 2% process looks exactly like one spiking to 80%,
    // which defeats the only reason to put them in a column together.
    let app = app_with_shapes();
    let ink = |name: &str| {
        spark_for(&app, name)
            .chars()
            .map(|c| (c as u32 - 0x2800).count_ones())
            .sum::<u32>()
    };
    assert!(
        ink("flat") < ink("wave"),
        "a 2% process drew as much ink as one averaging 45%"
    );
}

#[test]
fn a_reused_pid_does_not_splice_two_processes_into_one_line() {
    // Matched on pid alone, a recycled pid would draw a graph of two different
    // programs — the same trap the name cache had, with a worse result.
    let mut app = App::new(600);
    for i in (0..60).rev() {
        let mut s = sample_at(10.0, i as u64);
        // Same pid throughout, but a different process for the first half.
        let (started, cpu) = if i > 30 {
            (Some(111), 90.0)
        } else {
            (Some(222), 2.0)
        };
        s.procs = vec![ProcSample {
            cpu,
            started,
            ..proc_named(4242, "recycled", 0.0, 1 << 20)
        }];
        app.push(s);
    }
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);

    // The live process started at 222 and has only ever been at 2%. Its
    // sparkline must not show the 90% the previous occupant of that pid had.
    let spark = spark_for(&app, "recycled");
    let ink: u32 = spark
        .chars()
        .map(|c| (c as u32 - 0x2800).count_ones())
        .sum();
    let full: u32 = spark.chars().count() as u32 * 8;
    assert!(
        ink * 3 < full,
        "sparkline shows the previous process's history: {spark:?}"
    );
}

#[test]
fn a_process_using_several_cores_still_has_a_readable_sparkline() {
    // A virtual machine on three cores is 300%, and the axis ladder used to
    // stop at 100 — so everything a busy process did above one core was drawn
    // at the same height. Not a clipped graph: no graph.
    //
    // A ramp rather than a sawtooth. A repeating waveform can alias against the
    // two-samples-per-cell packing and come out as one glyph repeated whatever
    // the axis does, which is a property of the test data, not the code — an
    // earlier version of this failed for exactly that reason.
    let spark = |peak: f32| {
        let mut app = App::new(60);
        for i in (0..40).rev() {
            let mut s = sample_at(10.0, i as u64);
            s.procs = vec![ProcSample {
                cpu: peak * (40 - i) as f32 / 40.0,
                ..proc_named(1, "vm", 0.0, 1 << 20)
            }];
            app.push(s);
        }
        app.theme = Theme::new(Palette::Safe, Tier::TrueColor);
        spark_for(&app, "vm")
    };

    // A ramp against an axis that fits it tops out only at the very end. Under
    // the old ceiling of 100 a ramp to 800% is above the axis for seven eighths
    // of its length and draws solid for all of it.
    for peak in [285.0f32, 800.0, 1400.0] {
        let s = spark(peak);
        let full = s.chars().filter(|&c| c == '⣿').count();
        assert!(
            full <= 3,
            "a ramp to {peak}% saturated {full} of {} cells: {s:?}",
            s.chars().count()
        );
        assert!(
            s.chars().collect::<std::collections::HashSet<_>>().len() > 2,
            "a ramp to {peak}% drew almost no variation: {s:?}"
        );
    }
}

#[test]
fn scrolling_the_list_does_not_rescale_everybody_else_history() {
    // The axis is shared by every row, so it must not be taken from the rows
    // that happen to be on screen. Drawn from the visible slice, scrolling a
    // multi-core process into view collapses every other row's history to the
    // floor and springs it back when that process scrolls off — the sparkline
    // answering a different question depending on where the list is sitting.
    let mut app = App::new(60);
    for i in (0..30).rev() {
        let mut s = sample_at(10.0, i as u64);
        // One heavy process, then a long tail of quiet ones. The heavy one
        // sorts to the top, so scrolling down takes it off screen.
        s.procs = vec![ProcSample {
            cpu: 900.0,
            ..proc_named(1, "vm", 0.0, 1 << 20)
        }];
        s.procs.extend((2..40).map(|pid| ProcSample {
            cpu: 6.0,
            ..proc_named(pid, &format!("quiet{pid}"), 0.0, 1 << 20)
        }));
        app.push(s);
    }
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);

    // Tall enough that rows are actually drawn, short enough that the heavy one
    // can be scrolled past. Too short and the table gets no rows at all, which
    // makes both readings agree for the wrong reason.
    let axis_of = |app: &App| {
        render(app, 200, 30)
            .lines()
            .find(|l| l.contains("PID"))
            .and_then(|l| {
                let i = l.find('≤')? + '≤'.len_utf8();
                Some(l[i..].split('%').next()?.to_string())
            })
            .expect("no axis label")
    };

    let at_top = axis_of(&app);
    // Far enough down that the 900% row is well off screen. Selected by
    // walking, because a selection is of a process rather than an index.
    for _ in 0..38 {
        app.select_delta(1);
    }
    let scrolled = axis_of(&app);
    assert_eq!(
        at_top, scrolled,
        "the history axis moved from {at_top}% to {scrolled}% just from scrolling"
    );
}

#[test]
fn the_sparkline_column_states_its_own_scale() {
    // One ceiling is shared by every row so the shapes can be compared, which
    // means the column has a scale — and a scale that moves without saying so
    // is an unlabelled y-axis. It moves below one core too: the steps there are
    // 10 / 25 / 50 / 100, a tenfold swing that one process touching 60% is
    // enough to cause.
    //
    // In the column header, not the section title. A legend belongs with the
    // thing it explains, and the title was three metres to the left of it. Read
    // per line with `rows`, not `render` — `render` returns the whole frame as
    // one string with no newlines, so `.lines()` on it yields a single blob and
    // any two facts from anywhere in the frame appear to share a line.
    let frame = |cpu: f32| {
        let mut app = App::new(60);
        let mut s = sample(10.0);
        s.procs = vec![ProcSample {
            cpu,
            ..proc_named(1, "vm", 0.0, 1 << 20)
        }];
        app.push(s);
        app.theme = Theme::new(Palette::Safe, Tier::TrueColor);
        rows(&app, 200, 20)
    };
    let find = |ls: &[String], pat: &str| {
        ls.iter()
            .find(|l| l.contains(pat))
            .unwrap_or_else(|| panic!("no line containing {pat:?}"))
            .clone()
    };

    for (cpu, want) in [(500.0f32, "≤800%"), (40.0, "≤50%")] {
        let ls = frame(cpu);
        let header = find(&ls, "PID");
        assert!(
            header.contains(want),
            "{want} not in the header: {header:?}"
        );
        let title = find(&ls, "processes (");
        assert!(
            !title.contains('≤'),
            "the scale is still in the section title as well: {title:?}"
        );
    }
}

/// A sample carrying one device at a given utilisation and service time.
fn with_disk(util: f32, await_ms: Option<f32>) -> Sample {
    let mut s = sample(10.0);
    s.disks = Some(vec![crate::sample::DiskStat {
        name: std::sync::Arc::from("nvme0n1"),
        read: 1 << 20,
        write: 2 << 20,
        reads: 40,
        writes: 90,
        util,
        await_ms,
        queue: 4.5,
    }]);
    s
}

#[test]
fn the_header_names_the_device_the_wait_figure_is_about() {
    // `WAIT 26.7%` says the CPU is idle waiting on storage and then strands
    // you. The next question is which device and how badly.
    let mut app = App::new(60);
    app.push(with_disk(88.0, Some(12.5)));
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);
    let frame = render(&app, 200, 30);
    assert!(frame.contains("nvme0n1"), "the device was not named");
    assert!(frame.contains("88.0%"), "utilisation was not shown");
    assert!(frame.contains("12.5ms"), "service time was not shown");
}

#[test]
fn a_device_that_completed_nothing_shows_no_service_time() {
    // A mean of no operations is not zero, and zero here would read as an
    // infinitely fast disk — the most flattering possible lie about the figure
    // most worth trusting.
    let mut app = App::new(60);
    app.push(with_disk(0.0, None));
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);
    let frame = render(&app, 200, 30);
    assert!(frame.contains("nvme0n1"));
    assert!(
        !frame.contains("0.0ms"),
        "an idle device claimed a service time"
    );
}

#[test]
fn a_platform_that_reads_no_disks_draws_no_disk_row() {
    // macOS. The graph gives the row back to memory rather than carrying an
    // empty one, which is the rule `WAIT` already follows.
    let mut absent = App::new(60);
    let mut present = App::new(60);
    for _ in 0..20 {
        absent.push(sample(10.0));
        present.push(with_disk(50.0, Some(3.0)));
    }
    absent.theme = Theme::new(Palette::Safe, Tier::TrueColor);
    present.theme = Theme::new(Palette::Safe, Tier::TrueColor);

    // Scoped to the timeline block: `DISK R` and `DISK W` are also process
    // table column headers, so a bare substring search over the frame passes
    // either way.
    let gutter = |app: &App| {
        rows(app, 200, 40)
            .iter()
            .skip_while(|l| !l.contains("── timeline"))
            .take_while(|l| !l.contains("shown,"))
            .any(|l| l.contains("DISK"))
    };
    assert!(!gutter(&absent), "a disk graph was drawn with no disks");
    assert!(
        gutter(&present),
        "no disk graph was drawn when disks are known"
    );

    // And it never takes memory's row. The graph drops rows from the end, so a
    // series added before memory would have made every three-row layout worse
    // in order to add this one.
    let labelled = |app: &App, h: u16, what: &str| {
        rows(app, 200, h)
            .iter()
            .skip_while(|l| !l.contains("── timeline"))
            .take_while(|l| !l.contains("shown,"))
            .any(|l| l.contains(what))
    };
    for h in 14..44 {
        assert!(
            labelled(&present, h, "MEM") || !labelled(&present, h, "DISK"),
            "at height {h} the disk graph displaced memory"
        );
    }
}

/// A sample reporting the given IO and memory stall percentages.
fn with_pressure(io_full: f32, mem_full: f32) -> Sample {
    use crate::sample::{Pressure, Stall};
    let mut s = sample(10.0);
    s.pressure = Some(Pressure {
        cpu: Stall {
            some: 1.0,
            full: 0.0,
        },
        io: Stall {
            some: io_full * 2.0,
            full: io_full,
        },
        memory: Stall {
            some: mem_full * 2.0,
            full: mem_full,
        },
    });
    s
}

#[test]
fn every_series_name_fits_the_gutter_it_is_drawn_in() {
    // `STALL` shipped rendering as `STAL`. The gutter truncates silently — the
    // name is simply a letter shorter and nothing looks wrong — so the guard
    // has to be that every name the timeline can draw survives being drawn.
    const ROWS: usize = 4;
    for name in ui::series_names() {
        // Scanned rather than aimed at one row: the anchors and the name sit on
        // different rows and which one carries the label is the gutter's own
        // business, not something this test should encode.
        let drawn: Vec<String> = (0..ROWS)
            .map(|r| ui::axis_label_for_test(r, ROWS, Some(name)))
            .collect();
        assert!(
            drawn.iter().any(|l| l.contains(name)),
            "series {name:?} never appears in its gutter: {drawn:?}"
        );
    }
}

#[test]
fn a_stall_is_measured_against_stall_thresholds_not_cpu_ones() {
    // A CPU at 50% is unremarkable; a machine that spent 50% of ten seconds
    // with nothing running at all is in serious trouble. Feeding the raw figure
    // to the utilisation thresholds leaves it cold until it is catastrophic.
    let t = Theme::new(Palette::Safe, Tier::TrueColor);
    assert_eq!(ui::stall_heat_for_test(0.0, &t), 0.0);
    assert_eq!(
        ui::stall_heat_for_test(4.9, &t),
        0.0,
        "cold below the floor"
    );
    assert_eq!(
        ui::stall_heat_for_test(6.1, &t),
        t.warn_pct,
        "the README's own worked example rendered cold"
    );
    assert_eq!(ui::stall_heat_for_test(25.0, &t), t.critical_pct);
}

#[test]
fn stall_colouring_follows_the_thresholds_the_user_set() {
    // Scaling the figure by a constant — which is what this did first —
    // silently reinterprets whatever the user configured: at `--warn 90` a
    // quadrupled figure needs 22.5% before it warns.
    let t = Theme::new(Palette::Safe, Tier::TrueColor).with_thresholds(90.0, 95.0);
    assert_eq!(
        ui::stall_heat_for_test(6.1, &t),
        90.0,
        "a raised threshold moved where stall starts warning"
    );
}

/// A sample with one busy interface and the given trouble counters.
fn with_net(rx: u64, tx: u64, retrans: Option<u64>, drops: Option<u64>) -> Sample {
    use crate::sample::{Link, NetStat};
    let mut s = sample(10.0);
    s.net = Some(NetStat {
        links: vec![
            Link {
                name: std::sync::Arc::from("lo0"),
                rx: 8,
                tx: 8,
                rx_packets: 1,
                tx_packets: 1,
            },
            Link {
                name: std::sync::Arc::from("en0"),
                rx,
                tx,
                rx_packets: 900,
                tx_packets: 400,
            },
        ],
        errors: Some(0),
        drops,
        retrans,
        listen_drops: Some(0),
    });
    s
}

/// A sample with one filesystem at the given fullness.
fn with_fs(used_pct: f32) -> Sample {
    use crate::sample::FsStat;
    let total = 1_000_000_000u64;
    let mut s = sample(10.0);
    s.filesystems = Some(vec![
        // Deliberately roomy, so the test controls fullness through one
        // filesystem and the other cannot trip the threshold on its own.
        FsStat {
            mount: std::sync::Arc::from("/boot"),
            total,
            avail: total - total / 10,
        },
        FsStat {
            mount: std::sync::Arc::from("/"),
            total,
            avail: (total as f32 * (1.0 - used_pct / 100.0)) as u64,
        },
    ]);
    s
}

#[test]
fn a_filesystem_with_room_spends_no_header_space_saying_so() {
    // The only figure here that describes a hard failure rather than a
    // slowdown, and the one that matters least on a machine with 400GB free.
    let mut app = App::new(60);
    app.push(with_fs(20.0));
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);
    assert!(
        !render(&app, 200, 30).contains("% full"),
        "an empty disk announced itself"
    );
}

#[test]
fn a_long_mount_point_does_not_push_the_header_apart() {
    // Every other figure budgets a fixed width. A container host can mount
    // something at `/var/snap/lxd/common/lxd/storage-pools/default`, and an
    // unbounded path would shove the figures ranked below it off the line.
    use crate::sample::FsStat;
    let mut app = App::new(60);
    let mut s = sample(10.0);
    s.filesystems = Some(vec![FsStat {
        mount: std::sync::Arc::from("/var/snap/lxd/common/lxd/storage-pools/default"),
        total: 1000,
        avail: 50,
    }]);
    app.push(s);
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);
    let f = render(&app, 200, 30);
    assert!(f.contains("95.0% full"), "the figure went missing");
    assert!(
        !f.contains("/var/snap/lxd/common"),
        "the whole path was rendered"
    );
    // Kept from the right, because that end identifies it.
    assert!(f.contains("default"), "the identifying end was cut: {f:?}");
}

#[test]
fn a_filesystem_close_to_full_is_named() {
    let mut app = App::new(60);
    app.push(with_fs(91.0));
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);
    let f = render(&app, 200, 30);
    assert!(f.contains("91.0% full"), "the figure was not shown: {f:?}");
    assert!(f.contains("/ "), "the mount point was not named");
}

#[test]
fn the_fullest_filesystem_is_the_one_reported() {
    // One figure, so it has to be the worst: a machine with a roomy root and a
    // full `/var` must not report itself roomy.
    let s = with_fs(97.0);
    assert_eq!(&*s.fullest().unwrap().mount, "/");
    assert!((s.fullest().unwrap().used_pct() - 97.0).abs() < 0.1);
}

#[test]
fn fullness_follows_the_threshold_the_user_set() {
    // "Close to full" is exactly the judgement the warn setting encodes, and
    // unlike a stall percentage a used-space percentage is the same kind of
    // quantity they set it for.
    let mut app = App::new(60);
    app.push(with_fs(60.0));
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor).with_thresholds(90.0, 95.0);
    assert!(
        !render(&app, 200, 30).contains("% full"),
        "a raised threshold did not move where fullness starts mattering"
    );
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor).with_thresholds(50.0, 80.0);
    assert!(render(&app, 200, 30).contains("60.0% full"));
}

#[test]
fn a_healthy_network_spends_no_header_space_saying_so() {
    // `NET 0 drops` every second would use the scarcest thing here to say
    // nothing happened.
    let mut app = App::new(60);
    app.push(with_net(1 << 20, 1 << 18, Some(0), Some(0)));
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);
    let f = render(&app, 200, 30);
    assert!(!f.contains("drops"), "a quiet network announced itself");
    assert!(!f.contains("retrans"));
    // But the throughput figure is there, and names the busy interface rather
    // than the loopback.
    assert!(f.contains("en0"), "the busy interface was not named");
}

#[test]
fn the_worst_thing_that_happened_is_the_one_reported() {
    // Ordered by how much each narrows the problem down, not by size. A machine
    // with one retransmit and four hundred errors is telling you about the
    // cable, but the retransmit is what changes what you do next.
    use crate::sample::NetStat;
    let n = |retrans, listen, drops, errors| NetStat {
        links: Vec::new(),
        errors,
        drops,
        retrans,
        listen_drops: listen,
    };
    assert_eq!(
        n(Some(1), Some(9), Some(9), Some(400)).trouble(),
        Some(("retrans", 1))
    );
    assert_eq!(
        n(Some(0), Some(2), Some(9), Some(400)).trouble(),
        Some(("listen drops", 2))
    );
    assert_eq!(
        n(Some(0), Some(0), Some(3), Some(400)).trouble(),
        Some(("drops", 3))
    );
    assert_eq!(
        n(Some(0), Some(0), Some(0), Some(400)).trouble(),
        Some(("errors", 400))
    );
    assert_eq!(n(Some(0), Some(0), Some(0), Some(0)).trouble(), None);
    // A platform that does not count a thing is not a platform where none of it
    // happened, so `None` is skipped rather than read as zero.
    assert_eq!(n(None, None, None, Some(5)).trouble(), Some(("errors", 5)));
}

#[test]
fn a_retransmitting_network_says_so_loudly() {
    let mut app = App::new(60);
    app.push(with_net(1 << 20, 1 << 18, Some(37), Some(0)));
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);
    let f = render(&app, 200, 30);
    assert!(
        f.contains("37 retrans"),
        "retransmits went unreported: {f:?}"
    );
}

#[test]
fn the_network_row_is_drawn_on_an_axis_of_its_own_units() {
    // It was left out while every series had to be a percentage of a fixed
    // denominator. Bytes a second has no such denominator, and normalising to
    // the window's own peak makes the busiest sample 100 by construction — an
    // idle laptop moving 8 B/s of loopback painted a full-scale graph straight
    // through the critical rule. With a unit of its own it carries a byte axis
    // and no rules.
    let mut app = App::new(60);
    for _ in 0..20 {
        // Two orders of magnitude apart, so a peak-relative scale would put the
        // smaller one at the top of the graph whatever its absolute size.
        app.push(with_net(8, 8, Some(0), Some(0)));
        app.push(with_net(1 << 20, 1 << 20, Some(0), Some(0)));
    }
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);

    let timeline: Vec<String> = rows(&app, 200, 48)
        .into_iter()
        .skip_while(|l| !l.contains("── timeline"))
        .take_while(|l| !l.contains("shown,"))
        .collect();
    assert!(
        timeline.iter().any(|l| l.contains("NET")),
        "the network row is still missing:\n{}",
        timeline.join("\n")
    );
    // Its axis is in bytes, not a bare percentage. 2 MiB a second at the peak,
    // and the ceiling sits strictly above it, so `4.0M` — never `100`, and
    // never the peak itself, which would draw the busiest sample as full scale
    // by construction.
    let axis = timeline
        .iter()
        .find(|l| l.contains('M') && !l.contains("MEM"))
        .unwrap_or_else(|| panic!("no byte axis:\n{}", timeline.join("\n")));
    assert!(
        axis.contains("4.0M"),
        "the axis is not in the series' own units: {axis:?}"
    );

    // The header still carries the figure too, which is where the number lives.
    assert!(render(&app, 200, 30).contains("en0"));
}

#[test]
fn a_series_that_is_not_a_percentage_carries_no_threshold_rules() {
    // The warn and critical percentages are shares of a whole. There is no
    // number of bytes a second at which a link is "critical", and ruling one
    // across the row says there is.
    //
    // Asserted on the decision rather than on a rendered frame, and the reason
    // is worth stating: a byte ceiling starts at a kilobyte, so fifty *bytes*
    // maps to the bottom five percent of the row — where the data already is,
    // and where the rule yields to it. The guard is there because the claim
    // would be false, not because it currently moves a pixel, and a rendered
    // test would pass with the guard removed and prove nothing. That is the
    // honest coverage available.
    assert!(
        ui::Unit::Percent.takes_thresholds_for_test(),
        "a share of a whole is exactly what warn and critical are about"
    );
    for unit in [ui::Unit::Rate, ui::Unit::Count] {
        assert!(
            !unit.takes_thresholds_for_test(),
            "a percentage threshold was applied to a series that is not one"
        );
    }

    // And the axes really do differ, which is the visible half.
    assert_eq!(ui::Unit::Percent.axis_for_test(100.0), "100");
    assert_eq!(ui::Unit::Count.axis_for_test(128.0), "128");
    assert_eq!(ui::Unit::Rate.axis_for_test(4.0 * 1024.0 * 1024.0), "4.0M");

    // A byte ceiling is a power of two from a kilobyte, so an idle link reads
    // as idle rather than being normalised to its own peak — the failure that
    // kept this row out of the panel.
    assert_eq!(ui::Unit::Rate.ceiling_for_test(8.0), 1024.0);
    assert_eq!(
        ui::Unit::Rate.ceiling_for_test(1_500_000.0),
        2.0 * 1024.0 * 1024.0
    );
    assert_eq!(ui::Unit::Count.ceiling_for_test(9.0), 16.0);
}

#[test]
fn the_header_names_which_resource_stopped_the_machine() {
    let mut app = App::new(60);
    app.push(with_pressure(7.5, 0.5));
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);
    let f = render(&app, 200, 30);
    assert!(f.contains("STALL"), "no stall figure");
    assert!(f.contains("io"), "the resource was not named");
    assert!(f.contains("7.5%"), "the figure was not shown");

    let mut app = App::new(60);
    app.push(with_pressure(0.5, 9.0));
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);
    let f = render(&app, 200, 30);
    assert!(f.contains("mem"), "memory pressure was reported as io");
    assert!(f.contains("9.0%"));
}

#[test]
fn cpu_never_wins_the_stall_figure() {
    // The kernel documents `full` as undefined for CPU and reports zero, so
    // including it would win every tie and name the wrong resource.
    use crate::sample::{Pressure, Stall};
    let p = Pressure {
        cpu: Stall {
            some: 99.0,
            full: 99.0,
        },
        io: Stall {
            some: 1.0,
            full: 0.5,
        },
        memory: Stall::default(),
    };
    assert_eq!(p.worst(), ("io", 0.5));
}

#[test]
fn a_kernel_without_pressure_says_nothing_rather_than_zero() {
    let mut absent = App::new(60);
    let mut present = App::new(60);
    for _ in 0..20 {
        absent.push(sample(10.0));
        present.push(with_pressure(6.0, 0.0));
    }
    absent.theme = Theme::new(Palette::Safe, Tier::TrueColor);
    present.theme = Theme::new(Palette::Safe, Tier::TrueColor);

    assert!(
        !render(&absent, 200, 30).contains("STALL"),
        "a stall figure was drawn for a kernel that publishes none"
    );
    assert!(render(&present, 200, 30).contains("STALL"));

    let gutter = |app: &App, h: u16| {
        rows(app, 200, h)
            .iter()
            .skip_while(|l| !l.contains("── timeline"))
            .take_while(|l| !l.contains("shown,"))
            .any(|l| l.contains("STALL"))
    };
    assert!(
        !gutter(&absent, 44),
        "a stall graph was drawn with no pressure"
    );

    // And it never displaces the three rows that were there before it.
    for h in 14..46 {
        for row in ["CPU", "MEM"] {
            let has = |a: &App| {
                rows(a, 200, h)
                    .iter()
                    .skip_while(|l| !l.contains("── timeline"))
                    .take_while(|l| !l.contains("shown,"))
                    .any(|l| l.contains(row))
            };
            assert!(
                has(&present) || !gutter(&present, h),
                "at height {h} the stall graph displaced {row}"
            );
        }
    }
}

#[test]
fn an_idle_machine_names_the_device_the_collector_meant() {
    // Every device at zero is the common case, and `max_by` returns the *last*
    // of equal maxima — so the header would name whichever device happened to
    // sort last, `loop3 0.0%` where the collector meant `nvme0n1`. A figure
    // that names a device reads as "this is the disk poptop is watching", so
    // which one it picks matters even when the number does not.
    use crate::sample::DiskStat;
    let idle = |name: &str| DiskStat {
        name: std::sync::Arc::from(name),
        read: 0,
        write: 0,
        reads: 0,
        writes: 0,
        util: 0.0,
        await_ms: None,
        queue: 0.0,
    };
    let mut s = sample(10.0);
    s.disks = Some(vec![idle("nvme0n1"), idle("loop3"), idle("sdb")]);
    assert_eq!(&*s.busiest_disk().unwrap().name, "nvme0n1");
}

#[test]
fn the_busiest_device_is_the_one_reported() {
    // One figure, so it has to be the worst device rather than the first: a
    // machine with a quiet system disk and a saturated data disk must not
    // report itself calm.
    use crate::sample::DiskStat;
    let d = |name: &str, util: f32| DiskStat {
        name: std::sync::Arc::from(name),
        read: 0,
        write: 0,
        reads: 1,
        writes: 1,
        util,
        await_ms: Some(1.0),
        queue: 0.0,
    };
    let mut s = sample(10.0);
    s.disks = Some(vec![d("sda", 2.0), d("nvme0n1", 97.0), d("sdb", 40.0)]);
    assert_eq!(&*s.busiest_disk().unwrap().name, "nvme0n1");

    let mut app = App::new(60);
    app.push(s);
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);
    let frame = render(&app, 200, 30);
    assert!(frame.contains("nvme0n1"), "the quiet disk was reported");
    assert!(frame.contains("97.0%"));
}

#[test]
fn an_unknown_thread_count_is_a_dash_not_a_one() {
    // macOS reported a flat `1` for every process, which beside the CPU column
    // was not merely missing but contradictory: this process is using three
    // cores, and one thread cannot do that.
    //
    // Compared against a process that really does have one thread, rather than
    // looked for as a dash. Other cells render dashes too, so "the row contains
    // an em dash" passes even when the count is fabricated — the question is
    // whether the two rows can be told apart at all.
    let row = |threads| {
        let mut app = App::new(60);
        let mut s = sample(10.0);
        s.procs = vec![ProcSample {
            cpu: 301.3,
            threads,
            ..proc_named(1, "zzsentinel", 0.0, 1 << 30)
        }];
        app.push(s);
        app.theme = Theme::new(Palette::Safe, Tier::TrueColor);
        render(&app, 200, 40)
            .lines()
            .find(|l| l.contains("zzsentinel"))
            .expect("no process row")
            .to_string()
    };

    let unknown = row(None);
    let one = row(Some(1));
    assert_ne!(
        unknown, one,
        "a process with no thread count rendered identically to one with a single thread"
    );
    assert!(
        unknown.contains('—'),
        "an unknown thread count was not an em dash: {unknown:?}"
    );
    assert_eq!(
        row(Some(36)).matches("36").count(),
        1,
        "a real thread count went missing"
    );
}

#[test]
fn a_pid_with_no_start_time_gets_no_history_rather_than_the_wrong_one() {
    // The macOS case before `kinfo`: sysinfo would not say when a process this
    // user does not own had started, and the key quietly became the pid alone.
    // A process with no token must draw nothing, because a line drawn from a
    // recycled pid is a line made of two different programs — worse than none.
    let mut app = App::new(600);
    for i in (0..60).rev() {
        let mut s = sample_at(10.0, i as u64);
        let cpu = if i > 30 { 90.0 } else { 2.0 };
        s.procs = vec![ProcSample {
            cpu,
            started: None,
            ..proc_named(4242, "unknowable", 0.0, 1 << 20)
        }];
        app.push(s);
    }
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);

    let spark = spark_for(&app, "unknowable");
    let ink: u32 = spark
        .chars()
        .map(|c| (c as u32).saturating_sub(0x2800).count_ones())
        .sum();
    assert_eq!(
        ink, 0,
        "a process with no identity was given a graph: {spark:?}"
    );
}

#[test]
fn thread_churn_does_not_credit_growth_across_a_pid_with_no_identity() {
    // `churn` compares thread counts between two samples. Keyed on pid alone, a
    // recycled pid looks like a process that shrank from 40 threads to 1, and
    // the difference is lost. With no identity the new occupant is counted as
    // new, which is the honest reading of "we cannot tell these apart".
    use crate::history::churn;
    let mut fat = threaded(4242, 0, 40);
    fat.started = None;
    let mut thin = threaded(4242, 0, 1);
    thin.started = None;
    let before = sample_with(Some(100), vec![fat]);
    let after = sample_with(Some(100), vec![thin]);
    let c = churn(&before, &after).unwrap();
    assert_eq!(
        c.visible, 1,
        "an unidentifiable process was matched to the previous occupant of its pid"
    );
}

#[test]
fn a_process_absent_from_a_sample_leaves_a_gap_not_a_zero() {
    // "It was not running" and "it was running and idle" are different facts.
    use crate::history::series_for;
    let mut app = App::new(600);
    for i in (0..10).rev() {
        let mut s = sample_at(10.0, i as u64);
        // Present only in the newest half.
        s.procs = if i < 5 {
            vec![proc_named(7, "late", 50.0, 1 << 20)]
        } else {
            vec![]
        };
        app.push(s);
    }
    let series = series_for(&app.history, &[(7, 0)], 10);
    let s = &series[&(7, 0)];
    assert_eq!(s.len(), 10);
    assert!(s[..5].iter().all(Option::is_none), "absence became data");
    assert!(s[5..].iter().all(Option::is_some), "presence became a gap");
}

#[test]
#[ignore = "measurement"]
fn measure_render_with_sparklines() {
    let mut app = App::new(600);
    for i in (0..600).rev() {
        let mut s = sample_at((i as f32 * 1.7) % 100.0, i as u64);
        s.procs = (0..900)
            .map(|p| proc_named(p, "some-process-name", (p as f32) % 100.0, 1 << 20))
            .collect();
        app.push(s);
    }
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);
    let mut term = Terminal::new(TestBackend::new(200, 60)).unwrap();
    let n = 50;
    let t0 = std::time::Instant::now();
    for _ in 0..n {
        term.draw(|f| ui::draw(f, &app)).unwrap();
    }
    println!(
        "  render: {:?}/frame at 900 procs x 600 samples, 200x60",
        t0.elapsed() / n
    );
}

/// The rendered frame as one string per terminal row, for tests that care
/// about geometry rather than the presence of a substring.
/// The row carrying the header figures.
///
/// Named rather than indexed. The header lost a row when the live/paused marker
/// moved down into the figures, and a dozen assertions had been reading line 1
/// by number — every one of them silently repointed at the per-core meters.
fn figures_line(app: &App, w: u16, h: u16) -> String {
    render_lines(app, w, h)[0].clone()
}

fn render_lines(app: &App, w: u16, h: u16) -> Vec<String> {
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    term.draw(|f| ui::draw(f, app)).unwrap();
    let buf = term.backend().buffer().clone();
    (0..h)
        .map(|y| (0..w).map(|x| buf[(x, y)].symbol()).collect())
        .collect()
}

/// A history where samples stop for a while and then resume.
fn history_with_gap(app: &mut App, before: u64, gap: u64, after: u64) {
    let total = before + gap + after;
    for i in 0..before {
        app.history.push(sample_at(5.0, total - i));
    }
    for i in 0..after {
        app.history.push(sample_at(5.0, after - i));
    }
}

/// Columns carrying a seam through the *graph*, which is the only place a
/// seam means anything.
///
/// Counting rows inside the timeline rather than looking for the character
/// anywhere on screen: the legend names the glyph too, so a substring search is
/// satisfied by the legend alone and passes with the drawing removed. That cost
/// one vacuous test to find out.
fn seam_columns(app: &App, w: u16, h: u16) -> Vec<usize> {
    let lines = render_lines(app, w, h);
    let rows = ui::timeline_rows_range(h);
    let seam = app.glyphs.gap_glyph();
    (0..w as usize)
        .filter(|&x| {
            lines[rows.start as usize..rows.end as usize]
                .iter()
                .filter(|l| l.chars().nth(x) == Some(seam))
                .count()
                >= 4
        })
        .collect()
}

#[test]
fn a_sampling_gap_is_visible_in_the_timeline() {
    let mut app = App::new(600);
    history_with_gap(&mut app, 40, 300, 40);
    assert_eq!(
        seam_columns(&app, 100, 40).len(),
        1,
        "expected exactly one full-height seam in the graph"
    );
}

#[test]
fn an_idle_stretch_is_not_mistaken_for_a_gap() {
    // The distinguishing case the item asks for: the same flat 5% signal, the
    // same length, sampled without interruption. A seam here would mean the
    // feature cannot tell absence from quiet, which is the whole distinction.
    let mut app = App::new(600);
    for i in 0..80 {
        app.history.push(sample_at(5.0, 80 - i));
    }
    assert!(
        seam_columns(&app, 100, 40).is_empty(),
        "an uninterrupted idle stretch drew a gap seam"
    );
}

/// The slot size the timeline legend claims, e.g. `2s`.
///
/// Read back from the rendered frame rather than computed, because it is the
/// observable that proves zoom reached the drawing at all.
fn slot_size(app: &App, w: u16, h: u16) -> String {
    render_lines(app, w, h)
        .into_iter()
        .find_map(|l| {
            let (head, _) = l.split_once("/slot")?;
            Some(head.rsplit(", ").next()?.trim().to_string())
        })
        .expect("the timeline legend names its slot size")
}

#[test]
fn a_gap_survives_being_zoomed_out() {
    // The failure guarded against is aggregation quietly dropping the gap at
    // the zoom level where the whole buffer fits on screen — exactly the view
    // you would be in to notice one.
    //
    // The buffer has to be big enough that zoom does something. An earlier
    // version of this test used 80 samples against 192 slots, where
    // `effective_zoom` clamps every level to 1: it asserted four times that
    // zoom 1 works, and never reached the aggregation it claimed to test.
    // Hence the slot-size check below, which fails if that comes back.
    let mut app = App::new(600);
    history_with_gap(&mut app, 400, 300, 60);

    let mut sizes = std::collections::HashSet::new();
    for _ in 0..crate::app::ZOOM_LEVELS.len() {
        assert!(
            !seam_columns(&app, 100, 40).is_empty(),
            "gap vanished at zoom {}",
            app.zoom()
        );
        sizes.insert(slot_size(&app, 100, 40));
        app.zoom_out();
    }
    assert!(
        sizes.len() > 1,
        "every zoom level drew the same slot size {sizes:?} — the buffer is \
         too small for zoom to have any effect, so nothing was aggregated"
    );
}

#[test]
fn gaps_are_missing_samples_not_slow_ones() {
    use std::time::{Duration, SystemTime};
    let base = SystemTime::UNIX_EPOCH;
    let at = |ms: u64| base + Duration::from_millis(ms);
    let nominal = Duration::from_secs(1);

    // Jitter under load stretches an interval; it does not double it.
    let jittery = [at(0), at(1000), at(2400), at(3900), at(5800)];
    assert_eq!(
        crate::history::gaps_in(&jittery, nominal),
        vec![false; 5],
        "collection jitter was reported as missing time"
    );

    // Two intervals means one tick went unobserved.
    let slept = [at(0), at(1000), at(3000), at(4000)];
    assert_eq!(
        crate::history::gaps_in(&slept, nominal),
        vec![false, false, true, false]
    );

    // A clock that stepped backwards is not a gap. Reporting it as one would
    // paint seams across the whole graph of a machine that just synced NTP.
    let stepped = [at(5000), at(1000), at(2000)];
    assert_eq!(
        crate::history::gaps_in(&stepped, nominal),
        vec![false, false, false]
    );
}

#[test]
fn zooming_out_cannot_erase_a_gap() {
    // Every position within a slot, since an aggregation that only checked the
    // first or last sample would pass a single-position test.
    for pos in 0..4 {
        let mut flags = vec![false; 8];
        flags[pos] = true;
        let slots = crate::history::any_slots(&flags, 4, 2);
        assert_eq!(
            slots,
            vec![true, false],
            "a gap at position {pos} of a 4-sample slot was aggregated away"
        );
    }
}

#[test]
#[ignore]
fn show_gap_frame() {
    let mut app = App::new(600);
    history_with_gap(&mut app, 40, 300, 40);
    for line in render_lines(&app, 92, 26) {
        println!("|{line}|");
    }
}

#[test]
fn the_caption_reports_real_time_not_sample_count() {
    // A caption saying `1m20s shown` beside a seam saying `time missing` is
    // the graph contradicting itself in adjacent characters. These 80 samples
    // span about 380 seconds, because 300 of them were never taken.
    let mut app = App::new(600);
    history_with_gap(&mut app, 40, 300, 40);
    let caption = render_lines(&app, 100, 40)
        .into_iter()
        .find(|l| l.contains(" shown, "))
        .expect("the timeline captions its span");
    // Taken from around " shown", not from the start of the line: the caption
    // is centred between the `past` and `now` anchors now, so the first token
    // on the row is `past`.
    let span = caption
        .split(" shown")
        .next()
        .unwrap()
        .split_whitespace()
        .last()
        .unwrap();

    // Asserted as a range, not a figure: the fixture builds its timestamps
    // from repeated `now()` calls, so the span is 379s give or take the time
    // the loop itself took. Counting samples would say 1m20s, which is nowhere
    // near this window, so the bug is still caught with room to spare.
    let (m, s) = span.split_once('m').expect("minutes");
    let secs: u64 =
        m.parse::<u64>().unwrap() * 60 + s.trim_end_matches('s').parse::<u64>().unwrap();
    assert!(
        (370..390).contains(&secs),
        "caption {span:?} = {secs}s; the window spans ~380s of wall clock, \
         and 80s only if you count samples and call each one a second"
    );
}

#[test]
fn the_legend_degrades_rather_than_truncating_a_word() {
    // The gap note costs about sixteen columns, which used to push `+/- zoom`
    // off the panel mid-word. Losing the whole hint is fine; losing half of it
    // reads as a rendering bug.
    let mut app = App::new(600);
    history_with_gap(&mut app, 40, 300, 40);
    for w in 30..=110u16 {
        let legend = render_lines(&app, w, 40)
            .into_iter()
            .find(|l| l.contains(" shown, "))
            .unwrap_or_default();
        assert!(
            !legend.contains('←') || legend.contains("+/- zoom"),
            "at w={w} the key hint was cut: {legend:?}"
        );
    }
}

#[test]
#[ignore]
fn show_sample_footprint() {
    use crate::sample::{ProcSample, Sample};
    println!("ProcSample: {} bytes", std::mem::size_of::<ProcSample>());
    println!("Sample:     {} bytes", std::mem::size_of::<Sample>());
    for procs in [100usize, 400, 4000] {
        let per = std::mem::size_of::<Sample>() + procs * std::mem::size_of::<ProcSample>();
        for (label, samples) in [
            ("10m at 1s", 600usize),
            ("1h at 1s", 3600),
            ("10m at 100ms", 6000),
            ("cap", 86_400),
        ] {
            println!(
                "{procs:5} procs, {label:14} = {samples:6} samples -> {:6.1} MB",
                (per * samples) as f64 / 1e6
            );
        }
    }
}

#[test]
fn spans_are_legible_at_both_ends_of_the_configurable_range() {
    // Whole seconds rendered a 100ms slot as `0s/slot` — the exact mode
    // sub-second sampling exists for — and a day-long window as `1440m00s`,
    // which is a number nobody reads as a day.
    for (ms, want) in [
        (50u64, "50ms"),
        (500, "500ms"),
        (1_000, "1s"),
        (59_000, "59s"),
        (60_000, "1m00s"),
        (380_000, "6m20s"),
        (3_600_000, "1h00m"),
        (86_400_000, "24h00m"),
    ] {
        assert_eq!(
            ui::fmt_lag_for_test(std::time::Duration::from_millis(ms)),
            want
        );
    }
}

#[test]
fn a_busy_frame_at_a_fast_rate_is_not_a_gap() {
    // At `interval = 50ms` a frame that took 100ms to draw and collect is
    // twice the nominal rate, and without a floor the timeline would paint
    // itself full of seams for a monitor that is merely busy.
    let base = std::time::SystemTime::UNIX_EPOCH;
    let at = |ms: u64| base + std::time::Duration::from_millis(ms);
    let fast = std::time::Duration::from_millis(50);

    let busy = [at(0), at(100), at(220), at(300)];
    assert_eq!(
        crate::history::gaps_in(&busy, fast),
        vec![false; 4],
        "a busy frame at 50ms was reported as time missing"
    );
    // A real stall still is one.
    let stalled = [at(0), at(50), at(2_000)];
    assert_eq!(
        crate::history::gaps_in(&stalled, fast),
        vec![false, false, true]
    );
}

#[test]
#[ignore]
fn write_builtin_theme_files() {
    // Writes `themes/*.theme` from the compiled palettes. Run it after
    // changing a palette, then commit the result:
    //
    //     cargo test -- --ignored write_builtin_theme_files
    //
    // `the_shipped_themes_match_the_compiled_ones` fails until you do.
    //
    // It writes rather than prints because cargo captures stdout for passing
    // tests: printing made the documented workflow silently do nothing, which
    // is a poor way to answer a test telling you to run it.
    use crate::theme::{Palette, Theme, Tier, Token, write_color};
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("themes");
    std::fs::create_dir_all(&dir).unwrap();
    for palette in [Palette::Safe, Palette::Classic] {
        let th = Theme::new(palette, Tier::TrueColor);
        let mut out = format!(
            "# poptop's built-in `{}` palette, as a theme file.\n\
             #\n\
             # Copy it to ~/.config/poptop/themes/mine.theme and change what you like.\n\
             # Every line is optional: a theme inherits `safe` for anything it does\n\
             # not name, so overriding two colours is two lines.\n\
             #\n\
             # Colours are `#rrggbb`, a 256-colour index like `80`, or an ANSI name\n\
             # like `cyan`. Generated by the `write_builtin_theme_files` test.\n\n",
            palette.name()
        );
        for token in Token::ALL {
            out += &format!("{:<12} = {}\n", token.name(), write_color(token.get(&th)));
        }
        let path = dir.join(format!("{}.theme", palette.name()));
        std::fs::write(&path, out).unwrap();
        println!("wrote {}", path.display());
    }
}

#[test]
fn a_colour_the_terminal_cannot_show_is_reported_not_approximated() {
    use crate::theme::{Palette, Theme, Tier, Token};
    use ratatui::style::Color;

    let wants = [
        (Token::Ok, Color::Rgb(0x8f, 0xbc, 0xbb)),
        (Token::Warn, Color::Indexed(222)),
        (Token::Critical, Color::Red),
    ];

    // Each tier takes what it can show and leaves the rest alone. Quantising
    // instead would destroy the separation the palettes were measured for, and
    // the sixteen ANSI slots are the user's terminal theme, not poptop's to
    // approximate into.
    for (tier, want_skipped) in [
        (Tier::TrueColor, vec![]),
        (Tier::Ansi256, vec![Token::Ok]),
        (Tier::Ansi16, vec![Token::Ok, Token::Warn]),
        // Monochrome ignores palettes entirely; a user theme does not get to
        // weaken the one guarantee that holds without colour at all.
        (Tier::Mono, vec![Token::Ok, Token::Warn, Token::Critical]),
    ] {
        let (theme, skipped) = Theme::new(Palette::Safe, tier).with_overrides(&wants);
        assert_eq!(skipped, want_skipped, "at {tier:?}");
        for (token, colour) in wants {
            let built_in = Token::get(token, &Theme::new(Palette::Safe, tier));
            let expected = if skipped.contains(&token) {
                built_in
            } else {
                colour
            };
            assert_eq!(
                Token::get(token, &theme),
                expected,
                "{} at {tier:?}",
                token.name()
            );
        }
    }
}

#[test]
#[ignore]
fn show_churn_against_a_real_burst() {
    // The case the item is about: processes that live milliseconds. Run on
    // Linux, where /proc/stat publishes the counter.
    use crate::collect::{Collector, Needs, Platform};
    let mut c = Platform::new().unwrap();
    let a = c.sample(Needs::NONE).unwrap();
    for _ in 0..300 {
        let _ = std::process::Command::new("/bin/true").status();
    }
    let b = c.sample(Needs::NONE).unwrap();
    match crate::history::churn(&a, &b) {
        Some(ch) => println!(
            "created {} tasks, {} visible in the table, {} came and went",
            ch.created,
            ch.visible,
            ch.unseen()
        ),
        None => println!("no fork counter on this platform"),
    }
    println!("procs before {} after {}", a.procs.len(), b.procs.len());
}

/// A sample with a stated fork counter and process list.
fn sample_with(forks: Option<u64>, procs: Vec<ProcSample>) -> Sample {
    Sample {
        forks,
        procs,
        ..sample(0.0)
    }
}

fn threaded(pid: i32, started: u64, threads: u32) -> ProcSample {
    ProcSample {
        started: Some(started),
        threads: Some(threads),
        ..proc_named(pid, "worker", 0.0, 0)
    }
}

#[test]
fn processes_that_lived_and_died_between_samples_are_counted() {
    // The case the whole item is about: 300 tasks created, none of them still
    // alive when poptop looked. The table cannot show them; it can refuse to
    // imply they did not happen.
    let a = sample_with(Some(1_000), vec![threaded(1, 0, 1)]);
    let b = sample_with(Some(1_300), vec![threaded(1, 0, 1)]);
    let churn = crate::history::churn(&a, &b).expect("both samples have the counter");
    assert_eq!(churn.created, 300);
    assert_eq!(churn.visible, 0);
    assert_eq!(churn.unseen(), 300);
}

#[test]
fn threads_are_counted_as_the_tasks_they_are() {
    // The kernel's counter advances on `clone` as well as `fork`, so comparing
    // it against a count of process *rows* would report a program that spawned
    // sixteen threads as sixteen invisible processes — worse than saying
    // nothing, because it invents an event.
    let a = sample_with(Some(100), vec![threaded(1, 0, 1)]);
    let b = sample_with(Some(116), vec![threaded(1, 0, 17)]);
    let churn = crate::history::churn(&a, &b).unwrap();
    assert_eq!(churn.created, 16);
    assert_eq!(churn.visible, 16, "thread growth was not credited");
    assert_eq!(
        churn.unseen(),
        0,
        "sixteen threads were reported as invisible"
    );

    // A wholly new process brings its threads with it.
    let c = sample_with(Some(120), vec![threaded(1, 0, 1), threaded(2, 5, 4)]);
    let churn = crate::history::churn(&a, &c).unwrap();
    assert_eq!(churn.visible, 4);
}

#[test]
fn a_recycled_pid_is_not_mistaken_for_the_process_that_had_it() {
    // Keyed on pid *and* start time. On pid alone the new process looks like
    // one that was here all along, so its tasks land on the wrong side and the
    // interval under-reports what it could not see.
    let a = sample_with(Some(100), vec![threaded(42, 111, 8)]);
    let b = sample_with(Some(104), vec![threaded(42, 999, 4)]);
    let churn = crate::history::churn(&a, &b).unwrap();
    assert_eq!(
        churn.visible, 4,
        "a recycled pid was credited as the old process continuing"
    );
    assert_eq!(churn.unseen(), 0);
}

#[test]
fn a_platform_that_cannot_say_says_nothing() {
    // "I do not know" and "none happened" are opposite answers, and a
    // fabricated zero would quietly promise the table is complete. macOS
    // publishes no equivalent of /proc/stat's `processes`.
    let known = sample_with(Some(100), vec![]);
    let unknown = sample_with(None, vec![]);
    assert!(crate::history::churn(&unknown, &known).is_none());
    assert!(crate::history::churn(&known, &unknown).is_none());
    assert!(crate::history::churn(&unknown, &unknown).is_none());
}

#[test]
fn the_process_panel_says_what_it_could_not_show() {
    // Without this the table sits under a graph it cannot explain and says
    // nothing about why.
    let mut app = App::new(60);
    app.push(sample_with(Some(1_000), vec![threaded(1, 0, 1)]));
    app.push(sample_with(Some(1_047), vec![threaded(1, 0, 1)]));
    let frame = render(&app, 100, 30);
    assert!(
        frame.contains("47 tasks came and went"),
        "the panel does not disclose the interval's churn"
    );

    // …and says nothing when there is nothing to say.
    let mut quiet = App::new(60);
    quiet.push(sample_with(Some(1_000), vec![threaded(1, 0, 1)]));
    quiet.push(sample_with(Some(1_000), vec![threaded(1, 0, 1)]));
    assert!(!render(&quiet, 100, 30).contains("came and went"));
}

#[test]
fn churn_is_not_summed_across_a_sleep() {
    // The two samples either side of a suspend can be hours apart, and the
    // counter would attribute a whole night's task creation to the one second
    // the table is describing. The timeline already draws a seam there; this
    // is the same event and must not be summed through it.
    let mut app = App::new(60);
    app.history
        .push(sample_with_at(Some(1_000), 4_000, vec![threaded(1, 0, 1)]));
    app.history
        .push(sample_with_at(Some(1_204_331), 0, vec![threaded(1, 0, 1)]));
    let frame = render(&app, 100, 30);
    assert!(
        !frame.contains("came and went"),
        "a sleep's task creation was billed to one second"
    );

    // The same two counts one interval apart are reported normally, so the
    // suppression is about the gap and not about the size of the number.
    let mut adjacent = App::new(60);
    adjacent
        .history
        .push(sample_with_at(Some(1_000), 1, vec![threaded(1, 0, 1)]));
    adjacent
        .history
        .push(sample_with_at(Some(1_204_331), 0, vec![threaded(1, 0, 1)]));
    assert!(render(&adjacent, 100, 30).contains("came and went"));
}

#[test]
fn the_panel_says_tasks_because_that_is_what_it_counts() {
    // A thread pool recycling workers advances the kernel's counter without
    // changing any process's thread count, so its turnover is unseen by this
    // definition. Calling that "processes" would invent an event — a JVM at
    // steady state would show a permanent phantom count of short-lived
    // processes that do not exist.
    let mut app = App::new(60);
    app.push(sample_with(Some(1_000), vec![threaded(1, 0, 8)]));
    app.push(sample_with(Some(1_064), vec![threaded(1, 0, 8)]));
    let frame = render(&app, 100, 30);
    assert!(frame.contains("64 tasks came and went"), "{frame:?}");
}

/// A sample with a fork counter, a process list, and an age in seconds.
fn sample_with_at(forks: Option<u64>, age_secs: u64, procs: Vec<ProcSample>) -> Sample {
    Sample {
        forks,
        procs,
        ..sample_at(0.0, age_secs)
    }
}

#[test]
#[ignore]
fn show_header_widths() {
    // The header is a fixed line that does not fit every terminal. Reading it
    // at each width is the only way to see what actually survives.
    let mut app = App::new(60);
    let mut s = sample(12.4);
    s.iowait = Some(61.2);
    s.running = Some(1);
    s.blocked = Some(23);
    s.cpu_per_core = vec![5.0; 14];
    app.push(s);
    for w in [140u16, 120, 100, 80, 60, 40] {
        let lines = render_lines(&app, w, 24);
        println!("{w:>4} |{}|", lines[1].trim_end());
    }
}

/// A sample of a machine that is stalled rather than busy: almost no CPU, most
/// of the wall clock waiting on disk, nothing runnable, plenty stuck in D.
fn stalled() -> Sample {
    let mut s = sample(2.0);
    s.iowait = Some(61.2);
    s.running = Some(1);
    s.blocked = Some(23);
    s.cpu_per_core = vec![2.0; 14];
    s
}

#[test]
fn a_stalled_machine_does_not_look_like_an_idle_one() {
    // The case every troubleshooting guide names as the confusing one: high
    // load, idle CPU. Before this, poptop rendered it as a calm 2% and said
    // nothing about why the box was on its knees.
    let mut busy = App::new(60);
    busy.push(sample(2.0));
    let mut stuck = App::new(60);
    stuck.push(stalled());

    let idle_header = figures_line(&busy, 120, 24).clone();
    let stalled_header = figures_line(&stuck, 120, 24).clone();
    assert_ne!(
        idle_header, stalled_header,
        "a machine with 23 tasks stuck in D reads identically to an idle one"
    );
    assert!(stalled_header.contains("61.2"), "{stalled_header}");
    assert!(stalled_header.contains("BLOCKED 23"), "{stalled_header}");
}

#[test]
fn runnable_is_reported_against_the_cores_it_competes_for() {
    // Four runnable is catastrophic on one core and idle on ninety-six, so the
    // bare count is not a fact anyone can act on.
    let mut app = App::new(60);
    let mut s = stalled();
    s.running = Some(4);
    s.cpu_per_core = vec![1.0; 96];
    app.push(s);
    assert!(
        figures_line(&app, 120, 24).contains("RUN 4/96"),
        "the runnable count is missing its denominator"
    );
}

#[test]
fn a_platform_that_cannot_see_a_signal_omits_it_rather_than_showing_zero() {
    // macOS publishes none of these. "I cannot see this" and "there is none of
    // it" are opposite answers, and a zero would claim the box is never stuck.
    let mut app = App::new(60);
    app.push(sample(2.0)); // iowait/running/blocked all None
    let header = figures_line(&app, 120, 24).clone();
    for absent in ["WAIT", "RUN ", "BLOCKED"] {
        assert!(
            !header.contains(absent),
            "{absent} was reported on a platform that cannot see it: {header}"
        );
    }
    assert!(
        header.contains("CPU"),
        "the rest of the header went with it"
    );
}

#[test]
fn a_group_separator_is_measured_in_columns_not_bytes() {
    // `│` is one column and three bytes. Charging its byte length overstated
    // every group boundary by two, so the header dropped figures that fitted
    // and left a fistful of columns unused.
    let mut app = App::new(60);
    app.push(stalled());
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);

    // The units, directly. `│` is one column and three bytes, so a separator
    // that looks five characters long is seven bytes long — and the byte length
    // is what the fitting arithmetic used, while `full_width` had the correct
    // five written out by hand. The two disagreed, and the header dropped
    // figures that fitted while leaving columns unused.
    let (near_cols, near_bytes, far_cols, far_bytes) = ui::separator_widths_for_test();
    assert_eq!(near_cols, 2);
    assert_eq!(far_cols, 5, "the group separator is not five columns wide");
    assert_ne!(
        far_cols, far_bytes,
        "this proves nothing unless columns and bytes differ"
    );
    assert_eq!(near_cols, near_bytes, "the narrow separator is plain ASCII");

    // And every figure is still there when there is room for it.
    let line = figures_line(&app, 300, 24);
    for label in [
        "CPU", "WAIT", "RUN", "BLOCKED", "LOAD", "MEM", "SWP", "UP ", "PROCS",
    ] {
        assert!(
            line.contains(label),
            "{label} missing at 300 columns: {line:?}"
        );
    }
}

#[test]
fn a_deep_tree_never_leaves_a_row_without_a_name() {
    // A tree prefix grows three columns a level against a command column that
    // is nineteen wide, so a deep enough chain elided the name to nothing and
    // the row rendered as a bare `└`. Before eliding, the terminal clipped —
    // which at least kept the head — so this was a regression against doing
    // nothing.
    let mut app = App::new(60);
    let mut s = sample(10.0);
    s.procs = (0..9)
        .map(|i| {
            let mut p = proc_named(i + 101, "Google Chrome Helper (Renderer)", 0.0, 1 << 20);
            // Two users, so the USER column keeps its ten columns. This test is
            // about a deep tree starving the name, not about a table that has
            // folded a constant column into its title — which would hand those
            // ten columns back and hide what is being measured here.
            if i == 0 {
                p.user = std::sync::Arc::from("someone-else");
            }
            p.cpu = 10.0 - i as f32;
            p.ppid = if i == 0 { 0 } else { i + 100 };
            p
        })
        .collect();
    app.push(s);
    app.tree = true;
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);

    let rows: Vec<String> = rows(&app, 104, 24)
        .into_iter()
        .filter(|l| l.contains("Chrome") || l.contains('…'))
        .collect();
    assert_eq!(rows.len(), 9, "expected nine rows: {rows:?}");
    for r in &rows {
        let row = r.trim_end();
        // The last whitespace-separated token is the name. A bare connector or
        // a single letter is the failure this test exists for.
        let name = row.rsplit(' ').next().unwrap_or("");
        assert!(
            row.ends_with(')'),
            "a row lost the end of its name to the indent: {row:?}"
        );
        let (head, tail) = name
            .split_once('…')
            .unwrap_or_else(|| panic!("name {name:?} was not elided at all: {row:?}"));
        assert!(
            !head.is_empty() && !tail.is_empty(),
            "the indent left only {name:?}: {row:?}"
        );
    }
}

#[test]
fn a_narrow_table_does_not_elide_the_name_to_a_letter() {
    // Below its floor ratatui squeezes the fixed columns instead of honouring
    // them, so the command cell is wider than the arithmetic says. Eliding
    // against the arithmetic rendered a thirty-character name as `G`.
    let mut app = App::new(60);
    let mut s = sample(10.0);
    s.procs = vec![ProcSample {
        cpu: 9.0,
        ..proc_named(1, "Google Chrome Helper (Renderer)", 0.0, 1 << 20)
    }];
    app.push(s);
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);

    // Below `FIXED_COLUMNS + spacing + MIN_COMMAND_W` — about 75 — ratatui
    // stops honouring the fixed lengths, which is exactly where the arithmetic
    // and the drawing part company.
    for w in [64u16, 70, 74, 80, 100] {
        let row = rows(&app, w, 20)
            .into_iter()
            .find(|l| l.contains("Google") || l.contains('…'))
            .unwrap_or_else(|| panic!("no process row at w={w}"));
        let name: String = row
            .trim_end()
            .chars()
            .rev()
            .take_while(|c| *c != ' ')
            .collect();
        assert!(
            name.chars().count() >= 8,
            "the name was cut to {:?} at w={w}",
            name.chars().rev().collect::<String>()
        );
    }
}

#[test]
fn a_tree_prefix_is_charged_against_the_name_it_indents() {
    // The prefix and the name share one column. Eliding the name against the
    // column's full width lets `│  └─ ` push its tail off the end — the tail
    // being the half that says which of several similar processes this is.
    let mut app = App::new(60);
    let mut s = sample(10.0);
    let long = "Google Chrome Helper (Renderer)";
    s.procs = vec![
        proc_named(1, "launchd", 0.1, 1 << 20),
        ProcSample {
            cpu: 9.0,
            ..proc_named(42, long, 0.0, 1 << 20)
        },
    ];
    s.procs[1].ppid = 1;
    app.push(s);
    app.tree = true;
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);

    let row = rows(&app, 104, 20)
        .into_iter()
        .find(|l| l.contains("Chrome") || l.contains('…'))
        .expect("no row for the nested process");
    let row = row.trim_end();
    assert!(
        row.ends_with("nderer)"),
        "the indent pushed the identifying tail off the line: {row:?}"
    );
    assert!(
        row.chars().count() <= 104,
        "the row overflowed its terminal: {row:?}"
    );
}

#[test]
fn a_long_name_keeps_both_ends() {
    // Cutting the tail is what the terminal does on its own, and for a process
    // name it removes exactly the part that tells two of them apart.
    assert_eq!(ui::elide_middle("short", 20), "short");
    assert_eq!(ui::elide_middle("exactlyten", 10), "exactlyten");

    let cut = ui::elide_middle("Google Chrome Helper (Renderer)", 20);
    assert_eq!(cut.chars().count(), 20);
    assert!(cut.starts_with("Google"), "the head was lost: {cut:?}");
    assert!(
        cut.ends_with("nderer)"),
        "the identifying tail was lost: {cut:?}"
    );
    assert!(cut.contains('…'), "no elision mark: {cut:?}");

    // Absurd widths do not panic or produce something wider than asked for.
    for w in 0..8 {
        assert!(ui::elide_middle("Google Chrome Helper", w).chars().count() <= w);
    }
}

#[test]
fn processes_that_differ_only_by_a_suffix_are_told_apart() {
    // Three rows reading `Google Chrome Helpe` are a renderer, a GPU process
    // and a network service, and the table said nothing about which was which.
    let mut app = App::new(60);
    let mut s = sample(10.0);
    s.procs = vec![
        ProcSample {
            cpu: 9.0,
            // Two users, so the USER column is not folded into the title and
            // the command column is the width this test is about.
            user: std::sync::Arc::from("someone-else"),
            ..proc_named(101, "Google Chrome Helper (Renderer)", 0.0, 1 << 20)
        },
        ProcSample {
            cpu: 8.0,
            ..proc_named(102, "Google Chrome Helper (GPU)", 0.0, 1 << 20)
        },
        ProcSample {
            cpu: 7.0,
            ..proc_named(103, "Google Chrome Helper (Network Service)", 0.0, 1 << 20)
        },
    ];
    app.push(s);
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);

    let shown: Vec<String> = rows(&app, 104, 20)
        .into_iter()
        .filter(|l| l.contains("Chrome") || l.contains('…'))
        .map(|l| l.trim_end().to_string())
        .collect();
    assert_eq!(shown.len(), 3, "expected three rows: {shown:?}");

    // The distinguishing tail, not the whole row. Whole rows differ by pid
    // whatever the name does, so comparing them passes even when every command
    // reads `Google Chrome Helpe` — which is the bug.
    for tail in ["Renderer)", "(GPU)", "Service)"] {
        assert!(
            shown.iter().any(|l| l.ends_with(tail)),
            "no row identifies itself as {tail}: {shown:?}"
        );
    }
    // And each was actually shortened, so the test is not passing because the
    // column happened to be wide enough.
    assert!(
        shown.iter().all(|l| l.contains('…')),
        "nothing was elided, so this proves nothing: {shown:?}"
    );
}

#[test]
fn figures_sit_with_the_resource_they_are_about() {
    // Ranked and ordered by one number, the header read compute, storage,
    // compute, network, memory, network, memory, machine — network split in
    // half with memory in between, and `LOAD`, the most compute figure there
    // is, after uptime.
    let mut app = App::new(60);
    app.push(stalled());
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);
    let line = figures_line(&app, 220, 24);

    let at = |s: &str| {
        line.find(s)
            .unwrap_or_else(|| panic!("no {s:?} in {line:?}"))
    };
    // Compute, all of it, before memory.
    assert!(at("CPU") < at("WAIT"));
    assert!(at("WAIT") < at("RUN"));
    assert!(at("RUN") < at("BLOCKED"));
    assert!(at("BLOCKED") < at("LOAD"), "load left its own group");
    assert!(
        at("LOAD") < at("MEM"),
        "compute did not finish before memory"
    );
    // Then memory, whole — and in the order it is written rather than the order
    // it is ranked. `MEM`, its byte detail and `SWP` rank 50, 90 and 60, so
    // sorting the group by rank would put the byte figure after swap, away from
    // the bar it belongs to. That is what makes the sort's stability load
    // bearing rather than incidental.
    assert!(at("MEM") < at("SWP"));
    let bytes = at(" / ");
    assert!(
        at("MEM") < bytes && bytes < at("SWP"),
        "the byte detail left the bar it explains: {line:?}"
    );
    // Then the machine facts, last.
    assert!(at("SWP") < at("UP "));
    assert!(at("UP ") < at("PROCS"));
}

#[test]
fn a_group_boundary_looks_different_from_a_gap_inside_one() {
    // A boundary that reads like the gap within a group is not a boundary. The
    // mark carries on a terminal with no colour to spend, which the dimmer
    // separator alone would not.
    let mut app = App::new(60);
    app.push(stalled());
    app.theme = Theme::new(Palette::Safe, Tier::Mono);
    let line = figures_line(&app, 220, 24);
    assert!(line.contains('│'), "no group boundary drawn: {line:?}");
    // One boundary per gap between adjacent groups present.
    let rules = line.matches('│').count();
    assert!((2..=5).contains(&rules), "{rules} boundaries in {line:?}");
    // And the figures inside a group are not separated by one.
    let compute = &line[..line.find('│').unwrap()];
    assert!(compute.contains("CPU") && compute.contains("WAIT"));
}

#[test]
fn the_state_marker_is_never_given_up_for_room() {
    // Reading a stale process table as the current one is the single worst
    // thing this tool could let you do, so the one figure that cannot be
    // dropped is the one saying whether it is stale.
    let mut app = App::new(600);
    for i in (0..40).rev() {
        app.push(sample_at(50.0, i));
    }
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);
    app.history.scrub(-20);
    for w in [200u16, 120, 80, 60, 40, 24] {
        let line = figures_line(&app, w, 24);
        assert!(
            line.contains("PAUSED"),
            "the staleness warning was dropped at w={w}: {line:?}"
        );
    }
}

#[test]
fn one_row_under_the_graph_carries_the_axis_and_the_scale() {
    // Two rows said the same thing: `past … now` and `2s shown, 1s/slot` are
    // both about the x-axis, and on a thirty-row terminal a duplicated chrome
    // row is a process the table cannot show.
    let mut app = App::new(600);
    for i in (0..40).rev() {
        app.push(sample_at(50.0, i));
    }
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);

    let range = ui::timeline_rows_range(30);
    let lines = render_lines(&app, 120, 30);
    let under: Vec<&String> = lines[range.start as usize..range.end as usize]
        .iter()
        .filter(|l| l.contains("past") || l.contains("shown"))
        .collect();
    assert_eq!(under.len(), 1, "the axis still costs two rows: {under:?}");
    let row = under[0];
    assert!(row.contains("past") && row.contains("now"), "{row:?}");
    assert!(row.contains("shown"), "the scale was lost: {row:?}");
}

#[test]
fn the_axis_row_repeats_no_key_that_the_footer_lists() {
    // A reminder that is always on screen twice is not a reminder.
    let mut app = App::new(600);
    for i in (0..40).rev() {
        app.push(sample_at(50.0, i));
    }
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);
    let range = ui::timeline_rows_range(30);
    let lines = render_lines(&app, 160, 30);
    let timeline = lines[range.start as usize..range.end as usize].join("\n");
    assert!(
        !timeline.contains("scrub"),
        "the timeline repeats the footer's keys: {timeline:?}"
    );
    // …and the footer still has them.
    assert!(lines.last().unwrap().contains("scrub"));
}

#[test]
fn the_header_gives_up_its_least_diagnostic_figures_first() {
    // Letting ratatui clip drops whatever is rightmost, and rightmost is not
    // least useful. The four figures that answer "why is this slow" have to
    // outlive uptime and a load average that conflates the two of them.
    let mut app = App::new(60);
    app.push(stalled());
    let at = |w: u16| figures_line(&app, w, 24).clone();

    // Load ranks last, so it is the last figure to *appear* as the terminal
    // widens rather than merely the first to go. That is the intended
    // consequence of ranking it below the two figures that take it apart.
    assert!(at(180).contains("LOAD"), "{}", at(180));
    let wide = at(140);
    assert!(wide.contains("UP ") && wide.contains("PROCS"), "{wide}");
    assert!(!wide.contains("LOAD"), "load outranked uptime: {wide}");

    // The discriminating pair: swap outranks the memory byte detail, but is
    // built after it. At a width that fits exactly one, rank has to decide —
    // without the ranking, insertion order keeps the wrong one.
    let middle = at(100);
    // `avail` used to be the marker here; the memory bar replaced that text,
    // which quietly made the negative clause unfalsifiable. The byte detail is
    // still the lower-ranked half of the pair, so it is still the discriminator
    // — it just has different words now.
    assert!(
        middle.contains("SWP") && !middle.contains(" / 16.0G"),
        "figures were kept in build order rather than by rank: {middle}"
    );
    // …and the ranking holds as a rule rather than at one lucky width: under a
    // prefix rule a lower-ranked figure can never appear without every figure
    // above it. A fat figure ranked high blocks everything behind it, which
    // once cost a hundred-column terminal two figures that fit twice over.
    for w in 20..=200u16 {
        let line = at(w);
        let has = |s: &str| line.contains(s);
        for (lower, higher) in [
            ("8.0G / 16.0G", "PROCS"),
            ("PROCS", "UP "),
            ("UP ", "SWP"),
            ("LOAD", "PROCS"),
        ] {
            assert!(
                !has(lower) || has(higher),
                "at w={w} `{lower}` appeared without `{higher}`: {line}"
            );
        }
    }

    let narrow = at(60);
    for kept in ["CPU", "WAIT", "RUN", "BLOCKED"] {
        assert!(narrow.contains(kept), "{kept} was dropped at 60: {narrow}");
    }
    assert!(
        !narrow.contains("LOAD"),
        "load outlived the signals: {narrow}"
    );

    // …and nothing is ever cut mid-figure, at any width.
    for w in 20..=140u16 {
        let line = at(w);
        assert!(
            line.chars().count() <= w as usize,
            "the header overflowed at w={w}"
        );
        for figure in ["WAIT", "BLOCKED", "LOAD"] {
            if let Some(rest) = line.split(figure).nth(1) {
                assert!(
                    rest.starts_with(' ') || rest.is_empty(),
                    "{figure} was cut in half at w={w}: {line}"
                );
            }
        }
    }
}

#[test]
fn the_process_count_survives_in_the_header() {
    // It was in the header before the ranking, and dropping it was an
    // unannounced regression — `fit`'s own comment cites it as the reason the
    // ranking exists at all.
    let mut app = App::new(60);
    app.push(stalled());
    assert!(figures_line(&app, 140, 24).contains("PROCS"));
}

#[test]
#[ignore]
fn show_timeline_heights() {
    // The timeline gains and loses series with height. Reading it at each is
    // the only way to see whether the ladder is sane.
    let mut app = App::new(600);
    for i in (0..200).rev() {
        let mut s = sample_at(((200 - i) as f32 * 0.4).sin().abs() * 90.0, i as u64);
        s.iowait = Some(((200 - i) as f32 * 0.13).sin().abs() * 70.0);
        s.running = Some(2);
        s.blocked = Some(9);
        app.push(s);
    }
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);
    // The narrow-tall case is in here too: without a gutter the series count
    // is clamped, so it is a different ladder rather than the same one.
    for (w, h) in [(64u16, 24u16), (64, 34), (64, 50), (28, 50)] {
        println!("=== {w}x{h} ===");
        let rows = ui::timeline_rows_range(h);
        for line in &render_lines(&app, w, h)[rows.start as usize..rows.end as usize] {
            println!("{}", line.trim_end());
        }
    }
}

/// A history of a machine that is stalled as well as busy.
fn stalled_history(app: &mut App, n: u64) {
    for i in (0..n).rev() {
        let mut s = sample_at(((n - i) as f32 * 0.4).sin().abs() * 90.0, i);
        s.iowait = Some(((n - i) as f32 * 0.13).sin().abs() * 70.0);
        s.running = Some(2);
        s.blocked = Some(9);
        app.push(s);
    }
}

/// The series names the timeline gutter is currently showing, top to bottom.
fn timeline_series(app: &App, w: u16, h: u16) -> Vec<String> {
    let rows = ui::timeline_rows_range(h);
    render_lines(app, w, h)[rows.start as usize..rows.end as usize]
        .iter()
        .map(|l| {
            l.chars()
                .take(ui::GUTTER_W)
                .collect::<String>()
                .trim()
                .to_string()
        })
        .filter(|g| matches!(g.as_str(), "CPU" | "WAIT" | "MEM"))
        .collect()
}

#[test]
fn a_short_terminal_keeps_the_two_series_that_answer_the_question() {
    // Memory over ten minutes is a flat line or a slow ramp that repeats what
    // the header says. Waiting is the row that turns a stalled machine from a
    // mystery into a shape, so it is memory that yields.
    let mut app = App::new(600);
    stalled_history(&mut app, 200);
    assert_eq!(timeline_series(&app, 64, 24), ["CPU", "WAIT"]);
}

#[test]
fn a_tall_terminal_gets_memory_back() {
    let mut app = App::new(600);
    stalled_history(&mut app, 200);
    assert_eq!(timeline_series(&app, 64, 50), ["CPU", "WAIT", "MEM"]);
}

#[test]
fn a_platform_without_iowait_gives_the_row_back_to_memory() {
    // macOS publishes no iowait. The graph carries memory rather than an empty
    // row captioned with a figure the platform cannot produce.
    let mut app = App::new(600);
    for i in (0..200).rev() {
        app.push(sample_at(50.0, i));
    }
    let series = timeline_series(&app, 64, 24);
    assert!(!series.contains(&"WAIT".to_string()), "{series:?}");
    assert_eq!(series, ["CPU", "MEM"]);
}

#[test]
fn every_graph_row_starts_at_the_same_column() {
    // The whole reason to stack them is to read one against another, and a
    // gutter that is one column wider on one row makes the same instant sit in
    // different places. `WAIT` is four characters and the gutter reserved
    // three, so the format width — a minimum, not a maximum — silently let it
    // through.
    let mut app = App::new(600);
    stalled_history(&mut app, 200);
    let rows = ui::timeline_rows_range(50);
    let lines = render_lines(&app, 64, 50);
    let graph_rows: Vec<&String> = lines[rows.start as usize..rows.end as usize]
        .iter()
        .filter(|l| l.chars().any(|c| ('\u{2800}'..='\u{28ff}').contains(&c)))
        .collect();
    assert!(graph_rows.len() >= 6, "expected a stack of graphs");
    for line in &graph_rows {
        let first = line
            .char_indices()
            .find(|(_, c)| ('\u{2800}'..='\u{28ff}').contains(c))
            .map(|(i, _)| line[..i].chars().count());
        assert_eq!(
            first,
            Some(ui::GUTTER_W),
            "a graph row began at a different column: {line:?}"
        );
    }
}

#[test]
fn the_legend_names_whichever_series_are_on_screen() {
    // A fixed "cpu · mem" was wrong the moment the series became a decision.
    let mut app = App::new(600);
    stalled_history(&mut app, 200);
    // Narrow enough that the gutter is dropped, so the legend has to do the
    // naming instead.
    let all = render(&app, 28, 24);
    assert!(
        all.contains("cpu · wait"),
        "the legend does not name the rows"
    );
    assert!(
        !all.contains("cpu · mem"),
        "the legend names a row that is not shown"
    );
}

#[test]
fn the_gutter_is_exactly_its_width_whatever_it_is_given() {
    // A format width is a *minimum*. `WAIT` is four characters against three
    // reserved, so it widened its own row and shifted that graph sideways
    // relative to the ones above it — two graphs whose columns are not the
    // same instant are worse than one graph. The guard has to hold for a name
    // nobody has added yet, which is the only reason it is worth having.
    for name in [None, Some("CPU"), Some("WAIT"), Some("NETWORK")] {
        for row in 0..4 {
            let g = ui::axis_label_for_test(row, 4, name);
            assert_eq!(
                g.chars().count(),
                ui::GUTTER_W,
                "row {row} with {name:?} rendered {g:?}"
            );
        }
    }
}

#[test]
fn the_caption_names_the_rows_that_are_on_screen_while_scrubbing() {
    // A caption that identifies a series the graph is not drawing disagrees
    // with the thing it labels. It said `MEM` while the graph showed `WAIT`.
    //
    // At a width where the gutter can label the rows the caption does not
    // identify them at all — that is the ladder working, not a bug — so this is
    // checked at the width where the caption is the only thing naming them. And
    // while scrubbing, because that is the mode where this row used to be
    // replaced wholesale and the identification went with it.
    let caption = |app: &App, h: u16| {
        let r = ui::timeline_rows_range(h);
        render_lines(app, 24, h)[r.start as usize..r.end as usize]
            .iter()
            .find(|l| l.contains("cpu"))
            .cloned()
            .unwrap_or_default()
    };

    let mut app = App::new(600);
    stalled_history(&mut app, 200);
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);
    app.history.scrub(-10);

    // Short: only CPU has room, so the caption must not claim anything else.
    let short = caption(&app, 14);
    assert!(short.contains("cpu"), "{short:?}");
    for absent in ["wait", "mem"] {
        assert!(
            !short.contains(absent),
            "the caption names {absent}, which is not drawn at this height: {short:?}"
        );
    }

    // Taller: stall pressure gains a row, and the caption picks it up.
    let taller = caption(&app, 20);
    assert!(
        taller.contains("cpu") && taller.contains("wait"),
        "the caption omits a row that is drawn: {taller:?}"
    );
    assert!(
        !taller.contains("mem"),
        "the caption names a row that is not drawn: {taller:?}"
    );
}

#[test]
fn a_panel_with_no_gutter_does_not_stack_three_unnamed_graphs() {
    // Without a gutter the legend does the naming, in one flat list the reader
    // has to map onto the stack by position. Workable for two rows, guesswork
    // for three — and since the hues alternate, a third graph shares the first
    // one's colour, so position is the *only* thing telling them apart.
    let tall = 20;
    assert_eq!(
        ui::sections(tall, 3, ui::GUTTER_W).len(),
        3,
        "the height is not the constraint being tested"
    );
    assert_eq!(
        ui::sections(tall, 3, 0).len(),
        2,
        "three graphs were stacked with only a flat legend to name them"
    );

    // And it is the clamp, not a floor: two candidates still give two.
    assert_eq!(ui::sections(tall, 2, 0).len(), 2);
}

#[test]
fn the_legend_stops_at_a_phrase_boundary_at_every_width() {
    // The identification grew from a fixed `cpu · mem` to as much as
    // `cpu · wait · mem`, which pushed the old two-tier legend past a narrow
    // panel and let the terminal cut `1s/slot` in half — precisely what
    // dropping the key hints was supposed to prevent.
    //
    // Asserted as "ends where a tier ends" rather than by naming the tiers,
    // so the property survives the wording changing.
    let mut app = App::new(600);
    stalled_history(&mut app, 200);
    for w in 20..=90u16 {
        let rows = ui::timeline_rows_range(30);
        let legend = render_lines(&app, w, 30)[rows.start as usize..rows.end as usize]
            .iter()
            .find(|l| l.contains("shown") || l.contains(" · "))
            .cloned()
            .unwrap_or_default();
        // The caption shares its row with the `past`/`now` anchors now, so the
        // phrase to check is what sits between them rather than the whole line.
        let trimmed = legend
            .trim_end()
            .trim_end_matches("now")
            .trim_end()
            .trim_start()
            .trim_start_matches("past")
            .trim();
        if trimmed.is_empty() {
            continue;
        }
        assert!(
            trimmed.chars().count() <= w as usize,
            "the legend overflowed at w={w}: {trimmed:?}"
        );
        let ends_well = ["zoom", "/slot", "shown", "cpu", "wait", "mem", "missing"]
            .iter()
            .any(|s| trimmed.ends_with(s));
        assert!(
            ends_well,
            "the legend was cut mid-phrase at w={w}: {trimmed:?}"
        );
    }
}

#[test]
fn memory_is_shown_as_a_composition_not_just_a_level() {
    // "37% used" reads identically on a box with eight gigabytes free and on
    // one whose only headroom is page cache it is about to have to drop. The
    // level is the same and the situation is not.
    let mut roomy = App::new(60);
    let mut s = sample(5.0);
    s.mem = MemStat {
        total: 16 << 30,
        used: 6 << 30,
        available: 10 << 30,
        free: Some(10 << 30), // all headroom is genuinely free
        swap_total: 0,
        swap_used: 0,
    };
    roomy.push(s.clone());

    let mut cached = App::new(60);
    s.mem.free = Some(1 << 30); // …the same level, but the headroom is cache
    cached.push(s);

    let bar = |app: &App| {
        figures_line(app, 120, 24)
            .chars()
            .filter(|c| "█▒░".contains(*c))
            .collect::<String>()
    };
    assert_eq!(
        bar(&roomy).chars().count(),
        12,
        "the bar is not twelve columns"
    );
    assert_ne!(
        bar(&roomy),
        bar(&cached),
        "two very different machines drew the same memory bar"
    );
    assert!(bar(&cached).contains('▒'), "the cache segment is missing");
}

#[test]
fn the_memory_bar_separates_by_glyph_so_it_survives_monochrome() {
    // Every other meaning-bearing element here is legible without colour, and
    // a bar whose segments are only told apart by hue would be the exception.
    let mut app = App::new(60);
    let mut s = sample(5.0);
    s.mem = MemStat {
        total: 16 << 30,
        used: 6 << 30,
        available: 10 << 30,
        free: Some(4 << 30),
        swap_total: 0,
        swap_used: 0,
    };
    app.push(s);
    app.theme = Theme::new(Palette::Safe, Tier::Mono);

    let line = figures_line(&app, 120, 24).clone();
    for glyph in ['█', '▒', '░'] {
        assert!(
            line.contains(glyph),
            "the {glyph} segment vanished at the mono tier: {line}"
        );
    }
}

#[test]
fn the_memory_bar_is_always_exactly_its_width() {
    // A bar one column short of its box reads as a rendering fault, and one
    // column long pushes every figure after it sideways.
    let shapes = [
        (16u64 << 30, 0u64, 16u64 << 30),
        (16 << 30, 16 << 30, 0),
        (16 << 30, 1, 16 << 30),
        (16 << 30, 15 << 30, 1 << 30),
        (0, 0, 0),
    ];
    for (total, used, available) in shapes {
        let mut app = App::new(60);
        let mut s = sample(5.0);
        s.mem = MemStat {
            total,
            used,
            available,
            free: Some(available / 2),
            swap_total: 0,
            swap_used: 0,
        };
        app.push(s);
        let n = figures_line(&app, 120, 24)
            .chars()
            .filter(|c| "█▒░".contains(*c))
            .count();
        let want = usize::from(total > 0) * 12;
        assert_eq!(n, want, "total={total} used={used} avail={available}");
    }
}

#[test]
fn a_platform_that_cannot_partition_memory_draws_two_parts_not_three() {
    // macOS `used` and `available` come from overlapping vm_stat quantities and
    // routinely sum to more than the machine has — 20.0G used plus 11.5G
    // available on a 24G box. There is no cache/free split to draw there, and
    // deriving one reports "no free memory, all headroom is reclaimable cache"
    // on a healthy machine: the exact alarming misreading this figure exists to
    // prevent.
    let mut app = App::new(60);
    let mut s = sample(5.0);
    s.mem = MemStat {
        total: 24 << 30,
        used: 18 << 30,
        available: 11 << 30,
        free: None,
        swap_total: 0,
        swap_used: 0,
    };
    app.push(s);
    let bar: String = figures_line(&app, 120, 24)
        .chars()
        .filter(|c| "█▒░".contains(*c))
        .collect();
    assert_eq!(bar.chars().count(), 12);
    assert!(!bar.contains('▒'), "a cache segment was invented: {bar}");
    // …and the two parts still agree with the figure beside them: 18/24 is 9
    // of 12.
    assert_eq!(bar.chars().filter(|c| *c == '█').count(), 9, "{bar}");
}

/// A sample where `denied` of `n` processes had unreadable IO.
fn with_denied(n: usize, denied: usize) -> Sample {
    let mut s = sample(5.0);
    s.procs = (0..n)
        .map(|i| proc_named(i as i32, "worker", 1.0, 1 << 20))
        .collect();
    s.io_collected = true;
    s.io_denied = denied;
    s
}

#[test]
fn io_columns_stay_where_most_of_the_table_can_be_read() {
    // The header may have just said the machine is blocked on IO, and the table
    // is where the culprit is named. A default that hides it makes the default
    // view unable to answer the question the default view raised.
    let mut app = App::new(60);
    app.probe_io(&with_denied(100, 3));
    assert!(app.show_io, "the columns were withdrawn on a readable box");
    assert!(
        app.needs().asked(crate::collect::Source::Io),
        "collection stopped on a readable box"
    );
}

#[test]
fn io_columns_withdraw_where_they_would_be_a_wall_of_dashes() {
    // `/proc/<pid>/io` needs CAP_SYS_PTRACE for other users' processes. On a
    // box running its services as root while you are not, the columns cost
    // width and invite the reading that those processes are doing no IO.
    let mut app = App::new(60);
    app.probe_io(&with_denied(100, 90));
    assert!(!app.show_io, "a wall of em dashes was shown by default");
    // …and collection stops too, rather than paying half a millisecond a
    // sample for a column nobody can read.
    assert!(
        !app.needs().asked(crate::collect::Source::Io),
        "collection continued for an unreadable column"
    );
}

#[test]
fn the_probe_does_not_fire_on_an_empty_sample() {
    // The first sample on a machine poptop cannot read at all would otherwise
    // divide by zero, or decide from nothing.
    let mut app = App::new(60);
    let mut empty = sample(5.0);
    empty.procs.clear();
    app.probe_io(&empty);
    assert!(app.show_io);
}

#[test]
fn the_key_still_overrides_whatever_the_probe_decided() {
    // It is a default, not a policy. Someone with partial access may well want
    // the column for the processes they can see.
    let mut app = App::new(60);
    app.probe_io(&with_denied(100, 90));
    assert!(!app.show_io);
    app.toggle_io();
    assert!(app.show_io, "the key could not bring the columns back");
    assert!(
        app.needs().asked(crate::collect::Source::Io),
        "the key did not restart collection"
    );
}

#[test]
fn the_io_columns_drop_rather_than_squeezing_the_table() {
    // Every column is a fixed `Length`, so ratatui squeezes them all when they
    // do not fit rather than dropping any. Before the columns became a
    // default nobody hit that; afterwards, an eighty-column terminal rendered
    // truncated figures under a `RSS` header reading `512.`.
    let mut app = App::new(60);
    let mut s = sample(10.0);
    s.io_collected = true;
    s.procs[0].io = Some(crate::sample::IoRates {
        read: 2048,
        write: 4096,
    });
    app.push(s);
    assert!(app.show_io, "the fixture is not testing what it claims");

    let wide = render(&app, 120, 30);
    assert!(wide.contains("DISK R") && wide.contains("2.0K/s"));

    // Narrow: the columns go, and nothing else is truncated to make room.
    let narrow = render(&app, 78, 30);
    assert!(
        !narrow.contains("DISK"),
        "the columns squeezed the table instead"
    );
    assert!(
        narrow.contains("512.0M"),
        "a figure was truncated to fit the IO columns"
    );

    // Every width in between is either clean or without the columns; nothing
    // in the table is ever cut mid-figure.
    for w in 60..=130u16 {
        let out = render(&app, w, 30);
        assert!(
            !out.contains("DISK") || out.contains("512.0M"),
            "at w={w} the IO columns were kept at the cost of the table"
        );
    }
}

#[test]
// Linux only, because a kernel thread is a Linux notion and
// `is_kernel_thread` now says so — pid 2 is `kthreadd` there and either absent
// or an ordinary process here, and answering `true` for it on macOS would drop
// a real row from the table and a real process from this ratio. So the
// scenario below cannot arise on a Mac, and building it out of pids that mean
// nothing on this platform would assert about nothing.
#[cfg(target_os = "linux")]
fn kernel_threads_do_not_trigger_the_io_probe() {
    // They are root-owned and unreadable to an ordinary user, and on a
    // many-core box they outnumber the real processes — so counting them would
    // withdraw the columns on exactly the laptop the probe exists to protect.
    let mut app = App::new(60);
    let mut s = sample(5.0);
    s.io_collected = true;
    // Two readable programs, and a crowd of kworkers under kthreadd.
    s.procs = vec![
        proc_named(100, "nginx", 1.0, 0),
        proc_named(101, "psql", 1.0, 0),
    ];
    for pid in 200..260 {
        let mut k = proc_named(pid, "kworker/0:1", 0.0, 0);
        k.ppid = 2;
        s.procs.push(k);
    }
    s.io_denied = 0; // the collector skips them, so none are counted

    app.probe_io(&s);
    assert!(
        app.show_io,
        "sixty kernel threads withdrew the columns on a readable box"
    );

    // …and they must not hide real denial either. Two programs, both
    // unreadable, is a box where the column is useless — whatever crowd of
    // kernel threads happens to be standing next to them.
    let mut app = App::new(60);
    s.io_denied = 2;
    app.probe_io(&s);
    assert!(
        !app.show_io,
        "sixty kernel threads diluted a fully unreadable box into a passing one"
    );
}

#[test]
fn the_denied_ratio_is_not_measured_against_the_filtered_rows() {
    // `90/2 need root` is not a ratio of anything. The two numbers came from
    // different populations the moment a filter was active.
    let mut app = App::new(60);
    let mut s = sample(5.0);
    s.io_collected = true;
    // From 100, because pid 2 is kthreadd and would rightly be excluded from
    // the denominator — which is a different fact from the one under test.
    s.procs = (100..110)
        .map(|i| proc_named(i, if i == 100 { "nginx" } else { "other" }, 1.0, 0))
        .collect();
    s.io_denied = 6;
    app.push(s);
    app.filter = "nginx".into();

    let out = render(&app, 130, 30);
    assert!(out.contains("6/10 need root"), "{out}");
}

#[test]
fn the_key_says_why_it_did_nothing_on_a_narrow_panel() {
    // Otherwise `i` is a silent no-op: the columns do not appear, nothing says
    // why, and the obvious conclusion is that the feature is broken.
    let mut app = App::new(60);
    let mut s = sample(5.0);
    s.io_collected = true;
    app.push(s);
    assert!(app.show_io);

    let narrow = render(&app, 78, 30);
    assert!(!narrow.contains("DISK"), "the columns fit after all");
    assert!(
        narrow.contains("too narrow"),
        "the panel does not say why: {narrow}"
    );

    // …and says nothing when the columns were not asked for.
    app.toggle_io();
    assert!(!render(&app, 78, 30).contains("too narrow"));
}

/// The measurements behind the metric-accuracy audit (items 0023-0027).
///
/// Kept because the findings are claims about live numbers, and a claim about a
/// live number is only as good as the last time someone ran it.
#[test]
#[ignore]
fn show_metric_audit() {
    use crate::collect::{Collector, Needs, Platform};
    let mut c = Platform::new().unwrap();
    c.sample(Needs::NONE).unwrap();

    // Does a fast sample rate still produce sane CPU? 0013 allows 50ms, and
    // sysinfo documents a 200ms minimum between CPU refreshes.
    for ms in [50u64, 100, 200, 1000] {
        std::thread::sleep(std::time::Duration::from_millis(ms));
        let s = c.sample(Needs::NONE).unwrap();
        println!(
            "interval {ms:>4}ms -> cpu_total {:>6.1}%  max core {:>6.1}%  max proc {:>7.1}%",
            s.cpu_total,
            s.cpu_per_core.iter().cloned().fold(0.0f32, f32::max),
            s.procs.iter().map(|p| p.cpu).fold(0.0f32, f32::max),
        );
    }

    let s = c.sample(Needs::NONE).unwrap();
    let cores = s.cpu_per_core.len();
    let no_start = s.procs.iter().filter(|p| p.started.is_none()).count();
    let over = s
        .procs
        .iter()
        .filter(|p| p.cpu > cores as f32 * 100.0)
        .count();
    println!(
        "procs {}  no start time {}  cpu over cores*100 {}  cores {}",
        s.procs.len(),
        no_start,
        over,
        cores
    );
    let biggest = s.procs.iter().max_by_key(|p| p.rss).unwrap();
    println!("largest rss: {} = {} bytes", biggest.name, biggest.rss);
}

#[test]
fn the_first_sample_reports_no_cpu_rather_than_a_wrong_one() {
    // Both backends need a previous reading before CPU means anything, and
    // neither gets one on the first sample. `/proc` has said zero all along;
    // sysinfo was refreshed by `System::new_all` microseconds earlier and
    // returned a figure from inside the very interval the config floor exists
    // to refuse — drawn as the first frame, pushed into history, and persisted.
    use crate::collect::{Collector, Needs, Platform};
    let mut c = Platform::new().unwrap();
    let first = c.sample(Needs::NONE).unwrap();

    assert_eq!(first.cpu_total, 0.0, "the first sample claims a CPU figure");
    assert!(
        first.cpu_per_core.iter().all(|&c| c == 0.0),
        "a core claims a figure on the first sample"
    );
    assert!(
        first.procs.iter().all(|p| p.cpu == 0.0),
        "a process claims a figure on the first sample"
    );
    // …and the machine is still described: this is about CPU, not about
    // refusing to sample.
    assert!(
        first.mem.total > 0,
        "the first sample carries nothing at all"
    );
}

#[test]
fn a_kernel_with_no_io_accounting_withdraws_the_columns() {
    // CONFIG_TASK_IO_ACCOUNTING is optional and some hardened runtimes hide the
    // file. Every read then fails with NotFound — correctly not a permission
    // problem, and so counted towards nothing, so the ratio probe never fires.
    // The columns would sit on screen permanently empty while the collector
    // kept paying for them.
    let mut app = App::new(60);
    let mut s = with_denied(100, 0);
    s.io_supported = false;
    app.probe_io(&s);
    assert!(
        !app.show_io,
        "empty columns were kept on an unsupporting kernel"
    );
    assert!(
        !app.needs().asked(crate::collect::Source::Io),
        "collection continued for a file that does not exist"
    );
}

#[test]
fn an_unsupporting_kernel_is_described_differently_from_a_locked_down_one() {
    // Nothing the user does will make this appear, so "needs root" would send
    // them somewhere pointless.
    let mut app = App::new(60);
    let mut s = with_denied(10, 0);
    s.io_supported = false;
    app.push(s);
    // Shown explicitly, since the probe would otherwise have hidden it.
    app.show_io = true;
    let out = render(&app, 130, 30);
    assert!(out.contains("keeps no per-process accounting"), "{out}");
    assert!(
        !out.contains("need root"),
        "sent the user after privileges: {out}"
    );
}

#[test]
fn the_table_names_a_process_by_its_command_line() {
    // The item: four rows reading `node` say nothing about any of them. With
    // wide-enough columns the arguments are what the row is about.
    let mut app = App::new(60);
    let mut s = sample(10.0);
    let cmds = [
        "node /srv/api/server.js --port 3000",
        "node /srv/web/bundler.js --watch",
        "node /srv/api/worker.js",
    ];
    s.procs = cmds
        .iter()
        .enumerate()
        .map(|(i, c)| ProcSample {
            cpu: 30.0 - i as f32,
            cmd: Some(std::sync::Arc::from(*c)),
            ..proc_named(i as i32 + 10, "node", 0.0, 1 << 20)
        })
        .collect();
    app.push(s);
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);

    let body: Vec<String> = rows(&app, 200, 20)
        .into_iter()
        .filter(|l| l.contains("node"))
        .collect();
    assert_eq!(body.len(), 3, "expected three rows: {body:?}");
    for (row, cmd) in body.iter().zip(cmds) {
        assert!(row.contains(cmd), "{row:?} does not name {cmd:?}");
    }
}

#[test]
fn a_process_with_no_command_line_is_named_by_its_comm() {
    // A kernel thread has none, and `[kworker/3:1]` is a real name — where a
    // blank would be a row that says nothing at all.
    let mut app = App::new(60);
    let mut s = sample(10.0);
    s.procs = vec![ProcSample {
        cmd: None,
        ..proc_named(9, "[kworker/3:1]", 5.0, 1 << 20)
    }];
    app.push(s);
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);
    assert!(
        rows(&app, 200, 20)
            .iter()
            .any(|l| l.contains("[kworker/3:1]")),
        "the row lost its name"
    );
}

#[test]
fn the_filter_searches_the_command_line() {
    // The question people arrive with is "which of these is the API server",
    // and the answer is in the arguments.
    let mut app = App::new(60);
    let mut s = sample(10.0);
    s.procs = ["node /srv/api/server.js", "node /srv/web/bundler.js"]
        .iter()
        .enumerate()
        .map(|(i, c)| ProcSample {
            cpu: 30.0 - i as f32,
            cmd: Some(std::sync::Arc::from(*c)),
            ..proc_named(i as i32 + 10, "node", 0.0, 1 << 20)
        })
        .collect();
    app.push(s);
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);

    app.filter = "bundler".into();
    let body: Vec<String> = rows(&app, 200, 20)
        .into_iter()
        .filter(|l| l.contains("node"))
        .collect();
    assert_eq!(body.len(), 1, "expected one match: {body:?}");
    assert!(body[0].contains("bundler.js"), "{:?}", body[0]);
}

#[test]
fn sorting_by_name_orders_by_what_the_column_shows() {
    // The column renders `command()`; sorting on `name` produced four identical
    // `node`s and a column that looked unsorted — for exactly the processes the
    // command line was added to tell apart.
    let mut procs: Vec<ProcSample> = [
        "node /srv/web/bundler.js",
        "node /srv/api/server.js",
        "node /srv/api/worker.js",
    ]
    .iter()
    .enumerate()
    .map(|(i, c)| ProcSample {
        cmd: Some(std::sync::Arc::from(*c)),
        ..proc_named(i as i32 + 10, "node", 1.0, 0)
    })
    .collect();
    procs.sort_by(|a, b| crate::app::Sort::Name.compare(a, b));
    let got: Vec<&str> = procs.iter().map(|p| p.command()).collect();
    assert_eq!(
        got,
        vec![
            "node /srv/api/server.js",
            "node /srv/api/worker.js",
            "node /srv/web/bundler.js",
        ]
    );
}

/// Kernel threads for a fixture: `kthreadd` itself and a crowd of workers under
/// it, matching what [`ProcSample::is_kernel_thread`] recognises.
#[cfg(target_os = "linux")]
fn with_kernel_threads(s: &mut Sample, n: i32) {
    let mut kthreadd = proc_named(2, "kthreadd", 0.0, 0);
    kthreadd.ppid = 0;
    s.procs.push(kthreadd);
    for i in 0..n {
        let mut k = proc_named(1000 + i, &format!("kworker/{i}:1"), 0.0, 0);
        k.ppid = 2;
        s.procs.push(k);
    }
}

// Linux only: a kernel thread is a Linux notion, `is_kernel_thread` says so,
// and on macOS these fixtures are ordinary processes that are never hidden.
#[cfg(target_os = "linux")]
#[test]
fn kernel_threads_are_hidden_and_a_key_shows_them() {
    // On a many-core box they outnumber the real processes several times over,
    // and none of them is what anyone opened a monitor to find.
    let mut app = App::new(60);
    let mut s = sample(10.0);
    s.procs = vec![
        proc_named(101, "nginx", 9.0, 1 << 20),
        proc_named(102, "postgres", 8.0, 1 << 20),
    ];
    with_kernel_threads(&mut s, 60);
    app.push(s);
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);

    let named = |app: &App| -> Vec<String> {
        rows(app, 120, 40)
            .into_iter()
            .filter(|l| l.contains("kworker") || l.contains("kthreadd") || l.contains("nginx"))
            .collect()
    };

    let hidden = named(&app);
    assert!(
        hidden.iter().any(|l| l.contains("nginx")),
        "the real process went missing: {hidden:?}"
    );
    assert!(
        !hidden
            .iter()
            .any(|l| l.contains("kworker") || l.contains("kthreadd")),
        "a kernel thread was drawn: {hidden:?}"
    );

    app.show_kernel = true;
    let shown = named(&app);
    assert!(
        shown.iter().any(|l| l.contains("kworker")),
        "the key showed nothing: {shown:?}"
    );
    assert!(
        shown.iter().any(|l| l.contains("kthreadd")),
        "kthreadd itself stayed hidden: {shown:?}"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn the_number_of_hidden_kernel_threads_is_stated() {
    // Every other omission in poptop states itself — an idle interface, a
    // device that has done no IO. A table quietly sixty rows shorter than the
    // process count beside it would be the one that did not.
    let mut app = App::new(60);
    let mut s = sample(10.0);
    s.procs = vec![proc_named(101, "nginx", 9.0, 1 << 20)];
    with_kernel_threads(&mut s, 60);
    app.push(s);
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);

    // Sixty workers plus kthreadd.
    assert_eq!(app.hidden_kernel_threads(), 61);
    let frame = rows(&app, 120, 40).join("\n");
    assert!(
        frame.contains("61 kernel hidden"),
        "the omission is silent: {:?}",
        frame.lines().next()
    );

    app.show_kernel = true;
    assert_eq!(app.hidden_kernel_threads(), 0);
    assert!(
        !rows(&app, 120, 40).join("\n").contains("kernel hidden"),
        "nothing is hidden and it still says so"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn hiding_kernel_threads_does_not_orphan_their_children_in_the_tree() {
    // `kthreadd` is the ancestor of every kernel thread, so a tree that hides
    // it must not keep it alive as somebody's visible ancestor — nor drop a
    // real process that happens to descend from a hidden one.
    let mut app = App::new(60);
    let mut s = sample(10.0);
    s.procs = vec![proc_named(101, "nginx", 9.0, 1 << 20)];
    with_kernel_threads(&mut s, 3);
    app.push(s);
    app.tree = true;
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);

    let frame = rows(&app, 120, 40).join("\n");
    assert!(frame.contains("nginx"), "the real process went missing");
    assert!(!frame.contains("kthreadd"), "the hidden ancestor was drawn");
    assert!(!frame.contains("kworker"), "a hidden child was drawn");
}

#[cfg(target_os = "linux")]
#[test]
fn a_filter_does_not_bring_hidden_kernel_threads_back() {
    // Filtering narrows what is shown; it does not overrule what is withheld.
    // Searching for `kworker` with them hidden should find nothing, not
    // everything.
    let mut app = App::new(60);
    let mut s = sample(10.0);
    s.procs = vec![proc_named(101, "nginx", 9.0, 1 << 20)];
    with_kernel_threads(&mut s, 6);
    app.push(s);
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);

    for tree in [false, true] {
        app.tree = tree;
        app.filter = "kworker".into();
        assert_eq!(
            app.visible_rows().len(),
            0,
            "a filter resurrected hidden kernel threads (tree: {tree})"
        );
        app.show_kernel = true;
        assert_eq!(
            app.visible_rows()
                .iter()
                .filter(|r| !r.context_only)
                .count(),
            6,
            "the same filter with them shown found the wrong number (tree: {tree})"
        );
        app.show_kernel = false;
    }
}

#[cfg(target_os = "linux")]
#[test]
fn hiding_kernel_threads_does_not_silently_select_another_process() {
    // Toggling them off takes the watched row away. There is no index to
    // strand any more — the question is whether the panel says the process is
    // gone or quietly highlights whatever is at that position.
    let mut app = App::new(60);
    let mut s = sample(10.0);
    s.procs = vec![proc_named(101, "nginx", 9.0, 1 << 20)];
    with_kernel_threads(&mut s, 40);
    app.push(s);
    app.show_kernel = true;
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);

    select_until(&mut app, "a kworker", |n| n.starts_with("kworker"));
    let watched = app.selected.clone().expect("nothing selected");
    assert!(watched.name().starts_with("kworker"), "{watched:?}");

    app.show_kernel = false;
    let rows = app.visible_rows();
    assert_eq!(
        app.selected.as_ref(),
        Some(&watched),
        "the selection jumped to a different process"
    );
    assert!(
        app.row_of(&rows).is_none(),
        "a hidden process is still being highlighted"
    );
    // Hidden is not gone. The kworker is still in the sample and still running,
    // so announcing `kworker/3:1 not running here` would be a claim that is
    // false about the machine — and the reader pressed `K`, so nothing needs
    // saying.
    assert!(
        app.watched_but_absent(&rows).is_none(),
        "a hidden but running process was reported as stopped"
    );
    assert!(
        !render(&app, 120, 30).contains("not running here"),
        "the panel says a running process stopped"
    );

    // …and showing them again brings the selection back.
    app.show_kernel = true;
    let rows = app.visible_rows();
    let i = app.row_of(&rows).expect("the selection did not come back");
    assert_eq!(rows[i].proc.command(), &**watched.name());
}

// macOS only: this asserts the *absence* of the Linux rule, which on Linux is
// the rule.
#[cfg(not(target_os = "linux"))]
#[test]
fn pid_two_is_an_ordinary_process_off_linux() {
    // `kthreadd` is pid 2 on Linux and nowhere else. Applying that rule here
    // would hide a real row from the table by default, and drop a real process
    // from the IO ratio — on the strength of a number that means nothing on
    // this platform.
    let mut app = App::new(60);
    let mut s = sample(10.0);
    let mut two = proc_named(2, "a-real-process", 9.0, 1 << 20);
    two.ppid = 1;
    let mut child = proc_named(3, "its-child", 8.0, 1 << 20);
    child.ppid = 2;
    s.procs = vec![two, child];
    app.push(s);
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);

    assert_eq!(
        app.hidden_kernel_threads(),
        0,
        "a real process was counted as a kernel thread"
    );
    let frame = rows(&app, 120, 30).join("\n");
    for name in ["a-real-process", "its-child"] {
        assert!(
            frame.contains(name),
            "{name} was hidden on a platform with no kthreadd"
        );
    }
}

/// A sample whose processes all belong to one user, pushed enough times to
/// clear the constancy window.
fn settled_single_user(app: &mut App, names: &[&str]) {
    for _ in 0..App::CONSTANT_FOR {
        let mut s = sample(10.0);
        s.procs = names
            .iter()
            .enumerate()
            .map(|(i, n)| proc_named(i as i32 + 101, n, 20.0 - i as f32, 1 << 20))
            .collect();
        app.push(s);
    }
}

#[test]
fn a_column_of_one_repeated_value_gives_its_width_to_the_command() {
    // Measured in the item: `USER` cost ten columns — more than `CPU%` — to
    // repeat one word twelve times, while `COMMAND` differed on every row and
    // had to elide.
    let mut app = App::new(60);
    settled_single_user(&mut app, &["postgres", "nginx", "redis"]);
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);

    let one = rows(&app, 120, 20);
    let head = one.iter().find(|l| l.contains("PID")).unwrap();
    assert!(!head.contains("USER"), "the column stayed: {head:?}");
    let title = one.iter().find(|l| l.contains("processes")).unwrap();
    assert!(
        title.contains("· all root"),
        "what the column said was not said anywhere: {title:?}"
    );

    // The width goes to COMMAND, which is the point.
    let wide = ui::command_width_for_test(120, false, false);
    let narrow = ui::command_width_for_test(120, false, true);
    assert_eq!(
        wide - narrow,
        11,
        "the column's width was not handed over: {narrow} -> {wide}"
    );
}

#[test]
fn a_second_user_keeps_the_column() {
    let mut app = App::new(60);
    settled_single_user(&mut app, &["postgres", "nginx"]);
    let mut s = sample(10.0);
    s.procs = vec![
        proc_named(101, "postgres", 20.0, 1 << 20),
        ProcSample {
            // Eight characters, so it fits the ten-column USER field. The first
            // draft used a twelve-character name and asserted on the whole of
            // it, which the column truncates — the test failed against correct
            // output.
            user: std::sync::Arc::from("operator"),
            ..proc_named(102, "nginx", 19.0, 1 << 20)
        },
    ];
    app.push(s);
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);

    let frame = rows(&app, 120, 20);
    let head = frame.iter().find(|l| l.contains("PID")).unwrap();
    assert!(
        head.contains("USER"),
        "the column was folded away: {head:?}"
    );
    assert!(
        frame.iter().any(|l| l.contains("operator")),
        "the second user is not on screen anywhere"
    );
    assert!(
        !frame.iter().any(|l| l.contains("\u{b7} all ")),
        "the title claimed one user over a sample with two"
    );
}

#[test]
fn the_layout_does_not_oscillate_as_a_process_comes_and_goes() {
    // The failure this guards against: one short-lived `root` process takes the
    // column away and gives it back a second later, and the whole table shifts
    // ten columns sideways twice. Expanding is immediate — never hide a fact —
    // but collapsing waits out the window.
    let mut app = App::new(60);
    settled_single_user(&mut app, &["postgres", "nginx"]);
    let has_user = |app: &App| {
        rows(app, 120, 20)
            .iter()
            .any(|l| l.contains("PID") && l.contains("USER"))
    };
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);
    assert!(!has_user(&app), "settled, and the column is still there");

    // A second user appears: the column comes back on that very frame.
    let mut s = sample(10.0);
    s.procs = vec![
        proc_named(101, "postgres", 20.0, 1 << 20),
        ProcSample {
            user: std::sync::Arc::from("someone-else"),
            ..proc_named(102, "a-visitor", 19.0, 1 << 20)
        },
    ];
    app.push(s);
    assert!(
        has_user(&app),
        "a second user did not bring the column back"
    );

    // It leaves again. The column must not vanish on the next frame — that is
    // the flicker.
    let mut s = sample(10.0);
    s.procs = vec![proc_named(101, "postgres", 20.0, 1 << 20)];
    app.push(s.clone());
    assert!(
        has_user(&app),
        "the column vanished one frame after the visitor left"
    );

    // …and it does come back, once the window is quiet.
    for _ in 0..App::CONSTANT_FOR {
        app.push(s.clone());
    }
    assert!(!has_user(&app), "the column never yielded its width again");
}

#[test]
fn scrubbing_back_to_two_users_shows_the_column_again() {
    // The decision is read from the displayed sample and the ones before it,
    // not from live. A collapsed column while scrubbed back over a moment with
    // two users would put `· all root` above rows that were not all root.
    let mut app = App::new(60);
    let mut s = sample(10.0);
    s.procs = vec![
        proc_named(101, "postgres", 20.0, 1 << 20),
        ProcSample {
            user: std::sync::Arc::from("someone-else"),
            ..proc_named(102, "a-visitor", 19.0, 1 << 20)
        },
    ];
    app.push(s);
    settled_single_user(&mut app, &["postgres"]);
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);
    assert!(
        !rows(&app, 120, 20).iter().any(|l| l.contains("USER")),
        "live, one user, and the column is still drawn"
    );

    app.history.goto_oldest();
    let frame = rows(&app, 120, 20);
    assert!(
        frame
            .iter()
            .any(|l| l.contains("PID") && l.contains("USER")),
        "scrubbed back over two users and the column stayed folded"
    );
    assert!(
        !frame.iter().any(|l| l.contains("· all ")),
        "the title claimed one user over a sample with two"
    );
}

#[test]
fn the_footer_drops_whole_hints_rather_than_cutting_one() {
    // A clipped footer reads as a key called `filt`. Adding `K kernel` pushed
    // the line two columns past a hundred-column terminal and that is exactly
    // what appeared.
    for w in 20..=140u16 {
        let line = ui::fit_hints_for_test(w);
        assert!(
            line.chars().count() <= w as usize,
            "the footer overflowed at {w}: {line:?}"
        );
        for hint in line.split(" · ") {
            assert!(
                ui::KEY_HINTS.contains(&hint),
                "a hint was cut in half at {w}: {hint:?}"
            );
        }
    }
}

#[test]
fn the_footer_gives_up_the_least_useful_key_first() {
    // Order is the ladder. `/` is reached for constantly and `K` is the most
    // niche, so a narrow terminal must lose `K` and keep `/`.
    let at_100 = ui::fit_hints_for_test(100);
    assert!(at_100.contains("/ filter"), "{at_100:?}");
    assert!(!at_100.contains("K kernel"), "{at_100:?}");
    assert!(
        ui::fit_hints_for_test(140).contains("K kernel"),
        "a wide terminal lost a hint it had room for"
    );
}

#[test]
fn the_column_headers_name_the_columns_under_them() {
    // `Table` pairs header and body cells by index, and the two lists had
    // diverged: with the IO columns shown, `HISTORY` sat over DISK R, `DISK R`
    // over DISK W, and `DISK W` over the sparkline. Every one of the three
    // named the column beside it.
    let mut app = App::new(60);
    for _ in 0..App::CONSTANT_FOR {
        let mut s = sample(10.0);
        s.io_collected = true;
        s.procs = vec![ProcSample {
            io: Some(crate::sample::IoRates {
                read: 1 << 20,
                write: 1 << 21,
            }),
            ..proc_named(101, "postgres", 20.0, 1 << 20)
        }];
        app.push(s);
    }
    app.show_io = true;
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);

    let frame = rows(&app, 140, 16);
    let head = frame.iter().find(|l| l.contains("PID")).unwrap();
    let body = frame.iter().find(|l| l.contains("postgres")).unwrap();

    // The read rate renders as `1.0M/s` and the write as `2.0M/s`, so each
    // label must sit over the value it names. Compared by the column each ends
    // at, counted in chars — the sparkline glyphs are multi-byte.
    let col = |l: &str, pat: &str| {
        l.find(pat)
            .map(|b| l[..b].chars().count())
            .unwrap_or_else(|| panic!("{pat:?} not in {l:?}"))
    };
    let r_head = col(head, "DISK R") + "DISK R".len();
    let w_head = col(head, "DISK W") + "DISK W".len();
    let r_body = col(body, "1.0M/s") + "1.0M/s".len();
    let w_body = col(body, "2.0M/s") + "2.0M/s".len();
    assert_eq!(
        r_head, r_body,
        "DISK R does not sit over the read rate\n{head}\n{body}"
    );
    assert_eq!(
        w_head, w_body,
        "DISK W does not sit over the write rate\n{head}\n{body}"
    );
    // And the sparkline's header is past both of them, over the sparkline.
    assert!(
        col(head, "HIST ") > w_head,
        "the history column is still to the left of the disk columns\n{head}"
    );
    // Identity comes after every measurement, including the sparkline.
    assert!(
        col(head, "PID") > col(head, "HIST "),
        "identity is not at the end of the row\n{head}"
    );
    assert!(
        col(head, "COMMAND") > col(head, "PID"),
        "PID and COMMAND are not adjacent at the end\n{head}"
    );
}

#[test]
fn a_window_on_an_empty_history_is_empty_rather_than_a_panic() {
    // `cursor_index` saturates to zero on an empty buffer, so the range was
    // `0..1` against a deque of length zero. Every other accessor here is
    // empty-safe; this one was safe only because `main` happens to push a
    // sample before the first draw.
    let app = App::new(10);
    assert_eq!(app.history.window(5).count(), 0);
    assert_eq!(app.one_user(), None, "an empty history claimed a user");
    assert_eq!(app.hidden_kernel_threads(), 0);
}

#[cfg(target_os = "linux")]
#[test]
fn showing_kernel_threads_stops_the_title_claiming_one_user() {
    // With `K` pressed the table draws root-owned kworkers. A title reading
    // `· all alice` above them is a claim that is false about the rows
    // directly underneath it.
    let mut app = App::new(60);
    for _ in 0..App::CONSTANT_FOR {
        let mut s = sample(10.0);
        s.procs = vec![ProcSample {
            user: std::sync::Arc::from("alice"),
            ..proc_named(101, "postgres", 20.0, 1 << 20)
        }];
        with_kernel_threads(&mut s, 8);
        app.push(s);
    }
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);
    assert_eq!(
        app.one_user().as_deref(),
        Some("alice"),
        "hidden kernel threads should not count against the user"
    );

    app.show_kernel = true;
    assert_eq!(
        app.one_user(),
        None,
        "the kworkers are on screen and root, and the title still says one user"
    );
    let frame = rows(&app, 120, 30).join("\n");
    assert!(
        !frame.contains("· all "),
        "the title claims one user: {frame:?}"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn the_hidden_count_is_filtered_like_the_count_beside_it() {
    // `processes (1) · 250 kernel hidden` under a filter for `nginx` implies
    // two hundred and fifty rows were withheld from a list that had one
    // candidate.
    let mut app = App::new(60);
    let mut s = sample(10.0);
    s.procs = vec![proc_named(101, "nginx", 20.0, 1 << 20)];
    with_kernel_threads(&mut s, 20);
    app.push(s);

    assert_eq!(app.hidden_kernel_threads(), 21);
    app.filter = "nginx".into();
    assert_eq!(
        app.hidden_kernel_threads(),
        0,
        "kernel threads that the filter would have excluded anyway were counted as hidden"
    );
    app.filter = "kworker/3".into();
    assert_eq!(
        app.hidden_kernel_threads(),
        1,
        "the one match was not counted"
    );
}

#[test]
fn the_title_gives_up_whole_clauses_and_keeps_the_io_message() {
    // A clipped title reads as a message called `io: panel too narr`. The io
    // status is the one thing this panel guarantees — without it the `i` key
    // looks broken — so it outranks the sort label, the tree marker and the
    // axis.
    let mut app = App::new(60);
    for _ in 0..App::CONSTANT_FOR {
        let mut s = sample(10.0);
        s.io_collected = true;
        s.io_denied = 3;
        s.procs = (0..4)
            .map(|i| proc_named(101 + i, "postgres", 20.0 - i as f32, 1 << 20))
            .collect();
        app.push(s);
    }
    app.tree = true;
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);

    for w in 40..=160u16 {
        let frame = rows(&app, w, 20);
        let title = frame.iter().find(|l| l.contains("processes")).unwrap();
        let text = title.trim_end_matches(['─', ' ']);
        // Nothing is ever cut mid-clause.
        // Prefixes of clauses that are actually drawn. `"histor "` sat here
        // after the axis moved to the column header: a sentinel for a clause
        // that no longer exists can never match, so that quarter of the loop
        // was guaranteeing nothing.
        for tail in ["too narr", "need roo", "all roo", "sort: C "] {
            assert!(!text.ends_with(tail), "clipped mid-clause at {w}: {text:?}");
        }
        assert!(
            text.contains("processes ("),
            "the panel lost its own name at {w}: {text:?}"
        );
        // Wide enough for the warning, and it is there — reading as a warning
        // rather than as one more fact behind an identical `·`.
        if w >= 100 {
            assert!(
                text.contains("! io: 3/4 need root"),
                "the io warning went missing at {w}: {text:?}"
            );
        }
    }
}

#[test]
fn folding_the_user_column_lets_the_io_columns_appear_sooner() {
    // `command_width` learned that the folded column's ten columns are free;
    // this sibling threshold did not, so the disk columns went on refusing to
    // appear until the terminal was ten columns wider than they needed.
    let with_user = ui::min_width_for_io_for_test(true);
    let without = ui::min_width_for_io_for_test(false);
    assert_eq!(
        with_user - without,
        10,
        "the threshold did not come down by the width of the column"
    );

    // And it is the rendered behaviour, not just the arithmetic: at a width
    // between the two, one user gets the disk columns and two do not.
    let between = without + 2;
    assert!(between < with_user, "no width lies between the thresholds");

    let build = |user: &str| {
        let mut app = App::new(60);
        for _ in 0..App::CONSTANT_FOR {
            let mut s = sample(10.0);
            s.io_collected = true;
            s.procs = vec![
                proc_named(101, "postgres", 20.0, 1 << 20),
                ProcSample {
                    user: std::sync::Arc::from(user),
                    ..proc_named(102, "nginx", 19.0, 1 << 20)
                },
            ];
            app.push(s);
        }
        app.show_io = true;
        app.theme = Theme::new(Palette::Safe, Tier::TrueColor);
        app
    };

    let one = build("root");
    assert!(
        rows(&one, between, 20).iter().any(|l| l.contains("DISK R")),
        "one user, and the disk columns are still withheld at {between}"
    );
    let two = build("operator");
    assert!(
        !rows(&two, between, 20).iter().any(|l| l.contains("DISK R")),
        "two users, and there is not room for the disk columns at {between}"
    );
}

#[test]
fn the_columns_that_identify_a_process_are_adjacent() {
    // They used to sit at opposite ends of the row with eight columns of
    // measurement between them and ten of braille immediately before the name,
    // so reading a row meant starting at the left, jumping seventy columns
    // right to find out what it was, and coming back.
    let mut app = App::new(60);
    for _ in 0..App::CONSTANT_FOR {
        let mut s = sample(10.0);
        s.io_collected = true;
        // Two users, so the USER column is not folded into the title — this
        // test is about where the identity columns sit, not about collapsing.
        s.procs = vec![
            ProcSample {
                io: Some(crate::sample::IoRates {
                    read: 1 << 20,
                    write: 0,
                }),
                user: std::sync::Arc::from("operator"),
                cmd: Some(std::sync::Arc::from("node /srv/api/server.js")),
                ..proc_named(4821, "node", 31.2, 1 << 30)
            },
            proc_named(4822, "cron", 1.0, 1 << 20),
        ];
        app.push(s);
    }
    app.show_io = true;
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);

    let frame = rows(&app, 150, 16);
    let head = frame.iter().find(|l| l.contains("PID")).unwrap();
    let at = |pat: &str| {
        head.find(pat)
            .map(|b| head[..b].chars().count())
            .unwrap_or_else(|| panic!("{pat:?} not in {head:?}"))
    };

    // Every measurement comes before every part of the identity.
    let identity = at("PID");
    // `S` is searched with its padding: a bare `"S"` matches inside `RSS` and
    // the assertion would pass for the wrong reason.
    for measure in ["CPU%", "RSS", "  S ", "THR", "DISK R", "DISK W", "HIST "] {
        assert!(
            at(measure) < identity,
            "{measure} is drawn after the identity columns: {head:?}"
        );
    }
    // And the identity columns are contiguous: nothing between PID and COMMAND
    // but USER.
    assert!(at("USER") > identity, "{head:?}");
    assert!(at("COMMAND") > at("USER"), "{head:?}");
    let between = &head[head.find("PID").unwrap() + 3..head.find("COMMAND").unwrap()];
    assert!(
        between.split_whitespace().eq(["USER"]),
        "something other than USER sits between PID and COMMAND: {between:?}"
    );
}

#[test]
fn a_warning_in_the_title_does_not_look_like_a_legend() {
    // Four kinds of statement behind identical `·` marks read as one
    // undifferentiated string of facts, and a reader could not tell which of
    // them was telling them something was wrong.
    let mut app = App::new(60);
    for _ in 0..App::CONSTANT_FOR {
        let mut s = sample(10.0);
        s.io_collected = true;
        s.io_denied = 7;
        s.procs = (0..9)
            .map(|i| proc_named(101 + i, "postgres", 20.0 - i as f32, 1 << 20))
            .collect();
        app.push(s);
    }
    app.show_io = true;
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);

    let mut term = Terminal::new(TestBackend::new(150, 20)).unwrap();
    term.draw(|f| ui::draw(f, &app)).unwrap();
    let buf = term.backend().buffer();
    let y = (0..20u16)
        .find(|y| (0..150u16).any(|x| buf[(x, *y)].symbol() == "!"))
        .expect("the warning is not on screen");

    let style_at = |pat: &str| {
        let line: String = (0..150u16).map(|x| buf[(x, y)].symbol()).collect();
        let col = line.find(pat).map(|b| line[..b].chars().count()).unwrap() as u16;
        buf[(col, y)].style()
    };
    let warned = style_at("io: 7/9 need root");
    let plain = style_at("processes (");
    assert_ne!(
        warned, plain,
        "the warning is drawn exactly like the count beside it"
    );
    // Compared on what the token sets. A cell's style also carries the
    // terminal's own `Reset` for background and underline, which the token does
    // not mention, so the two are never equal as whole structs.
    let token = app.theme.warning_style();
    assert_eq!(warned.fg, token.fg, "the warning is not the warning colour");
    assert_eq!(
        warned.add_modifier, token.add_modifier,
        "the warning is not weighted like one"
    );

    // And when nothing is denied there is no message at all: the columns are
    // their own legend.
    let mut clean = App::new(60);
    for _ in 0..App::CONSTANT_FOR {
        let mut s = sample(10.0);
        s.io_collected = true;
        s.io_denied = 0;
        s.procs = vec![proc_named(101, "postgres", 20.0, 1 << 20)];
        clean.push(s);
    }
    clean.show_io = true;
    clean.theme = Theme::new(Palette::Safe, Tier::TrueColor);
    let title = rows(&clean, 150, 20)
        .into_iter()
        .find(|l| l.contains("processes ("))
        .unwrap();
    assert!(
        !title.contains("io") && !title.contains('!'),
        "a readable box is being told about io it can already see: {title:?}"
    );
}

#[test]
fn the_sparkline_column_keeps_its_name_on_a_many_core_box() {
    // The ceiling doubles past one core, so a busy process on a sixteen-core
    // box gives 1600 and `HIST ≤1600%` is eleven columns against ten. Falling
    // straight back to the bare scale left nothing on screen saying that column
    // was history — on exactly the machines where the sparkline matters most,
    // and the section title no longer says it either.
    for ceiling in [10.0f32, 50.0, 100.0, 200.0, 800.0, 1600.0, 3200.0, 12800.0] {
        let h = ui::spark_header_for_test(ceiling);
        assert!(
            h.chars().count() <= ui::SPARK_W,
            "the header overflows its column at {ceiling}: {h:?} is {} wide",
            h.chars().count()
        );
        assert!(
            h.contains(&format!("{ceiling:.0}")) || ceiling >= 10000.0,
            "the scale is missing at {ceiling}: {h:?}"
        );
        if ceiling <= 3200.0 {
            assert!(
                h.starts_with('H'),
                "the column lost its name at {ceiling}: {h:?}"
            );
        }
    }
}

#[test]
fn every_marker_the_chrome_draws_is_one_column_wide() {
    // Every width in `ui` is counted in `chars`, so a glyph a terminal draws
    // two columns wide runs the rule past its panel. `⚠` was the one that
    // prompted this: several terminals give the warning sign emoji
    // presentation. This is the byte-versus-column mistake one layer up.
    let mut app = App::new(60);
    for _ in 0..App::CONSTANT_FOR {
        let mut s = sample(10.0);
        s.io_collected = true;
        s.io_denied = 3;
        s.procs = (0..4)
            .map(|i| proc_named(101 + i, "postgres", 20.0 - i as f32, 1 << 20))
            .collect();
        app.push(s);
    }
    app.show_io = true;
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);

    for w in [80u16, 100, 140] {
        for line in rows(&app, w, 20) {
            for ch in line.chars() {
                assert!(
                    !matches!(ch, '⚠' | '⛔' | '❗' | '✅' | '❌'),
                    "an emoji-presentation glyph reached the frame at {w}: {ch:?} in {line:?}"
                );
            }
        }
    }
}

#[test]
fn the_readme_shows_the_table_this_version_draws() {
    // The sample output went stale in exactly the way the change that made it
    // stale was about: it showed the old split identity and the old column
    // headers, so anyone comparing the README to a running instance saw a
    // different table.
    let readme = include_str!("../README.md");
    let mut app = App::new(60);
    for _ in 0..App::CONSTANT_FOR {
        let mut s = sample(10.0);
        // The README's own four, so the count in the title matches too.
        s.procs = vec![
            ProcSample {
                cpu: 88.4,
                rss: 512 << 20,
                ..proc_named(824, "postgres", 0.0, 0)
            },
            ProcSample {
                cpu: 12.5,
                rss: 32 << 20,
                ..proc_named(1190, "nginx", 0.0, 0)
            },
            ProcSample {
                cpu: 4.2,
                rss: 148 << 20,
                ..proc_named(2077, "node", 0.0, 0)
            },
            ProcSample {
                cpu: 0.1,
                rss: 12 << 20,
                ..proc_named(1, "systemd", 0.0, 0)
            },
        ];
        app.push(s);
    }
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);
    let drawn = rows(&app, 78, 24);

    // The footer too. It is the line most likely to drift, because every key
    // added to the ladder changes what fits — and at this width the ladder
    // drops the newest ones, so the README is right only by a margin nobody
    // would notice going.
    let footer = drawn.last().expect("no footer").trim_end();
    assert!(
        readme.contains(footer),
        "the README's key line is not the one poptop draws:\n  drawn: {footer:?}"
    );

    for pat in [
        "processes (",
        "CPU%",
        "824 postgres",
        "1190 nginx",
        "2077 node",
        "1 systemd",
    ] {
        let line = drawn
            .iter()
            .find(|l| l.contains(pat))
            .unwrap_or_else(|| panic!("{pat:?} is not drawn at all"))
            .trim_end();
        assert!(
            readme.contains(line),
            "the README does not show what poptop draws:\n  drawn:  {line:?}"
        );
    }
}

/// A run of samples where three processes trade places, so the table is sorted
/// differently at every one.
fn shuffling_history(app: &mut App) {
    // postgres climbs, nginx falls, redis peaks in the middle: at sample 0 the
    // order is redis/nginx/postgres and by sample 9 it is postgres/redis/nginx.
    for i in 0..10 {
        let f = i as f32;
        let mut s = sample_at(50.0, 9 - i);
        s.procs = vec![
            ProcSample {
                cpu: f * 10.0,
                started: Some(1),
                ..proc_named(101, "postgres", 0.0, 1 << 20)
            },
            ProcSample {
                cpu: 90.0 - f * 10.0,
                started: Some(2),
                ..proc_named(102, "nginx", 0.0, 1 << 20)
            },
            ProcSample {
                cpu: 45.0 - (f - 5.0).abs() * 5.0,
                started: Some(3),
                ..proc_named(103, "redis", 0.0, 1 << 20)
            },
        ];
        app.push(s);
    }
}

#[test]
fn scrubbing_keeps_the_same_process_selected_while_the_table_reorders() {
    // The gesture this tool exists for: find the moment it went wrong, then
    // watch what that process was doing around it. Selection followed the row
    // index, so scrubbing moved the highlight from row 3 to row 11 to
    // off-screen while the reader sat still — the one gesture the tool is for
    // was the one that lost your place.
    let mut app = App::new(60);
    shuffling_history(&mut app);
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);

    // Pick redis, which is neither top nor bottom at the live sample.
    select_until(&mut app, "redis", |n| n == "redis");

    let mut positions = Vec::new();
    for _ in 0..9 {
        app.history.scrub(-1);
        let rows = app.visible_rows();
        let i = app
            .row_of(&rows)
            .expect("redis is in every sample and lost the selection");
        assert_eq!(
            &*rows[i].proc.name, "redis",
            "the selection landed on a different process"
        );
        positions.push(i);
    }
    // The table really did reorder underneath: if redis sat on the same row
    // throughout, this test would pass without proving anything.
    assert!(
        positions.windows(2).any(|w| w[0] != w[1]),
        "the table never reordered, so following it was never tested: {positions:?}"
    );
}

#[test]
fn sorting_does_not_move_the_selection_to_a_different_process() {
    let mut app = App::new(60);
    shuffling_history(&mut app);
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);
    select_until(&mut app, "redis", |n| n == "redis");

    let mut seen = Vec::new();
    for _ in 0..4 {
        app.sort = app.sort.next(false);
        let rows = app.visible_rows();
        let i = app.row_of(&rows).expect("the sort lost the selection");
        assert_eq!(&*rows[i].proc.name, "redis", "the sort moved the selection");
        seen.push(i);
    }
    assert!(
        seen.windows(2).any(|w| w[0] != w[1]),
        "no sort actually reordered the rows: {seen:?}"
    );
}

#[test]
fn a_process_absent_at_the_cursor_is_stated_rather_than_swapped() {
    // A process appearing partway through the buffer is information, and often
    // it is the information the reader scrubbed back to find.
    let mut app = App::new(60);
    for i in 0..6 {
        let mut s = sample_at(50.0, 5 - i);
        s.procs = vec![ProcSample {
            started: Some(1),
            ..proc_named(101, "postgres", 20.0, 1 << 20)
        }];
        // The build only starts halfway through.
        if i >= 3 {
            s.procs.push(ProcSample {
                started: Some(2),
                ..proc_named(102, "cargo", 90.0, 1 << 20)
            });
        }
        app.push(s);
    }
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);

    select_until(&mut app, "cargo", |n| n == "cargo");

    // Back before it started.
    app.history.goto_oldest();
    let rows = app.visible_rows();
    assert!(
        app.row_of(&rows).is_none(),
        "something is highlighted for a process that was not running"
    );
    assert_eq!(
        app.watched_but_absent(&rows).map(|w| w.name().to_string()),
        Some("cargo".to_string())
    );
    assert!(
        render(&app, 120, 20).contains("cargo not running here"),
        "the absence is silent"
    );

    // Forward to where it exists again: the selection was held, not dropped.
    app.history.goto_live();
    let rows = app.visible_rows();
    let i = app
        .row_of(&rows)
        .expect("the selection was dropped, not held");
    assert_eq!(&*rows[i].proc.name, "cargo");
}

#[test]
fn following_a_process_does_not_follow_a_recycled_pid() {
    // The whole reason the start time is part of the key. A pid reused between
    // samples would otherwise silently move the selection to a stranger — the
    // same splice `ProcSample::key` refuses one field over.
    let mut app = App::new(60);
    let mut before = sample_at(50.0, 1);
    before.procs = vec![ProcSample {
        started: Some(100),
        ..proc_named(4821, "the-first-one", 20.0, 1 << 20)
    }];
    app.push(before);

    app.select_delta(1);
    let watched = app.selected.clone().expect("nothing selected");
    assert_eq!(&**watched.name(), "the-first-one");

    // Same pid, different process.
    let mut after = sample_at(50.0, 0);
    after.procs = vec![ProcSample {
        started: Some(200),
        ..proc_named(4821, "a-stranger", 20.0, 1 << 20)
    }];
    app.push(after);

    let rows = app.visible_rows();
    assert!(
        app.row_of(&rows).is_none(),
        "the selection followed a recycled pid onto a different process"
    );
    assert_eq!(
        app.watched_but_absent(&rows).map(|w| w.name().to_string()),
        Some("the-first-one".to_string()),
        "the panel does not know the process it was following is gone"
    );
}

#[test]
fn the_cursor_row_keeps_its_anchors_when_there_is_no_room_for_a_caption() {
    // The narrowest panels drop every rung of the caption ladder. The anchors
    // are what is left, and they are the only thing that makes the marker's
    // position mean anything — a lone `▐` in an unlabelled row says nothing.
    //
    // Reachable between about eight and ten columns: below that even `past`
    // does not fit beside the marker, above it some rung of the ladder always
    // does. A first draft swept 12 to 20 and asserted nothing at all, because
    // every width in that range still had a caption.
    let mut app = App::new(600);
    for i in (0..40).rev() {
        app.push(sample_at(50.0, i));
    }
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);
    app.history.scrub(-20);

    let mut checked = 0;
    for w in 6..=12u16 {
        let r = ui::timeline_rows_range(24);
        let row = render_lines(&app, w, 24)[r.end as usize - 1].clone();
        if row.contains("shown") || row.contains("cpu") {
            continue; // the other branch, which has its own test
        }
        // `past` is drawn only where the marker is not standing in it.
        let Some(col) = cursor_column(&app, w, 24) else {
            continue;
        };
        if col >= 4 {
            checked += 1;
            assert!(
                row.contains("past"),
                "a caption-less cursor row lost its anchor at {w}: {row:?}"
            );
        }
    }
    assert!(
        checked > 0,
        "no width exercised the caption-less branch, so this test asserted nothing"
    );
}

#[test]
fn the_viewport_holds_its_place_while_the_watched_process_is_absent() {
    // Absence suppresses the highlight; it must not also snap the list home.
    // Scrubbing back past the moment a process started made the table jump to
    // the top and back on every arrow key.
    let mut app = App::new(60);
    for i in 0..6 {
        let mut s = sample_at(50.0, 5 - i);
        s.procs = (0..40)
            .map(|n| ProcSample {
                started: Some(n as u64 + 1),
                ..proc_named(200 + n, &format!("worker{n:02}"), 40.0 - n as f32, 1 << 20)
            })
            .collect();
        // The one being watched only exists in the newest three samples.
        if i >= 3 {
            s.procs.push(ProcSample {
                started: Some(999),
                ..proc_named(999, "latecomer", 0.5, 1 << 20)
            });
        }
        app.push(s);
    }
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);

    select_until(&mut app, "latecomer", |n| n == "latecomer");
    let at_live = app.resume_row();
    assert!(at_live > 20, "the fixture does not scroll: row {at_live}");

    app.history.goto_oldest();
    let visible = app.visible_rows();
    assert!(
        app.row_of(&visible).is_none(),
        "highlighted a process that had not started"
    );
    assert_eq!(
        app.resume_row(),
        at_live,
        "the viewport snapped home when the process went missing"
    );
    // The frame is still scrolled there: the row it was on is on screen.
    let frame = rows(&app, 120, 20).join("\n");
    assert!(
        frame.contains("worker2") || frame.contains("worker3"),
        "the list scrolled back to the top:\n{frame}"
    );
}

#[test]
fn an_empty_filter_result_does_not_destroy_the_selection() {
    // A filter matching nothing says nothing about the watched process — it is
    // still running. One reflexive arrow key used to clear it, and clearing the
    // filter came back with nothing selected.
    let mut app = App::new(60);
    app.push(sample(10.0));
    app.select_delta(1);
    let watched = app.selected.clone().expect("nothing selected");

    app.filter = "no-such-process".into();
    assert!(
        app.visible_rows().is_empty(),
        "the fixture matched something"
    );
    app.select_delta(1);
    app.select_delta(-1);
    assert_eq!(
        app.selected.as_ref(),
        Some(&watched),
        "an arrow key during an empty filter destroyed the selection"
    );

    app.filter.clear();
    assert!(
        app.row_of(&app.visible_rows()).is_some(),
        "the selection did not come back with the rows"
    );
}

#[test]
fn the_absence_message_names_the_process_the_way_the_table_did() {
    // `name` is the kernel's fifteen-character `comm`, so a reader who selected
    // `node /srv/api/server.js` was told `node not running here` — which on a
    // box with four node services identifies nothing. That is the failure the
    // command-line column exists to fix.
    let mut app = App::new(60);
    for i in 0..4 {
        let mut s = sample_at(50.0, 3 - i);
        s.procs = vec![ProcSample {
            started: Some(1),
            ..proc_named(101, "postgres", 20.0, 1 << 20)
        }];
        if i >= 2 {
            s.procs.push(ProcSample {
                started: Some(2),
                cmd: Some(std::sync::Arc::from("node /srv/api/server.js --port 3000")),
                ..proc_named(102, "node", 30.0, 1 << 20)
            });
        }
        app.push(s);
    }
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);

    select_until(&mut app, "the node service", |n| n.contains("server.js"));
    app.history.goto_oldest();
    let frame = render(&app, 140, 20);
    assert!(
        frame.contains("node /srv/api/server.js --port 3000 not running here"),
        "the absence message does not name what the table showed"
    );
}

#[test]
fn the_now_anchor_survives_a_cursor_near_but_not_on_it() {
    // The guard was `cell + 3 <= width - 3`, which drops `now` two columns
    // early — and just shy of the live edge is one of the commonest scrub
    // positions. `now` occupies the last three columns, so it is only in the
    // way when the marker is actually in them.
    let mut app = App::new(600);
    for i in (0..200).rev() {
        app.push(sample_at(50.0, i));
    }
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);

    let (w, h) = (100u16, 14u16);
    let mut checked = 0;
    for back in 1..12 {
        app.history.goto_live();
        app.history.scrub(-back);
        let Some(col) = cursor_column(&app, w, h) else {
            continue;
        };
        let r = ui::timeline_rows_range(h);
        let row = render_lines(&app, w, h)[r.end as usize - 1].clone();
        if col + 3 < w {
            checked += 1;
            assert!(
                row.contains("now"),
                "the now anchor was dropped with the marker at {col} of {w}: {row:?}"
            );
        }
    }
    assert!(checked > 0, "no cursor position exercised this");
}

/// A sample with all three stall figures set independently.
///
/// Distinct from `with_pressure`, which pins `cpu.full` at zero — this one has
/// to be able to make the CPU the worst of the three.
fn stalling(cpu: f32, io: f32, memory: f32) -> Sample {
    let mut s = sample(10.0);
    s.pressure = Some(crate::sample::Pressure {
        cpu: crate::sample::Stall {
            some: cpu,
            full: cpu,
        },
        io: crate::sample::Stall { some: io, full: io },
        memory: crate::sample::Stall {
            some: memory,
            full: memory,
        },
    });
    s
}

#[test]
fn the_constrained_resource_is_the_one_stalling_work_not_the_one_that_is_busy() {
    use crate::app::{Constraint, constraint_of};

    // A disk at 100% utilisation that nothing is waiting on is not a
    // constraint, which is why stall pressure is consulted first where the
    // kernel publishes it.
    assert_eq!(constraint_of(&stalling(0.0, 0.0, 0.0)), None);
    assert_eq!(
        constraint_of(&stalling(0.0, 30.0, 0.0)),
        Some(Constraint::Disk)
    );
    assert_eq!(
        constraint_of(&stalling(40.0, 6.0, 0.0)),
        Some(Constraint::Cpu),
        "the worst stall did not win"
    );
    // `full`, not `some`: a working machine has something waiting on something
    // constantly, so a low figure is the normal state and names nothing.
    assert_eq!(constraint_of(&stalling(4.9, 4.9, 4.9)), None);

    // And the distinction itself, which `stalling` cannot express because it
    // sets the two equal. `some` at 80% with `full` at zero is a busy, healthy
    // machine: something was always waiting, and nothing was ever blocked.
    // Reading `some` here would name a constraint on every working box.
    let mut busy = sample(10.0);
    busy.pressure = Some(crate::sample::Pressure {
        cpu: crate::sample::Stall {
            some: 80.0,
            full: 0.0,
        },
        io: crate::sample::Stall {
            some: 90.0,
            full: 0.0,
        },
        memory: crate::sample::Stall {
            some: 70.0,
            full: 0.0,
        },
    });
    assert_eq!(
        constraint_of(&busy),
        None,
        "a busy machine with nothing blocked was called constrained"
    );
}

#[test]
fn swap_in_use_is_not_by_itself_a_memory_constraint() {
    use crate::app::{Constraint, constraint_of};

    // macOS swaps routinely on a machine with gigabytes free, so `swap_used >
    // 0` named memory as the constraint on every idle Mac — permanently, and
    // while the disk was the thing actually in the way.
    let steady = |used: u64| {
        let mut s = sample(10.0);
        s.pressure = None;
        s.mem.swap_total = 4 << 30;
        s.mem.swap_used = used;
        s.mem.total = 24 << 30;
        s.mem.available = 1 << 30;
        s
    };
    // Not even out of headroom: `available` on the only platform that reaches
    // this path is not a partition of `total`, so it is not consulted at all.
    assert_eq!(constraint_of(&steady(2 << 30)), None);

    let mut app = App::new(60);
    for _ in 0..App::CONSTANT_FOR {
        app.push(steady(2 << 30));
    }
    assert_eq!(app.constraint(), None, "idle swap named a constraint");

    // Growing swap is memory pressure being paid for, and it is a measurement
    // rather than an estimate.
    let mut rising = App::new(60);
    for i in 0..App::CONSTANT_FOR {
        rising.push(steady((2 << 30) + (i as u64) * (64 << 20)));
    }
    assert_eq!(rising.constraint(), Some(Constraint::Memory));
}

#[test]
fn the_panel_names_the_constraint_but_never_applies_it() {
    let mut app = App::new(60);
    for _ in 0..App::CONSTANT_FOR {
        let mut s = stalling(0.0, 30.0, 0.0);
        s.io_collected = true;
        s.procs = (0..4)
            .map(|i| ProcSample {
                io: Some(crate::sample::IoRates {
                    read: (4 - i as u64) << 20,
                    write: 0,
                }),
                ..proc_named(101 + i, &format!("worker{i}"), 20.0 - i as f32, 1 << 20)
            })
            .collect();
        app.push(s);
    }
    app.show_io = true;
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);

    let before = app.sort;
    let title = rows(&app, 140, 20)
        .into_iter()
        .find(|l| l.contains("processes ("))
        .unwrap();
    assert!(
        title.contains("disk is the constraint"),
        "the panel does not name the constraint: {title:?}"
    );
    assert_eq!(app.sort, before, "the sort was applied without being asked");

    // Accepting it is one key.
    app.sort = app.constraint().expect("no constraint").sort();
    assert_eq!(app.sort, crate::app::Sort::Disk);
    let rows_now = app.visible_rows();
    let names: Vec<&str> = rows_now.iter().map(|r| r.proc.command()).collect();
    assert_eq!(names, vec!["worker0", "worker1", "worker2", "worker3"]);

    // …and once accepted there is nothing left to suggest.
    let title = rows(&app, 140, 20)
        .into_iter()
        .find(|l| l.contains("processes ("))
        .unwrap();
    assert!(
        !title.contains("is the constraint"),
        "still suggesting a sort the table is already using: {title:?}"
    );
}

#[test]
fn nothing_is_claimed_when_no_resource_is_constrained() {
    let mut app = App::new(60);
    for _ in 0..App::CONSTANT_FOR {
        app.push(stalling(0.0, 0.0, 0.0));
    }
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);
    assert_eq!(app.constraint(), None);
    // Scoped to the title: the footer lists `S constraint` as a key on every
    // frame, so searching the whole thing finds that instead.
    let title = rows(&app, 140, 20)
        .into_iter()
        .find(|l| l.contains("processes ("))
        .unwrap();
    assert!(
        !title.contains("constraint"),
        "a quiet machine was told something was in the way: {title:?}"
    );
}

#[test]
fn a_constraint_that_flickers_is_not_suggested() {
    // A panel that changes its advice twice a second is worse than one that
    // gives none.
    let mut app = App::new(60);
    for i in 0..App::CONSTANT_FOR {
        app.push(if i % 2 == 0 {
            stalling(0.0, 30.0, 0.0)
        } else {
            stalling(30.0, 0.0, 0.0)
        });
    }
    assert_eq!(
        app.constraint(),
        None,
        "a constraint that changed every sample was suggested anyway"
    );
}

#[test]
fn the_constraint_is_the_one_at_the_cursor() {
    // Scrubbing back to a spike to find out what was constrained *then* is the
    // whole reason the buffer exists.
    let mut app = App::new(60);
    let io_bound = || {
        let mut s = stalling(0.0, 30.0, 0.0);
        // Collected, or the disk suggestion is withheld — a sort where every
        // figure is `None` orders nothing.
        s.io_collected = true;
        s
    };
    for _ in 0..App::CONSTANT_FOR {
        app.push(io_bound()); // disk-bound, in the past
    }
    for _ in 0..App::CONSTANT_FOR {
        app.push(stalling(0.0, 0.0, 0.0)); // quiet, now
    }
    assert_eq!(
        app.constraint(),
        None,
        "live is quiet and it says otherwise"
    );

    app.history.goto_oldest();
    assert_eq!(
        app.constraint(),
        Some(crate::app::Constraint::Disk),
        "scrubbing back to the spike does not report what was in the way then"
    );
}

#[test]
fn the_disk_sort_puts_unreadable_processes_last_not_among_the_idle() {
    // A process whose IO could not be read is not an idle one. Ordering it as
    // zero would be the fabricated zero this codebase refuses everywhere else,
    // and here it would hide the busiest process on the box from someone who
    // had just asked to see it.
    use crate::app::Sort;
    let mut procs = [
        ProcSample {
            io: None,
            ..proc_named(101, "unreadable", 1.0, 0)
        },
        ProcSample {
            io: Some(crate::sample::IoRates { read: 0, write: 0 }),
            ..proc_named(102, "idle", 1.0, 0)
        },
        ProcSample {
            io: Some(crate::sample::IoRates {
                read: 1 << 20,
                write: 0,
            }),
            ..proc_named(103, "busy", 1.0, 0)
        },
    ];
    procs.sort_by(|a, b| Sort::Disk.compare(a, b));
    let names: Vec<&str> = procs.iter().map(|p| p.command()).collect();
    assert_eq!(names, vec!["busy", "idle", "unreadable"]);
}

#[test]
fn the_sort_cycle_skips_disk_when_there_are_no_disk_figures() {
    // A sort key every row answers `None` to is not an ordering, it is a
    // shuffle.
    use crate::app::Sort;
    let mut seen = vec![Sort::Cpu];
    let mut s = Sort::Cpu;
    for _ in 0..6 {
        s = s.next(false);
        seen.push(s);
    }
    assert!(!seen.contains(&Sort::Disk), "cycled onto an empty column");
    // …and reaches it when the figures exist.
    assert_eq!(Sort::Mem.next(true), Sort::Disk);
}

#[test]
fn a_disk_constraint_is_not_suggested_when_there_are_no_disk_figures() {
    // On a box where most of `/proc/<pid>/io` is unreadable the probe withdraws
    // those columns for good, while PSI goes on reporting io stall. The panel
    // offered `disk is the constraint`, `S` set a sort where every figure is
    // `None`, the table did not move, and the title named a column that was not
    // even drawn.
    let mut app = App::new(60);
    for _ in 0..App::CONSTANT_FOR {
        let mut s = stalling(0.0, 30.0, 0.0);
        s.io_collected = false;
        s.procs = vec![proc_named(101, "postgres", 20.0, 1 << 20)];
        app.push(s);
    }
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);
    assert_eq!(
        app.constraint(),
        None,
        "offered a sort by a column that is not collected"
    );
    let title = rows(&app, 140, 20)
        .into_iter()
        .find(|l| l.contains("processes ("))
        .unwrap();
    assert!(!title.contains("constraint"), "{title:?}");

    // With the figures, the same stall does name it.
    let mut with = App::new(60);
    for _ in 0..App::CONSTANT_FOR {
        let mut s = stalling(0.0, 30.0, 0.0);
        s.io_collected = true;
        s.procs = vec![proc_named(101, "postgres", 20.0, 1 << 20)];
        with.push(s);
    }
    assert_eq!(with.constraint(), Some(crate::app::Constraint::Disk));
}

#[test]
fn a_single_sample_is_not_a_held_constraint() {
    // The window clamps to what exists, so one sample agreed with itself: a
    // spike a second after launch named a constraint, and the advice could
    // change on every frame for the first four seconds — the flicker the
    // window exists to prevent.
    let mut app = App::new(60);
    let io_bound = || {
        let mut s = stalling(0.0, 30.0, 0.0);
        s.io_collected = true;
        s
    };
    for n in 1..App::CONSTANT_FOR {
        app.push(io_bound());
        assert_eq!(
            app.constraint(),
            None,
            "named a constraint from {n} sample(s), fewer than the hold requires"
        );
    }
    app.push(io_bound());
    assert_eq!(
        app.constraint(),
        Some(crate::app::Constraint::Disk),
        "a full window did not produce a suggestion"
    );
}

#[test]
fn equal_stalls_rank_the_way_the_fallback_does() {
    use crate::app::{Constraint, constraint_of};
    // `max_by` keeps the last of equal maxima, so the array order is what
    // decides a tie. Written least-important-first it agrees with the
    // utilisation fallback below it, where a disk with no idle time outranks a
    // busy CPU — a busy CPU is often the machine working.
    assert_eq!(
        constraint_of(&stalling(20.0, 20.0, 0.0)),
        Some(Constraint::Disk),
        "a tie between cpu and io reported cpu"
    );
    assert_eq!(
        constraint_of(&stalling(0.0, 20.0, 20.0)),
        Some(Constraint::Disk),
        "a tie between memory and io reported memory"
    );
    assert_eq!(
        constraint_of(&stalling(20.0, 0.0, 20.0)),
        Some(Constraint::Memory),
        "a tie between cpu and memory reported cpu"
    );
}

/// Six workers sharing a name, plus one process that does not.
fn worker_pool(app: &mut App, io: Option<crate::sample::IoRates>) {
    for _ in 0..App::CONSTANT_FOR {
        let mut s = sample(10.0);
        s.io_collected = true;
        s.procs = (0..6)
            .map(|i| ProcSample {
                cpu: 6.0 - i as f32,
                rss: 100u64 << 20,
                threads: Some(10),
                io,
                started: Some(i as u64 + 1),
                cmd: Some(std::sync::Arc::from(format!("ruby /srv/app/worker{i}.rb"))),
                ..proc_named(26622 + i, "ruby", 0.0, 0)
            })
            .collect();
        s.procs.push(ProcSample {
            io,
            started: Some(99),
            cmd: Some(std::sync::Arc::from("node /srv/api/server.js")),
            ..proc_named(5531, "node", 31.2, 700 << 20)
        });
        app.push(s);
    }
}

#[test]
fn grouping_folds_a_worker_pool_into_one_row_that_sums() {
    // Six rows individually unremarkable, together 21% of a core and 2.3GB —
    // which is the fact worth knowing and the one six separate rows cannot
    // state.
    let mut app = App::new(60);
    worker_pool(&mut app, None);
    app.group = true;
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);

    let rows_data = app.visible_rows();
    assert_eq!(rows_data.len(), 2, "the pool was not folded");
    let group = rows_data
        .iter()
        .find(|r| &*r.proc.name == "ruby")
        .expect("no ruby row");
    assert_eq!(group.members, Some(6));
    // 6 + 5 + 4 + 3 + 2 + 1
    assert!(
        (group.proc.cpu - 21.0).abs() < 0.001,
        "cpu did not sum: {}",
        group.proc.cpu
    );
    assert_eq!(group.proc.rss, 600 << 20, "memory did not sum");
    assert_eq!(group.proc.threads, Some(60), "threads did not sum");

    // The count replaces the pid, because a group is not a process.
    let drawn = rows(&app, 110, 14);
    let row = drawn
        .iter()
        .find(|l| l.trim_end().ends_with(" ruby"))
        .expect("the group is not drawn");
    assert!(row.contains("×6"), "the count is not shown: {row:?}");
    assert!(
        !row.contains("26622"),
        "a group is showing one member's pid: {row:?}"
    );
}

#[test]
fn a_group_states_nothing_it_cannot_sum() {
    let mut app = App::new(60);
    worker_pool(&mut app, None);
    app.group = true;
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);

    let rows_data = app.visible_rows();
    // The row that folds more than one. Every row is a group row while
    // grouping — that is what makes a pool shrinking to one keep its selection
    // — so `is_group()` alone also finds the lone `node`.
    let group = rows_data.iter().find(|r| r.count() > 1).unwrap();

    // Four sleeping and two running is not a state.
    assert_eq!(group.proc.state, '—', "a group claimed a single state");
    // Six rubies were started six different ways; the shared name is the only
    // thing true of all of them.
    assert_eq!(group.proc.cmd, None, "a group claimed one command line");
    assert_eq!(group.proc.command(), "ruby");
    // No start time, so no identity — and therefore no sparkline. A group's
    // history is not the sum of its members': membership changes as processes
    // come and go, and a line through that is continuity that never happened.
    assert_eq!(group.proc.started, None);
    assert_eq!(group.proc.key(), None, "a group produced a history key");

    let drawn = rows(&app, 110, 14);
    let row = drawn.iter().find(|l| l.contains("×6")).unwrap();
    let sparkline_glyphs = row.chars().filter(|c| ('⠀'..='⣿').contains(c)).count();
    assert_eq!(sparkline_glyphs, 0, "a group was drawn a history: {row:?}");
}

#[test]
fn a_group_with_one_unreadable_member_reports_no_io_rather_than_a_short_total() {
    // A total that silently omits a member is a smaller number presented as a
    // complete one — the fabricated zero this codebase refuses everywhere else.
    let mut app = App::new(60);
    worker_pool(
        &mut app,
        Some(crate::sample::IoRates {
            read: 1 << 20,
            write: 0,
        }),
    );
    app.group = true;

    let rows_data = app.visible_rows();
    // The row that folds more than one. Every row is a group row while
    // grouping — that is what makes a pool shrinking to one keep its selection
    // — so `is_group()` alone also finds the lone `node`.
    let group = rows_data.iter().find(|r| r.count() > 1).unwrap();
    assert_eq!(
        group.proc.io.map(|io| io.read),
        Some(6 << 20),
        "readable members did not sum"
    );
    drop(rows_data);

    // Now make one of them unreadable.
    let mut s = sample(10.0);
    s.io_collected = true;
    s.procs = (0..6)
        .map(|i| ProcSample {
            io: (i > 0).then_some(crate::sample::IoRates {
                read: 1 << 20,
                write: 0,
            }),
            started: Some(i as u64 + 1),
            ..proc_named(26622 + i, "ruby", 1.0, 1 << 20)
        })
        .collect();
    app.push(s);
    let rows_data = app.visible_rows();
    // The row that folds more than one. Every row is a group row while
    // grouping — that is what makes a pool shrinking to one keep its selection
    // — so `is_group()` alone also finds the lone `node`.
    let group = rows_data.iter().find(|r| r.count() > 1).unwrap();
    assert!(
        group.proc.io.is_none(),
        "a group summed five of six and presented it as the total"
    );
}

#[test]
fn grouping_and_the_tree_are_mutually_exclusive() {
    // Grouping destroys parentage by construction, so a grouped tree would be a
    // tree of things that are not processes.
    let mut app = App::new(60);
    worker_pool(&mut app, None);

    app.group = true;
    app.tree = true;
    // Whichever the renderer honours, it must not try to do both: the tree path
    // is taken and the rows are processes, with pids.
    let rows_data = app.visible_rows();
    assert!(
        rows_data.iter().all(|r| !r.is_group()),
        "the tree drew a group"
    );
}

#[test]
fn a_group_can_be_followed_across_samples() {
    // A group has no `(pid, started)` — its figures are a sum and its
    // membership changes. Following the name is the only thing that stays true
    // across that, and it is what the reader picked.
    let mut app = App::new(60);
    worker_pool(&mut app, None);
    app.group = true;
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);

    select_until(&mut app, "the ruby group", |n| n == "ruby");
    assert!(
        matches!(app.selected, Some(crate::app::Watched::Group { .. })),
        "a group was followed as a process: {:?}",
        app.selected
    );

    // One member exits; the group is still the group.
    let mut s = sample(10.0);
    s.procs = (0..5)
        .map(|i| ProcSample {
            started: Some(i as u64 + 1),
            ..proc_named(26622 + i, "ruby", 1.0, 1 << 20)
        })
        .collect();
    app.push(s);
    let rows_data = app.visible_rows();
    let i = app
        .row_of(&rows_data)
        .expect("the group lost its selection");
    assert_eq!(rows_data[i].members, Some(5));
}

#[test]
fn the_title_counts_processes_even_when_a_row_stands_for_six() {
    // Rows and processes were the same thing until a row could stand for six of
    // them. The title then read `processes (2)` above seven running processes —
    // the lie by omission this panel is careful never to tell.
    let mut app = App::new(60);
    worker_pool(&mut app, None);
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);

    let title_of = |app: &App| {
        rows(app, 130, 16)
            .into_iter()
            .find(|l| l.contains("processes ("))
            .expect("no title")
    };
    assert!(
        title_of(&app).contains("processes (7)"),
        "{:?}",
        title_of(&app)
    );

    app.group = true;
    let grouped = title_of(&app);
    assert!(
        grouped.contains("processes (7)"),
        "grouping made the panel understate what is running: {grouped:?}"
    );
    assert!(grouped.contains("grouped"), "{grouped:?}");
}

#[test]
fn a_group_of_mixed_owners_claims_neither() {
    // Three rubies owned by alice and three by bob are not alice's, and taking
    // whichever sorted first renders a fact the group does not have.
    let mut app = App::new(60);
    for _ in 0..App::CONSTANT_FOR {
        let mut s = sample(10.0);
        s.procs = (0..6)
            .map(|i| ProcSample {
                user: std::sync::Arc::from(if i < 3 { "alice" } else { "bob" }),
                started: Some(i as u64 + 1),
                ..proc_named(26622 + i, "ruby", 6.0 - i as f32, 100 << 20)
            })
            .collect();
        app.push(s);
    }
    app.group = true;
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);

    let rows_data = app.visible_rows();
    let group = rows_data.iter().find(|r| r.count() > 1).unwrap();
    assert_eq!(&*group.proc.user, "—", "the group took one member's user");

    let frame = rows(&app, 130, 16).join("\n");
    assert!(
        !frame.contains("alice") && !frame.contains("bob"),
        "a mixed-owner group is drawn as belonging to somebody:\n{frame}"
    );

    // …and when they do agree, it says so.
    let mut same = App::new(60);
    worker_pool(&mut same, None);
    same.group = true;
    let rows_data = same.visible_rows();
    let group = rows_data.iter().find(|r| r.count() > 1).unwrap();
    assert_eq!(&*group.proc.user, "root");
}

#[test]
fn a_group_shrinking_to_one_process_keeps_its_selection() {
    // Every row is a group row while grouping, even a name with one process
    // under it. Deriving group-ness from the count instead made the highlight
    // vanish the moment a pool shrank to one, with nothing said — the row
    // stopped being a group, and a group selection stopped matching it.
    let mut app = App::new(60);
    worker_pool(&mut app, None);
    app.group = true;
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);
    select_until(&mut app, "the ruby group", |n| n == "ruby");

    for remaining in (1..=5).rev() {
        let mut s = sample(10.0);
        s.procs = (0..remaining)
            .map(|i| ProcSample {
                started: Some(i as u64 + 1),
                ..proc_named(26622 + i, "ruby", 1.0, 1 << 20)
            })
            .collect();
        app.push(s);

        let rows_data = app.visible_rows();
        let i = app
            .row_of(&rows_data)
            .unwrap_or_else(|| panic!("selection lost at {remaining} member(s)"));
        assert_eq!(rows_data[i].count(), remaining as usize);
    }

    // A lone group keeps the pid, because there is a single process there and
    // `×1` says less than its number does.
    let frame = rows(&app, 130, 16).join("\n");
    assert!(
        frame.contains("26622"),
        "the lone process lost its pid:\n{frame}"
    );
    assert!(
        !frame.contains("×1"),
        "a group of one is drawn as a count:\n{frame}"
    );
}

#[test]
fn sorting_by_pid_puts_a_group_where_its_oldest_process_is() {
    // A placeholder zero sorted every group above every process regardless of
    // what was in it, while the column it was nominally sorting by showed `×6`.
    let mut app = App::new(60);
    worker_pool(&mut app, None);
    app.group = true;
    app.sort = crate::app::Sort::Pid;
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);

    let rows_data = app.visible_rows();
    let names: Vec<&str> = rows_data.iter().map(|r| r.proc.name.as_ref()).collect();
    // node is pid 5531; the ruby pool starts at 26622.
    assert_eq!(
        names,
        vec!["node", "ruby"],
        "the group did not sort by the pids it contains"
    );
}

#[test]
fn a_query_is_evaluated_at_the_cursor_not_against_the_live_sample() {
    // The thing no live-only tool can be asked: what was in D-state at the
    // moment of the spike.
    let mut app = App::new(60);
    for i in 0..6 {
        let mut s = sample_at(50.0, 5 - i);
        // Blocked during the spike, running now.
        let state = if i < 3 { 'D' } else { 'R' };
        s.procs = vec![ProcSample {
            state,
            started: Some(1),
            ..proc_named(101, "postgres", 20.0, 1 << 20)
        }];
        app.push(s);
    }
    app.filter = "state = D".into();
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);

    assert!(
        app.visible_rows().is_empty(),
        "nothing is blocked now and the query found something"
    );
    app.history.goto_oldest();
    assert_eq!(
        app.visible_rows().len(),
        1,
        "scrubbing back to the spike does not answer what was blocked then"
    );
}

#[test]
fn a_malformed_query_hides_nothing_and_says_why() {
    let mut app = App::new(60);
    app.push(sample(10.0));
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);
    let all = app.visible_rows().len();

    app.filter = "cpuu > 5".into();
    assert_eq!(
        app.visible_rows().len(),
        all,
        "a mistyped query hid rows the reader was looking for"
    );
    let why = app.filter_error().expect("no error reported");
    assert!(why.contains("cpuu") && why.contains("cpu"), "{why}");

    // Said where the query is typed…
    app.editing_filter = true;
    // The filter line itself, not the whole frame. The title carries the same
    // message, so a frame-wide search is satisfied by the title even when the
    // line where the query is being typed says nothing — and the explanation,
    // not the echo, because the box already shows what was typed.
    let drawn = rows(&app, 140, 20);
    let line = drawn
        .iter()
        .find(|l| l.starts_with("filter:"))
        .expect("no filter line");
    assert!(
        line.contains("no field called"),
        "the error is not shown where the query is typed: {line:?}"
    );
    assert!(line.contains("mem"), "the error does not name the fields");

    // …and in the title once the box has closed, because a filter that is
    // filtering nothing is a surprising thing to be doing silently.
    app.editing_filter = false;
    let title = rows(&app, 160, 20)
        .into_iter()
        .find(|l| l.contains("processes ("))
        .unwrap();
    assert!(title.contains("filter:"), "{title:?}");
}

#[test]
fn a_query_finds_the_processes_a_header_figure_counts() {
    // Each of these is a figure the reader is already looking at, and the table
    // had no way to answer "which processes are *that*".
    let mut app = App::new(60);
    let mut s = sample(10.0);
    s.io_collected = true;
    s.procs = vec![
        ProcSample {
            state: 'D',
            io: Some(crate::sample::IoRates {
                read: 0,
                write: 4 << 20,
            }),
            threads: Some(200),
            started: Some(1),
            ..proc_named(101, "writer", 2.0, 1 << 30)
        },
        ProcSample {
            state: 'S',
            io: Some(crate::sample::IoRates { read: 0, write: 0 }),
            threads: Some(4),
            started: Some(2),
            ..proc_named(102, "idler", 1.0, 1 << 20)
        },
    ];
    app.push(s);
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);

    for (q, want) in [
        ("state = D", "writer"),
        ("write > 1mb", "writer"),
        ("threads > 100", "writer"),
        ("mem > 500mb", "writer"),
        ("state = S", "idler"),
    ] {
        app.filter = q.into();
        let got = app.visible_rows();
        assert_eq!(got.len(), 1, "`{q}` matched {} rows", got.len());
        assert_eq!(got[0].proc.command(), want, "`{q}` found the wrong one");
    }
}

#[test]
fn a_machine_at_nominal_clock_spends_no_header_space_saying_so() {
    // A figure present on every frame is one nobody reads by the second day.
    // Drivers also report ceilings a fraction under the hardware maximum as a
    // matter of course, so `CLK 99.7%` would be permanent on a machine that is
    // not throttled at all.
    let mut app = App::new(60);
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);
    for (ceiling, want) in [
        (None, false),
        (Some(100.0), false),
        (Some(99.7), false),
        (Some(98.0), true),
        (Some(62.0), true),
    ] {
        let mut s = sample(10.0);
        s.clock_ceiling = ceiling;
        let mut a = App::new(60);
        a.theme = app.theme;
        a.push(s);
        let header = rows(&a, 160, 20)
            .into_iter()
            .find(|l| l.contains("CPU"))
            .expect("no header");
        assert_eq!(
            header.contains("CLK"),
            want,
            "clock ceiling {ceiling:?} drew {header:?}"
        );
    }
}

#[test]
fn the_clock_figure_qualifies_the_cpu_figure_it_sits_beside() {
    // `CPU 100%` and `CLK 62%` together say the processor is flat out and
    // getting two thirds of the work done — a different machine from `CPU 100%`
    // alone, and nothing else on this header can tell them apart. So it sits
    // next to the figure it qualifies, not at the end of the row.
    let mut app = App::new(60);
    let mut s = sample(10.0);
    s.cpu_total = 100.0;
    s.clock_ceiling = Some(62.0);
    app.push(s);
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);

    let header = rows(&app, 160, 20)
        .into_iter()
        .find(|l| l.contains("CPU"))
        .unwrap();
    let cpu = header.find("CPU").unwrap();
    let clk = header.find("CLK").expect("the clock figure is not drawn");
    assert!(clk > cpu, "{header:?}");
    // Nothing between them but the CPU figure itself.
    let between = &header[cpu + 3..clk];
    assert!(
        !between.contains("MEM") && !between.contains("WAIT"),
        "another figure came between the clock and the cpu it qualifies: {between:?}"
    );
    assert!(header.contains("62.0%"), "{header:?}");

    // And it is given up late, which is the other half of "it qualifies CPU":
    // a header narrow enough to lose figures must lose the ones that are not
    // saying the machine is in trouble first. Rank decides what is dropped;
    // group decides where it sits, so the position above proves nothing about
    // the ladder.
    let narrow = rows(&app, 46, 20)
        .into_iter()
        .find(|l| l.contains("CPU"))
        .expect("no header at 46 columns");
    assert!(
        narrow.contains("CLK"),
        "the clock figure was given up before the incidental ones: {narrow:?}"
    );
    assert!(
        !narrow.contains("UP ") && !narrow.contains("PROCS"),
        "the narrow header still has room for everything, so this asserts \
         nothing: {narrow:?}"
    );
}

#[test]
fn a_clock_ceiling_survives_a_round_trip() {
    // It is a fact about the moment, so scrubbing back to a throttled minute
    // has to still report it.
    let mut s = sample(10.0);
    s.clock_ceiling = Some(62.5);
    let mut quiet = sample(10.0);
    quiet.clock_ceiling = None;
    let back = crate::store::decode(&crate::store::encode(&[&s, &quiet])).expect("did not decode");
    assert_eq!(back[0].clock_ceiling, Some(62.5));
    assert_eq!(
        back[1].clock_ceiling, None,
        "a platform that would not say came back claiming a figure"
    );
}

#[test]
fn the_scripted_output_says_when_the_machine_is_capped() {
    // A script reading only `cpu` sees 100% on a capped machine and on a
    // healthy one — the same failure the `stall` line was added to prevent, and
    // here there is nothing else in the output that could give it away.
    let mut s = sample(10.0);
    s.cpu_total = 100.0;
    s.clock_ceiling = Some(62.0);
    let capped = crate::clock_line(&s).expect("a capped machine said nothing");
    assert!(capped.contains("62.0%"), "{capped}");
    assert!(capped.starts_with("clock"), "{capped}");

    // …and a machine at full speed spends no line saying so, as the header
    // spends no columns.
    s.clock_ceiling = Some(100.0);
    assert_eq!(crate::clock_line(&s), None);
    s.clock_ceiling = Some(99.7);
    assert_eq!(
        crate::clock_line(&s),
        None,
        "a rounding artefact was announced"
    );
    s.clock_ceiling = None;
    assert_eq!(crate::clock_line(&s), None);
}

/// A buffer in which `cargo` runs for part of the window and `postgres` throughout.
fn a_build_that_starts_and_finishes(app: &mut App) {
    for i in 0..40 {
        let mut s = sample_at(50.0, 39 - i);
        s.io_collected = true;
        s.procs = vec![proc_named(101, "postgres", 4.0, 200 << 20)];
        if (12..36).contains(&i) {
            s.procs.push(ProcSample {
                cpu: 20.0 + (i - 12) as f32 * 2.5,
                rss: (300 + (i - 12) * 40) << 20,
                threads: Some(4 + (i as u32 - 12) / 3),
                io: Some(crate::sample::IoRates {
                    read: (i - 12) << 19,
                    write: 1 << 18,
                }),
                started: Some(2),
                cmd: Some(std::sync::Arc::from("cargo build --release")),
                ..proc_named(102, "cargo", 0.0, 0)
            });
        }
        app.push(s);
    }
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);
    // Selected from a moment it was running; the view is then asked about the
    // whole buffer, including the moments it was not.
    app.history.scrub(-10);
    select_until(app, "cargo", |n| n.contains("cargo"));
    app.history.goto_live();
}

#[test]
fn a_key_opens_the_selected_process_history_at_full_width() {
    // The buffer already holds it. It was a ten-column sparkline in a table
    // row: about one percent of the screen for the thing the tool is built
    // around.
    let mut app = App::new(600);
    a_build_that_starts_and_finishes(&mut app);

    let machine = rows(&app, 110, 24);
    assert!(
        machine.iter().any(|l| l.contains("── timeline")),
        "the timeline is not drawn before the key is pressed"
    );

    app.detail = true;
    let detail = rows(&app, 110, 24);
    let title = detail
        .iter()
        .find(|l| l.starts_with("── "))
        .expect("no panel title");
    assert!(
        title.contains("cargo build --release"),
        "the panel does not say whose history it is: {title:?}"
    );
    assert!(
        !title.contains("timeline"),
        "the panel changed subject and kept its old name: {title:?}"
    );
    // Full width, not ten columns. Scoped to the timeline block: the machine
    // header also says `CPU`, and a frame-wide search finds that instead — a
    // line with no braille on it at all, which then reads as "zero columns
    // wide" and fails for the wrong reason.
    let r = ui::timeline_rows_range(24);
    let graph = detail[r.start as usize..r.end as usize]
        .iter()
        .find(|l| l.contains("CPU"))
        .expect("no cpu row in the timeline block");
    let drawn = graph.chars().filter(|c| ('⠀'..='⣿').contains(c)).count();
    assert!(
        drawn > 60,
        "the history is {drawn} columns wide, not full width"
    );
}

#[test]
fn the_detail_view_draws_from_the_buffer_with_no_new_collection() {
    // Entirely a rendering question, which is what makes it cheap for its
    // value: the same `Needs` before and after, so nothing extra is read.
    let mut app = App::new(600);
    a_build_that_starts_and_finishes(&mut app);
    let before = app.needs();
    app.detail = true;
    let after = app.needs();
    assert_eq!(
        before.asked(crate::collect::Source::Io),
        after.asked(crate::collect::Source::Io),
        "opening the detail view asked the collector for something new"
    );
}

#[test]
fn where_the_process_was_absent_is_marked_rather_than_drawn_as_zero() {
    // A process that did not exist did not use no CPU — it used none of
    // anything because it was not there, and a flat line at the bottom says the
    // opposite. When it started and when it went are often the whole answer.
    let mut app = App::new(600);
    a_build_that_starts_and_finishes(&mut app);
    app.detail = true;

    let frame = rows(&app, 110, 24);
    let caption = frame
        .iter()
        .find(|l| l.contains("shown,"))
        .expect("no caption");
    assert!(
        caption.contains("not running"),
        "the absence is not named: {caption:?}"
    );
    // …and it is called that rather than `time missing`, which is what a seam
    // in the machine's own graph means and is a different claim.
    assert!(!caption.contains("time missing"), "{caption:?}");

    // Scoped to the timeline block, or the machine header's own `CPU` is
    // found instead — a line with no graph on it.
    let gap = app.glyphs.gap_glyph();
    let r = ui::timeline_rows_range(24);
    let cpu = frame[r.start as usize..r.end as usize]
        .iter()
        .find(|l| l.contains("CPU"))
        .expect("no cpu row in the timeline block");
    assert!(
        cpu.contains(gap),
        "the moments it was not running are not marked: {cpu:?}"
    );

    // The machine's own graph over the same buffer has no gaps at all, so the
    // marks really are about the process.
    app.detail = false;
    let machine = rows(&app, 110, 24);
    assert!(
        !machine[r.start as usize..r.end as usize]
            .iter()
            .any(|l| l.contains(gap)),
        "the buffer itself has seams, so this proves nothing"
    );
}

#[test]
fn the_detail_view_shares_the_timeline_cursor() {
    // The cursor is the same one, so scrubbing moves both — and the process
    // table below is the real one from the moment under it.
    let mut app = App::new(600);
    a_build_that_starts_and_finishes(&mut app);
    app.detail = true;

    let at = |app: &App| cursor_column(app, 110, 24);
    assert_eq!(at(&app), None, "live, and a cursor is drawn");
    app.history.scrub(-10);
    let first = at(&app).expect("no cursor after scrubbing");
    app.history.scrub(-10);
    let second = at(&app).expect("no cursor after scrubbing further");
    assert!(
        second < first,
        "the cursor did not move: {first} then {second}"
    );
}

#[test]
fn a_process_that_is_nowhere_in_the_window_has_no_detail_to_draw() {
    // An empty graph would say it was idle. Asked of `watched_series` directly,
    // because at every terminal size this fixture fits in, the window is the
    // whole buffer — so there is no cursor position that excludes the build,
    // and a rendered test would assert nothing.
    let mut app = App::new(600);
    a_build_that_starts_and_finishes(&mut app);
    let all: Vec<&crate::sample::Sample> = app.history.iter().collect();
    assert!(
        app.watched_series(&all).is_some(),
        "the fixture does not contain the process at all"
    );

    // The first ten samples, which are before it started.
    let before = &all[..10];
    assert!(
        app.watched_series(before).is_none(),
        "a process that was never in the window still produced a series"
    );
}

#[test]
fn the_detail_rows_are_the_processs_own_figures() {
    let mut app = App::new(600);
    a_build_that_starts_and_finishes(&mut app);
    let window: Vec<&crate::sample::Sample> = app.history.iter().collect();
    let series = app
        .watched_series(&window)
        .expect("no series for the selected process");

    let names: Vec<&str> = series.rows.iter().map(|r| r.name).collect();
    assert_eq!(names, vec!["CPU", "MEM", "THR", "DISK"]);
    // Its own CPU, which peaks at 20 + 23*2.5 = 77.5, not the machine's 50.
    let cpu = &series.rows[0].values;
    let peak = cpu.iter().copied().fold(0.0_f32, f32::max);
    assert!(
        (peak - 77.5).abs() < 0.01,
        "cpu peaks at {peak}, not the process's"
    );
    // Absent at both ends, present in the middle.
    assert!(series.absent[0], "the first sample is not marked absent");
    assert!(series.absent[39], "the last sample is not marked absent");
    assert!(
        !series.absent[20],
        "a sample it was running in is marked absent"
    );
}

#[test]
fn the_graph_draws_the_processs_figures_not_the_machines() {
    // The panel can be retitled, sized and gap-marked correctly and still be
    // drawing the machine's series underneath — every other assertion here
    // passes in that case, because the title, the width and the absence marks
    // are all computed separately from the series themselves.
    //
    // Told apart by shape: the machine sits flat at 50% throughout this
    // fixture, and the build ramps from 20% to 77.5%.
    let mut app = App::new(600);
    a_build_that_starts_and_finishes(&mut app);
    let r = ui::timeline_rows_range(24);
    let cpu_of = |app: &App| {
        rows(app, 110, 24)[r.start as usize..r.end as usize]
            .iter()
            .find(|l| l.contains("CPU"))
            .expect("no cpu row")
            .clone()
    };

    let machine = cpu_of(&app);
    app.detail = true;
    let process = cpu_of(&app);
    assert_ne!(
        machine, process,
        "the detail panel is drawing the machine's own series"
    );

    // And it is the *process's* shape. Not "reaches higher": every row scales
    // to its own peak, so the machine's flat 50% fills its row completely and
    // the ramp does not — the first version of this assertion compared fill
    // and had it exactly backwards. A ramp is told from a flat line by
    // *variety*: many levels against one.
    let levels = |l: &str| {
        l.chars()
            .filter(|c| ('⠀'..='⣿').contains(c))
            .collect::<std::collections::HashSet<_>>()
            .len()
    };
    assert!(
        levels(&process) > levels(&machine),
        "the process ramps and the machine is flat, and their graphs do not \
         differ in shape: {} levels against {}",
        levels(&process),
        levels(&machine)
    );
}

#[test]
fn a_process_row_carries_no_machine_thresholds() {
    // The warn and critical percentages are about a machine's saturation.
    // Ruling them across a row measured in threads or megabytes a second
    // invents a boundary that does not exist — a dashed line at 50 MB/s wearing
    // the chrome that elsewhere means "half of everything there is".
    //
    // The machine's CPU has to *vary* for this to prove anything: a flat series
    // fills its row to the ceiling, and the rule yields wherever data is
    // present, so a constant 50% hides the rule everywhere and both panels
    // would read as ruleless.
    let mut app = App::new(600);
    for i in 0..40 {
        let mut s = sample_at(i as f32 * 2.0, 39 - i);
        s.io_collected = true;
        s.procs = vec![ProcSample {
            threads: Some(9),
            io: Some(crate::sample::IoRates {
                read: 1 << 20,
                write: 0,
            }),
            started: Some(1),
            cmd: Some(std::sync::Arc::from("cargo build --release")),
            ..proc_named(102, "cargo", 4.0 + i as f32, 200 << 20)
        }];
        app.push(s);
    }
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);
    select_until(&mut app, "cargo", |n| n.contains("cargo"));

    // By colour, not by glyph. The rule is a braille dot pattern that data can
    // produce too — counting characters found four "rules" in a panel that
    // draws none — so it is told apart the way `rule_rows` tells it apart:
    // chrome-coloured cells in the graph.
    let ruled = |app: &App| rule_rows(app, 110, 30).len();

    assert!(
        ruled(&app) > 0,
        "the machine's own graph draws no threshold rule, so this proves nothing"
    );
    app.detail = true;
    assert_eq!(
        ruled(&app),
        0,
        "a process's history is ruled with the machine's thresholds"
    );
}

#[test]
fn a_sampling_gap_is_not_reported_as_the_process_being_absent() {
    // Suspend the laptop with postgres selected and press `d`: the seam
    // covering the sleep must not assert that postgres was gone. The tool was
    // not looking, and postgres ran throughout.
    let mut app = App::new(600);
    for i in 0..20 {
        // A ten-minute hole in the middle of the record.
        let ago = if i < 10 { 620 - i * 2 } else { 20 - i };
        let mut s = sample_at(50.0, ago as u64);
        s.procs = vec![ProcSample {
            started: Some(1),
            ..proc_named(101, "postgres", 4.0, 200 << 20)
        }];
        app.push(s);
    }
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);
    select_until(&mut app, "postgres", |n| n == "postgres");
    app.detail = true;

    let caption = rows(&app, 110, 24)
        .into_iter()
        .find(|l| l.contains("shown,"))
        .expect("no caption");
    assert!(
        caption.contains("time missing"),
        "the record has a hole and the caption does not say so: {caption:?}"
    );
    assert!(
        !caption.contains("not running"),
        "a sampling gap was reported as the process being absent: {caption:?}"
    );
}

#[test]
fn a_figure_the_platform_would_not_give_is_a_gap_not_a_zero() {
    // `threads` is `None` on macOS for processes this user does not own, and
    // *every* process has no `io` in the first sample it appears in — there is
    // no previous counter to diff against. Plotting zero puts a false floor
    // under the leftmost cell of every panel.
    let mut app = App::new(600);
    for i in 0..20 {
        let mut s = sample_at(50.0, 19 - i);
        s.io_collected = true;
        s.procs = vec![ProcSample {
            threads: (i > 0).then_some(8),
            io: (i > 0).then_some(crate::sample::IoRates {
                read: 1 << 20,
                write: 0,
            }),
            started: Some(1),
            ..proc_named(101, "postgres", 4.0, 200 << 20)
        }];
        app.push(s);
    }
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);
    select_until(&mut app, "postgres", |n| n == "postgres");

    let window: Vec<&crate::sample::Sample> = app.history.iter().collect();
    let series = app.watched_series(&window).expect("no series");
    for name in ["THR", "DISK"] {
        let row = series
            .rows
            .iter()
            .find(|r| r.name == name)
            .unwrap_or_else(|| panic!("no {name} row"));
        assert!(
            row.unknown[0],
            "{name} plots the unreadable first sample as a real figure"
        );
        assert!(!row.unknown[5], "{name} marks a readable sample unknown");
    }
    // The process itself was there throughout, so this is not the absence
    // machinery answering for the figures.
    assert!(!series.absent.iter().any(|&a| a));
}

#[test]
fn the_detail_title_gives_up_clauses_rather_than_being_cut() {
    // The name is a command line and can be any length, so a constant elide
    // width truncated the clause after it — `39s of 9m59s` losing the word
    // `buffered`, or the closing rule.
    let mut app = App::new(600);
    for i in 0..20 {
        let mut s = sample_at(50.0, 19 - i);
        s.procs = vec![ProcSample {
            started: Some(1),
            cmd: Some(std::sync::Arc::from(
                "/usr/local/lib/node_modules/thing/bin/serve.js --with --flags --and --more",
            )),
            ..proc_named(101, "node", 4.0, 200 << 20)
        }];
        app.push(s);
    }
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);
    select_until(&mut app, "the node service", |n| n.contains("serve.js"));
    app.detail = true;

    for w in 40..=140u16 {
        let title = rows(&app, w, 24)
            .into_iter()
            .find(|l| l.starts_with("── "))
            .unwrap_or_else(|| panic!("no title at {w}"));
        let text = title.trim_end_matches(['─', ' ']);
        // Never cut mid-word of a clause it chose to keep.
        for tail in ["of 9m", "buffere", "—", "of"] {
            assert!(
                !text.ends_with(tail),
                "the title was cut mid-clause at {w}: {text:?}"
            );
        }
        assert!(
            text.contains("serve.js") || text.contains('…'),
            "the title stopped identifying the process at {w}: {text:?}"
        );
    }
}

#[test]
fn pressing_detail_with_nothing_selected_says_what_to_do() {
    // The key is advertised in the footer, so pressing it and getting the panel
    // you already had is the one outcome that reads as broken — and the fix is
    // one arrow key, which nothing on screen would otherwise say.
    let mut app = App::new(600);
    for i in 0..20 {
        app.push(sample_at(50.0, 19 - i));
    }
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);
    assert!(app.selected.is_none(), "the fixture selected something");

    let before = rows(&app, 110, 24)
        .into_iter()
        .find(|l| l.starts_with("── "))
        .unwrap();
    app.detail = true;
    let after = rows(&app, 110, 24)
        .into_iter()
        .find(|l| l.starts_with("── "))
        .unwrap();
    assert_ne!(before, after, "pressing the key changed nothing at all");
    assert!(
        after.contains("pick a process"),
        "the panel does not say why there is nothing to show: {after:?}"
    );

    // …and once something is selected it stops saying it.
    app.select_delta(1);
    let chosen = rows(&app, 110, 24)
        .into_iter()
        .find(|l| l.starts_with("── "))
        .unwrap();
    assert!(!chosen.contains("pick a process"), "{chosen:?}");
}

#[test]
fn elision_never_exceeds_its_budget_in_columns() {
    // `chars().count()` counts scalar values, not terminal columns. A name
    // elided "to nineteen columns" of CJK draws thirty-eight and is clipped by
    // the terminal anyway — defeating the point of eliding deliberately, which
    // is that the part identifying the process survives.
    let names = [
        "postgres",
        "データベースサーバー",             // two columns per character
        "node /srv/api/サーバー.js --port", // mixed
        "🔥🔥🔥🔥🔥🔥🔥🔥",                 // emoji, two columns each
        "e\u{301}e\u{301}e\u{301}e\u{301}", // combining marks: zero columns
        // The cases where per-character widths and per-string width disagree,
        // which is what makes summing the characters wrong. A text symbol plus
        // a variation selector measures two as a string and one as a sum, so a
        // budget filled by summing takes twice what it was given; a family
        // emoji joined by zero-width joiners measures two and sums to six, so
        // the same code throws away columns it was allowed.
        "☂\u{FE0F}☂\u{FE0F}☂\u{FE0F}☂\u{FE0F}☂\u{FE0F}",
        "👨\u{200D}👩\u{200D}👧abcdefgh",
        "⚠\u{FE0F} warning ⚠\u{FE0F}",
        "a",
        "",
    ];
    for name in names {
        for w in 0..=40usize {
            let out = ui::elide_middle(name, w);
            assert!(
                ui::cols(&out) <= w,
                "{name:?} elided to {w} columns drew {}: {out:?}",
                ui::cols(&out)
            );
            // And it does not throw away room it was given.
            if ui::cols(name) <= w {
                assert_eq!(out, name, "an already-short name was elided at {w}");
            }
            // A zero-width mark left at the front of the tail renders on the
            // elision mark instead — and a variation selector there makes `…`
            // itself take emoji presentation and two columns, which is the
            // budget overrun arriving by the back door.
            if let Some(i) = out.find('…') {
                let after = &out[i + '…'.len_utf8()..];
                assert!(
                    after
                        .chars()
                        .next()
                        .is_none_or(|c| ui::cols(&c.to_string()) > 0),
                    "a mark from the dropped character was left on the ellipsis: {out:?}"
                );
            }
        }
    }
}

#[test]
fn a_double_width_name_keeps_the_tail_it_was_elided_to_keep() {
    // The rendered check. Measuring the drawn row in columns does not work:
    // ratatui writes a double-width character into one cell and pads the next
    // with a space, so a row of CJK measures wider than the terminal while
    // occupying exactly its cells. What over-budgeting actually costs is the
    // *tail* — we hand ratatui a 38-column string for a 19-column cell, it
    // truncates from the right, and the half of the name that middle elision
    // deliberately kept is the half that disappears.
    let mut app = App::new(60);
    let mut s = sample(10.0);
    s.procs = vec![ProcSample {
        cmd: Some(std::sync::Arc::from(
            "データベースサーバー・ロングネーム・ジャーナル",
        )),
        ..proc_named(101, "postgres", 20.0, 1 << 20)
    }];
    app.push(s);
    app.theme = Theme::new(Palette::Safe, Tier::TrueColor);

    for w in 60..=140u16 {
        let row = rows(&app, w, 20)
            .into_iter()
            .find(|l| l.contains("101"))
            .unwrap_or_else(|| panic!("no process row at {w}"));
        // Matched on characters, not substrings: ratatui writes a wide
        // character into one cell and pads the next, so the drawn row reads
        // `デ ー ` and no substring of the original appears in it.
        let head = row.contains('デ');
        let tail = row.contains('ル');
        assert!(head, "the head of the name is gone at {w}: {row:?}");
        // Either the whole name fits, or it was elided and both ends survive.
        if row.contains('…') {
            assert!(tail, "the elided tail was truncated away at {w}: {row:?}");
        }
    }
}

#[test]
fn the_header_measures_its_figures_in_columns() {
    // A group of figures whose content is double-width would overflow the row.
    // Asserted on the arithmetic rather than on the drawn cells, for the same
    // reason as above — and this is the arithmetic that decides what to drop.
    let wide = "/データ/ベース";
    assert!(
        ui::cols(wide) > wide.chars().count(),
        "columns and characters agree on the fixture, so the distinction this \
         test exists for is untested: {} against {}",
        ui::cols(wide),
        wide.chars().count()
    );

    // `short_mount` is the header's own elider, and its budget is in columns.
    for w in 1..=20usize {
        let out = ui::elide_middle(wide, w);
        assert!(ui::cols(&out) <= w, "{out:?} is wider than {w} columns");
    }
}

#[test]
fn a_wide_mount_keeps_the_end_that_identifies_it() {
    // The measure was converted to columns and the cut was left as a character
    // index, which for a wide mount overshoots by the difference:
    // `/データベース/ストレージプール` is thirty columns and sixteen characters,
    // and skipping thirty of them left three columns and none of the last path
    // component — the end the doc comment says identifies it.
    //
    // This is also the test the elision one claimed to be: it said
    // "`short_mount` is the header's own elider" and then called
    // `elide_middle`, so `short_mount` had no coverage at all and this passed.
    for mount in [
        "/データベース/ストレージプール",
        "/媒体/バックアップ",
        "/var/lib/postgresql/17/main/base",
        "/",
        "/mnt/データ",
    ] {
        let out = ui::short_mount_for_test(mount);
        assert!(
            ui::cols(&out) <= 16,
            "{mount:?} shortened to {out:?}, {} columns",
            ui::cols(&out)
        );
        // It spends the budget it was given. Cutting on characters while
        // measuring in columns overshoots by the difference and leaves `…ル` —
        // three columns of sixteen — which passes a width check and is useless.
        if ui::cols(mount) > 16 {
            assert!(
                ui::cols(&out) >= 15,
                "{mount:?} shortened to {out:?}, using {} of 16 columns",
                ui::cols(&out)
            );
        }
        // The end is what identifies a mount, so the last component survives
        // whenever there is room for it.
        let last = mount.rsplit('/').next().unwrap_or("");
        if ui::cols(last) <= 15 {
            assert!(
                out.ends_with(last),
                "{mount:?} lost the component that names it: {out:?}"
            );
        }
    }
}

#[test]
fn a_byte_axis_never_loses_its_unit_to_the_gutter() {
    // `fmt_bytes` writes one decimal always, so `128.0K` is six characters
    // against a five-column gutter and the truncation left `128.0` — a byte
    // rate drawn as what looks exactly like a percentage, which is the
    // confusion this row was excluded to avoid. Three of every ten rungs on the
    // power-of-two ladder land there.
    let mut c = 1024.0f32;
    let mut checked = 0;
    while c <= 8.0 * 1024.0 * 1024.0 * 1024.0 {
        let axis = ui::Unit::Rate.axis_for_test(c);
        assert!(
            ui::cols(&axis) < ui::GUTTER_W,
            "the axis {axis:?} for {c} is {} columns, against a gutter of {}",
            ui::cols(&axis),
            ui::GUTTER_W
        );
        assert!(
            axis.ends_with(['B', 'K', 'M', 'G', 'T']),
            "the axis {axis:?} lost its unit and reads as a percentage"
        );
        checked += 1;
        c *= 2.0;
    }
    assert!(
        checked > 20,
        "only {checked} rungs of the ladder were checked"
    );

    // …and the drawn gutter agrees, which is where the truncation happened.
    for ceiling in [1024.0, 131_072.0, 524_288.0, 536_870_912.0] {
        let axis = ui::Unit::Rate.axis_for_test(ceiling);
        assert!(
            ui::cols(&axis) < ui::GUTTER_W,
            "{axis:?} would be cut by the gutter"
        );
    }
}

#[test]
fn a_ceiling_sits_above_its_peak_so_a_steady_series_is_not_full_scale() {
    // `while c < peak` returns the peak exactly whenever the peak is a power of
    // two, and the row is then solid to the top: the busiest sample being 100
    // by construction, which is the failure that kept the network row out of
    // the panel in the first place.
    for (unit, peak) in [
        (ui::Unit::Rate, 1024.0f32),
        (ui::Unit::Rate, 1024.0 * 1024.0),
        (ui::Unit::Count, 8.0),
        (ui::Unit::Count, 64.0),
    ] {
        let c = unit.ceiling_for_test(peak);
        assert!(
            c > peak,
            "a peak of {peak} got a ceiling of {c}, drawing it at full scale"
        );
    }

    // A count starts at eight, or a single-threaded process gets a ceiling of
    // one and a permanently saturated row.
    assert_eq!(ui::Unit::Count.ceiling_for_test(1.0), 8.0);
    assert_eq!(ui::Unit::Count.ceiling_for_test(4.0), 8.0);
    assert_eq!(ui::Unit::Count.ceiling_for_test(200.0), 256.0);
}

#[test]
fn every_series_the_panel_draws_is_in_the_list_the_gutter_is_sized_from() {
    // `SERIES_NAMES` is the sole input to `GUTTER_W` and is documented as every
    // name the gutter may hold. `NET` and `THR` were drawn without being in it,
    // so the derivation guaranteed nothing about them and the test that
    // enforces the guarantee skipped them.
    for name in ["CPU", "WAIT", "MEM", "DISK", "STALL", "NET", "THR"] {
        assert!(
            ui::SERIES_NAMES.contains(&name),
            "{name} is drawn in the gutter and is not in the list it is sized from"
        );
    }
}

/// A process with threads, and the sample's flat task list to match.
fn sample_with_threads() -> Sample {
    let mut s = sample(10.0);
    s.procs = vec![
        ProcSample {
            threads: Some(3),
            ..proc_named(4021, "postgres", 40.0, 900 << 20)
        },
        proc_named(4200, "sshd", 0.5, 8 << 20),
    ];
    s.tasks = Some(vec![
        ThreadSample {
            pid: 4021,
            tid: 4021,
            name: std::sync::Arc::from("postgres"),
            state: 'S',
            cpu: 1.0,
        },
        ThreadSample {
            pid: 4021,
            tid: 4098,
            name: std::sync::Arc::from("bgwriter"),
            state: 'D',
            cpu: 2.0,
        },
        ThreadSample {
            pid: 4021,
            tid: 4099,
            name: std::sync::Arc::from("walwriter"),
            state: 'R',
            cpu: 37.0,
        },
    ]);
    s
}

#[test]
fn expanding_a_process_shows_its_threads_and_only_its_threads() {
    let mut app = App::new(600);
    app.push(sample_with_threads());
    app.select_delta(1); // postgres, the first row by CPU
    app.toggle_threads();

    let rows = app.visible_rows();
    let names: Vec<String> = rows
        .iter()
        .map(|r| match &r.thread {
            Some(t) => t.name.to_string(),
            None => r.proc.name.to_string(),
        })
        .collect();
    assert_eq!(
        names,
        vec!["postgres", "walwriter", "bgwriter", "postgres", "sshd"],
        "the threads are not under their process, sorted by CPU"
    );
    // The main thread carries the process's name, and `sshd` — the other
    // process — has no rows under it.
    assert_eq!(
        rows.iter().filter(|r| r.is_thread()).count(),
        3,
        "threads of an unselected process were expanded too"
    );
}

#[test]
fn a_thread_row_does_not_repeat_its_processs_memory() {
    // Forty thread rows each showing the process's RSS would say the same
    // 900 MB forty times and imply forty copies of it.
    let mut app = App::new(600);
    app.push(sample_with_threads());
    app.select_delta(1);
    app.toggle_threads();
    // Through `rows`, not `render`: the latter concatenates every cell with no
    // line breaks, so `.lines()` on it yields the whole frame as one line and
    // "is this on the thread's row" becomes "is this anywhere on screen".
    let frame = rows(&app, 120, 20);
    let thread_row = frame
        .iter()
        .find(|l| l.contains("walwriter"))
        .expect("no thread row was drawn");
    assert!(
        thread_row.contains('—'),
        "a thread row has no em dash where the process's figures would be: {thread_row}"
    );
    assert!(
        !thread_row.contains("900"),
        "a thread row repeated its process's memory: {thread_row}"
    );
    assert!(
        thread_row.contains("4099"),
        "a thread row does not carry its tid: {thread_row}"
    );
}

#[test]
fn the_title_counts_processes_and_not_the_threads_under_them() {
    let mut app = App::new(600);
    app.push(sample_with_threads());
    app.select_delta(1);
    app.toggle_threads();
    let frame = render(&app, 120, 20);
    assert!(
        frame.contains("processes (2)"),
        "the thread rows were counted as processes:\n{frame}"
    );
}

#[test]
fn moving_the_selection_steps_over_the_thread_rows() {
    // A thread row carries its process's identity, so selecting one would
    // re-select the process and leave the cursor where it started — an arrow
    // key that visibly does nothing.
    let mut app = App::new(600);
    app.push(sample_with_threads());
    app.select_delta(1);
    app.toggle_threads();

    let before = app.visible_rows();
    let from = app.row_of(&before).expect("nothing selected");
    app.select_delta(1);
    let after = app.visible_rows();
    let to = app.row_of(&after).expect("the selection was lost");
    assert!(to > from, "the selection did not move past the thread rows");
    assert_eq!(
        after[to].proc.name.as_ref(),
        "sshd",
        "the selection did not land on the next process"
    );
}

#[test]
fn a_blocked_task_is_findable_from_the_process_that_holds_it() {
    // What makes the header's task-level figures itemisable: `BLOCKED` counts
    // tasks, so two blocked threads inside one healthy-looking process are a
    // number the process table cannot otherwise account for.
    let mut app = App::new(600);
    app.push(sample_with_threads());
    app.filter = "task = D".into();
    let rows = app.visible_rows();
    let names: Vec<&str> = rows.iter().map(|r| r.proc.name.as_ref()).collect();
    assert_eq!(
        names,
        vec!["postgres"],
        "the process holding the blocked thread was not found"
    );

    // Its own state is `S`. The process is not blocked; a thread inside it is.
    assert_eq!(
        rows[0].proc.state, 'S',
        "the fixture does not test the case"
    );
}

#[test]
fn a_task_predicate_matches_nothing_when_no_threads_were_collected() {
    // A question about data that was not gathered. Matching everything would
    // claim every process has such a thread; the panel title says why the
    // result is empty.
    let mut app = App::new(600);
    let mut s = sample_with_threads();
    s.tasks = None;
    app.push(s);
    app.filter = "task = D".into();
    assert!(
        app.visible_rows().is_empty(),
        "a predicate about threads that were never collected matched anyway"
    );
    app.show_threads = true;
    // At the live edge, one interval after the key: collection starts with the
    // next sample. Telling this reader they scrubbed too far back would send
    // them scrolling forward, where nothing would help.
    assert_eq!(
        app.thread_note(),
        Some(if cfg!(target_os = "macos") {
            "threads: not read on macOS"
        } else {
            "threads: from the next sample"
        }),
        "nothing explained the empty result"
    );
}

#[test]
fn the_note_tells_the_live_edge_apart_from_a_scrub_back() {
    // Two different empty expansions with two different remedies: wait one
    // interval, or scrub forward. One message for both sends half the readers
    // the wrong way.
    let mut app = App::new(600);
    let mut old = sample_with_threads();
    old.tasks = None;
    app.push(old);
    app.push(sample_with_threads());
    app.show_threads = true;

    assert_eq!(app.thread_note(), None, "the live sample has threads");
    app.history.scrub(-1);
    assert_eq!(
        app.thread_note(),
        Some(if cfg!(target_os = "macos") {
            "threads: not read on macOS"
        } else {
            "threads: not collected this far back"
        }),
        "a sample from before the view was on did not say so"
    );
}

#[test]
fn the_tree_and_the_groups_say_they_are_not_expanding_anything() {
    // Both order rows by something other than "this process, then its
    // threads" — the tree by parentage, a group by a name folding several
    // processes — so neither splices them. A key that silently does nothing is
    // the ambiguity this note exists to remove.
    let mut app = App::new(600);
    app.push(sample_with_threads());
    app.select_delta(1);
    app.toggle_threads();
    assert_eq!(app.thread_note(), None);

    app.tree = true;
    assert_eq!(app.thread_note(), Some("threads: not shown in the tree"));
    assert!(
        !app.visible_rows().iter().any(|r| r.is_thread()),
        "threads were spliced into the tree, between a process and its children"
    );

    app.tree = false;
    app.group = true;
    assert_eq!(app.thread_note(), Some("threads: not shown while grouped"));
    assert!(!app.visible_rows().iter().any(|r| r.is_thread()));
}

#[test]
fn the_thread_ratchet_lets_go_after_the_view_has_been_off_a_while() {
    // Unlike the IO ratchet, which never releases and costs one extra read per
    // process. This costs 3.1us per thread and 17 bytes of every retained
    // sample per thread, so one keypress must not be a life sentence.
    let mut app = App::new(600);
    app.push(sample_with_threads());
    app.toggle_threads();
    assert!(
        app.needs().asked(crate::collect::Source::Threads),
        "the key did not start collection"
    );

    app.toggle_threads();
    for _ in 0..30 {
        app.push(sample_with_threads());
    }
    assert!(
        app.needs().asked(crate::collect::Source::Threads),
        "collection stopped while a reader could still scrub back over it"
    );
    for _ in 0..40 {
        app.push(sample_with_threads());
    }
    assert!(
        !app.needs().asked(crate::collect::Source::Threads),
        "collection never stopped after the view was turned off"
    );
}

#[test]
fn the_threads_stay_on_screen_when_their_process_is_not_the_first_row() {
    // The offset pins the selected row to the bottom visible line, and thread
    // rows are spliced in immediately after it — so on any table longer than
    // the panel, every one of them landed off-screen and `y` did nothing
    // visible. Every fixture above fits on one screen, which is why they all
    // passed while the feature did not work on a real machine.
    let mut app = App::new(600);
    let mut s = sample_with_threads();
    // Thirty processes, all busier than postgres, so it sorts well down the
    // list rather than to the top.
    for i in 0..30 {
        s.procs.push(proc_named(5000 + i, "filler", 90.0, 1 << 20));
    }
    app.push(s);
    app.selected = Some(crate::app::Watched::Process {
        pid: 4021,
        started: Some(0),
        name: std::sync::Arc::from("postgres"),
    });
    app.toggle_threads();

    let frame = rows(&app, 120, 14);
    let shown = frame.join("\n");
    assert!(
        shown.contains("walwriter") && shown.contains("bgwriter"),
        "the threads were drawn off-screen:\n{shown}"
    );
    assert!(
        shown.contains("postgres"),
        "the process itself scrolled away:\n{shown}"
    );
}

#[test]
fn a_task_predicate_refuses_in_both_directions_when_nothing_was_collected() {
    // `!hit` would otherwise turn "no threads were collected" into `task != R`
    // matching every process, as if all their threads had been inspected and
    // none was running. The numeric path refuses both directions for a figure
    // the platform could not read, and this is the same refusal.
    let mut app = App::new(600);
    let mut s = sample_with_threads();
    s.tasks = None;
    app.push(s);

    app.filter = "task = R".into();
    assert!(app.visible_rows().is_empty(), "`=` matched without data");
    app.filter = "task != R".into();
    assert!(
        app.visible_rows().is_empty(),
        "`!=` matched every process without data to justify it"
    );
}

#[test]
fn a_blocked_single_threaded_process_is_found_by_the_same_predicate() {
    // Single-threaded processes are not collected — a process *is* its only
    // thread — and they are the commonest contributor to `BLOCKED`. Without a
    // fallback, the workflow the README documents returns nothing for exactly
    // the case a reader is most likely to be chasing.
    let mut app = App::new(600);
    let mut s = sample_with_threads();
    s.procs.push(ProcSample {
        state: 'D',
        ..proc_named(4300, "dd", 0.0, 1 << 20)
    });
    app.push(s);

    app.filter = "task = D".into();
    let rows = app.visible_rows();
    let mut names: Vec<&str> = rows.iter().map(|r| r.proc.name.as_ref()).collect();
    names.sort_unstable();
    assert_eq!(
        names,
        vec!["dd", "postgres"],
        "the blocked single-threaded process was not found"
    );

    // And it is not matched for a state it is not in.
    app.filter = "task = R".into();
    let rows = app.visible_rows();
    assert_eq!(
        rows.iter()
            .map(|r| r.proc.name.as_ref())
            .collect::<Vec<_>>(),
        vec!["postgres"],
        "a single-threaded process matched the wrong state"
    );
}

#[test]
fn a_cadence_of_one_means_every_sample_including_the_first() {
    use crate::collect::{Needs, Source};
    // The off-by-one this shape invites: `tick % every == 0` is true at zero,
    // so a source read every sample must not be read every sample *but the
    // first*, and a source on a long cadence must be read at startup rather
    // than a minute into the run.
    for tick in 0..5u64 {
        assert!(
            Needs::at(tick).due(Source::Io),
            "a source read every sample was skipped at tick {tick}"
        );
    }
    assert!(
        Needs::at(0).due(Source::ClockPolicies),
        "not read at startup"
    );
    assert!(!Needs::at(1).due(Source::ClockPolicies));
    assert!(!Needs::at(59).due(Source::ClockPolicies));
    assert!(Needs::at(60).due(Source::ClockPolicies), "never read again");
}

#[test]
fn a_source_that_is_due_but_unwanted_is_still_not_read() {
    use crate::collect::{Needs, Source};
    // Two separate questions. Collapsing them would make a cadence into a
    // reason to read something nobody asked for.
    let n = Needs::at(0);
    assert!(n.due(Source::Io), "the fixture does not test the case");
    assert!(!n.wants(Source::Io), "an unwanted source was gathered");
    assert!(n.with(Source::Io).wants(Source::Io));
}

#[test]
fn the_budget_gives_up_the_most_expensive_source_first() {
    use crate::collect::{Needs, Source};
    // Per unit, and the ordering does not change with the count — which is
    // what makes this answerable before the collector has walked anything.
    assert_eq!(
        Needs::NONE
            .with(Source::Io)
            .with(Source::Threads)
            .costliest(),
        Some(Source::Io),
        "per-process IO is dearer per unit than a thread and was not chosen"
    );
    // Not the clock policy walk, though its per-sample figure is the biggest
    // number here. It is a directory listing once a minute: it cannot be why a
    // sample ran long, so giving it up would cost a figure and fix nothing.
    // Comparing it against a per-*unit* cost is comparing two different
    // quantities, and the first version of this did exactly that and chose it
    // over per-process IO on four hundred processes.
    assert_eq!(
        Needs::NONE
            .with(Source::Io)
            .with(Source::ClockPolicies)
            .costliest(),
        Some(Source::Io),
        "a fixed once-a-minute cost was chosen over one that scales"
    );
    assert_eq!(Needs::NONE.costliest(), None, "nothing to give up");
}

#[test]
fn one_slow_sample_does_not_withdraw_anything() {
    use std::time::Duration;
    // A page fault, a scheduler decision, another process finishing. Pulling a
    // column for one of those would be its own kind of noise, and on a busy box
    // it would happen constantly.
    let mut app = App::new(600);
    app.toggle_threads();
    app.spent(Duration::from_millis(900), Duration::from_secs(1));
    assert!(
        app.withheld().is_empty(),
        "one slow sample withdrew a source"
    );

    // …and a fast one resets the count, so three *scattered* slow samples are
    // not three strikes.
    app.spent(Duration::from_millis(900), Duration::from_secs(1));
    app.spent(Duration::from_millis(1), Duration::from_secs(1));
    app.spent(Duration::from_millis(900), Duration::from_secs(1));
    app.spent(Duration::from_millis(900), Duration::from_secs(1));
    assert!(
        app.withheld().is_empty(),
        "scattered slow samples were counted as consecutive ones"
    );
}

#[test]
fn sustained_over_budget_sampling_withdraws_a_source_and_says_which() {
    use crate::collect::Source;
    use std::time::Duration;
    let mut app = App::new(600);
    app.push(sample_with_threads());
    app.select_delta(1);
    app.toggle_threads();
    assert!(app.needs().asked(Source::Threads));
    assert!(app.needs().asked(Source::Io), "io is on by default");

    for _ in 0..3 {
        app.spent(Duration::from_millis(900), Duration::from_secs(1));
    }
    // Per-process IO, not threads: it is dearer *per unit*, and there are more
    // processes than multi-threaded ones. The most expensive thing goes first,
    // which is not the most recently added thing.
    assert_eq!(
        app.withheld(),
        [Source::Io],
        "the costliest source was not the one given up"
    );
    assert!(
        !app.needs().asked(Source::Io),
        "a withheld source was still collected"
    );
    assert!(
        app.needs().asked(Source::Threads),
        "everything was dropped at once rather than one at a time"
    );

    // Still over budget, so the next one goes too.
    for _ in 0..3 {
        app.spent(Duration::from_millis(900), Duration::from_secs(1));
    }
    assert_eq!(app.withheld(), [Source::Io, Source::Threads]);

    // Named on screen. A budget that silently dropped a figure would be the
    // objection to having a budget at all.
    let frame = rows(&app, 200, 20).join("\n");
    assert!(
        frame.contains("per-process disk IO and threads withheld, sampling was over budget"),
        "nothing said what stopped being measured:\n{frame}"
    );
}

#[test]
fn asking_for_a_withheld_source_again_gets_it_back() {
    use crate::collect::Source;
    use std::time::Duration;
    // The reader insisting. If it goes over budget again it will be given up
    // again, which is the honest answer: the machine cannot afford it at this
    // interval, and `--interval` is what acts on that.
    let mut app = App::new(600);
    for _ in 0..3 {
        app.spent(Duration::from_millis(900), Duration::from_secs(1));
    }
    assert_eq!(app.withheld(), [Source::Io]);

    app.toggle_io(); // off
    app.toggle_io(); // and on again, by name
    assert!(
        app.withheld().is_empty(),
        "asking for it again did not clear the withdrawal"
    );
    assert!(app.needs().asked(Source::Io));
}
