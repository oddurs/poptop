//! The export formats, held to what poptop says they are.
//!
//! `--schema` is a promise to every consumer that parses `--export`. These
//! tests read the schema back from the binary and check real output against
//! it, keep a golden copy so a change to it shows in review, and read the line
//! format back to the same values the JSON carries.

// A test crate: every helper here runs inside a test, where a panic is the
// failure being reported.
#![allow(clippy::unwrap_used)]

mod common;

use common::{Home, log_a_sample, logged_day};
use std::collections::BTreeMap;
use std::path::Path;
use std::process::Stdio;

/// Just enough JSON to check poptop's output: no dependency for a test.
#[derive(Clone, Debug, PartialEq)]
enum Json {
    Null,
    Bool(bool),
    /// The number's text as written, so an integer can be told from a float.
    Num(String),
    Str(String),
    Arr(Vec<Json>),
    /// Keys in the order written, which the schema's field order should match.
    Obj(Vec<(String, Json)>),
}

impl Json {
    fn parse(text: &str) -> Json {
        let mut p = Parser {
            b: text.as_bytes(),
            at: 0,
        };
        let v = p.value();
        p.ws();
        assert_eq!(p.at, p.b.len(), "trailing bytes after a JSON value");
        v
    }

    fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Obj(kv) => kv.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    fn str(&self) -> &str {
        match self {
            Json::Str(s) => s,
            other => panic!("not a string: {other:?}"),
        }
    }
}

struct Parser<'a> {
    b: &'a [u8],
    at: usize,
}

impl Parser<'_> {
    fn ws(&mut self) {
        while self.at < self.b.len() && self.b[self.at].is_ascii_whitespace() {
            self.at += 1;
        }
    }

    fn eat(&mut self, c: u8) {
        self.ws();
        assert_eq!(
            self.b.get(self.at),
            Some(&c),
            "expected `{}` at byte {}",
            c as char,
            self.at
        );
        self.at += 1;
    }

    fn value(&mut self) -> Json {
        self.ws();
        match self.b[self.at] {
            b'n' => self.word("null", Json::Null),
            b't' => self.word("true", Json::Bool(true)),
            b'f' => self.word("false", Json::Bool(false)),
            b'"' => Json::Str(self.string()),
            b'[' => {
                self.at += 1;
                let mut items = Vec::new();
                self.ws();
                if self.b[self.at] == b']' {
                    self.at += 1;
                    return Json::Arr(items);
                }
                loop {
                    items.push(self.value());
                    self.ws();
                    self.at += 1;
                    match self.b[self.at - 1] {
                        b',' => continue,
                        b']' => return Json::Arr(items),
                        c => panic!("`{}` in an array at byte {}", c as char, self.at),
                    }
                }
            }
            b'{' => {
                self.at += 1;
                let mut kv = Vec::new();
                self.ws();
                if self.b[self.at] == b'}' {
                    self.at += 1;
                    return Json::Obj(kv);
                }
                loop {
                    self.ws();
                    let k = self.string();
                    self.eat(b':');
                    kv.push((k, self.value()));
                    self.ws();
                    self.at += 1;
                    match self.b[self.at - 1] {
                        b',' => continue,
                        b'}' => return Json::Obj(kv),
                        c => panic!("`{}` in an object at byte {}", c as char, self.at),
                    }
                }
            }
            _ => {
                let start = self.at;
                while self.at < self.b.len()
                    && matches!(
                        self.b[self.at],
                        b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9'
                    )
                {
                    self.at += 1;
                }
                let n = std::str::from_utf8(&self.b[start..self.at]).unwrap();
                assert!(
                    n.parse::<f64>().is_ok_and(f64::is_finite),
                    "`{n}` is not a JSON number"
                );
                Json::Num(n.to_string())
            }
        }
    }

    fn word(&mut self, w: &str, v: Json) -> Json {
        assert!(self.b[self.at..].starts_with(w.as_bytes()));
        self.at += w.len();
        v
    }

    fn string(&mut self) -> String {
        self.eat(b'"');
        let mut out = String::new();
        loop {
            let c = self.b[self.at];
            self.at += 1;
            match c {
                b'"' => return out,
                b'\\' => {
                    let e = self.b[self.at];
                    self.at += 1;
                    match e {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let hex = std::str::from_utf8(&self.b[self.at..self.at + 4]).unwrap();
                            self.at += 4;
                            out.push(
                                char::from_u32(u32::from_str_radix(hex, 16).unwrap()).unwrap(),
                            );
                        }
                        e => panic!("`\\{}` is not a JSON escape", e as char),
                    }
                }
                c if c < 0x20 => panic!("a raw control byte {c:#x} inside a JSON string"),
                _ => {
                    // A whole UTF-8 sequence, however long.
                    let start = self.at - 1;
                    let len = match c {
                        0xf0.. => 4,
                        0xe0.. => 3,
                        0xc0.. => 2,
                        _ => 1,
                    };
                    self.at = start + len;
                    out.push_str(std::str::from_utf8(&self.b[start..self.at]).unwrap());
                }
            }
        }
    }
}

/// One field as `--schema` declares it.
struct Field {
    name: String,
    ty: String,
    optional: bool,
}

/// Every record the schema declares, by name.
fn schema(home: &Home) -> BTreeMap<String, Vec<Field>> {
    let text = home.run(&["--schema"]).ok().out;
    let root = Json::parse(&text);
    let Some(Json::Obj(records)) = root.get("records") else {
        panic!("no records in the schema");
    };
    records
        .iter()
        .map(|(name, fields)| {
            let Json::Arr(fields) = fields else {
                panic!("{name} is not a list of fields");
            };
            let fields = fields
                .iter()
                .map(|f| Field {
                    name: f.get("name").unwrap().str().to_string(),
                    ty: f.get("type").unwrap().str().to_string(),
                    optional: f.get("optional") == Some(&Json::Bool(true)),
                })
                .collect();
            (name.clone(), fields)
        })
        .collect()
}

/// Every way `v` fails to be a `ty`, with the path to it.
fn check(
    schema: &BTreeMap<String, Vec<Field>>,
    path: &str,
    ty: &str,
    v: &Json,
    bad: &mut Vec<String>,
) {
    let mut wrong = |what: &str| bad.push(format!("{path}: {what}, found {v:?}"));
    if let Some(inner) = ty.strip_prefix('[').and_then(|t| t.strip_suffix(']')) {
        // `[T]` or `[T; n]`.
        let (elem, len) = match inner.split_once("; ") {
            Some((e, n)) => (e, Some(n.parse::<usize>().unwrap())),
            None => (inner, None),
        };
        let Json::Arr(items) = v else {
            return wrong(&format!("expected a list of {elem}"));
        };
        if let Some(n) = len.filter(|n| *n != items.len()) {
            return wrong(&format!("expected exactly {n} items"));
        }
        for (i, item) in items.iter().enumerate() {
            check(schema, &format!("{path}[{i}]"), elem, item, bad);
        }
        return;
    }
    match (ty, v) {
        ("integer", Json::Num(n)) if !n.contains(['.', 'e', 'E']) => {}
        ("integer", _) => wrong("expected an integer"),
        ("number" | "epoch" | "duration", Json::Num(_)) => {}
        ("number" | "epoch" | "duration", _) => wrong("expected a number"),
        ("boolean", Json::Bool(_)) => {}
        ("boolean", _) => wrong("expected true or false"),
        ("string", Json::Str(_)) => {}
        ("string", _) => wrong("expected a string"),
        (record, Json::Obj(kv)) => {
            let Some(fields) = schema.get(record) else {
                return wrong(&format!("the schema has no type `{record}`"));
            };
            let names: Vec<&str> = kv.iter().map(|(k, _)| k.as_str()).collect();
            let want: Vec<&str> = fields.iter().map(|f| f.name.as_str()).collect();
            if names != want {
                return wrong(&format!("expected exactly the fields {want:?}"));
            }
            for (f, (_, v)) in fields.iter().zip(kv) {
                let at = format!("{path}.{}", f.name);
                match v {
                    Json::Null if f.optional => {}
                    Json::Null => bad.push(format!(
                        "{at}: null in a field the schema says is always there"
                    )),
                    v => check(schema, &at, &f.ty, v, bad),
                }
            }
        }
        (record, _) => wrong(&format!("expected a `{record}` record")),
    }
}

fn conforms(schema: &BTreeMap<String, Vec<Field>>, json_lines: &str) -> usize {
    let mut n = 0;
    for line in json_lines.lines() {
        let mut bad = Vec::new();
        check(schema, "sample", "Sample", &Json::parse(line), &mut bad);
        assert!(
            bad.is_empty(),
            "export disagrees with --schema:\n{}",
            bad.join("\n")
        );
        n += 1;
    }
    n
}

#[test]
fn a_live_sample_is_what_the_schema_says() {
    let home = Home::new();
    let schema = schema(&home);
    let out = home.run(&["--export=json", "--interval=200ms"]).ok().out;
    assert_eq!(conforms(&schema, &out), 1);
}

#[test]
fn a_recorded_day_is_what_the_schema_says() {
    let home = Home::new();
    let schema = schema(&home);
    log_a_sample(&home);
    log_a_sample(&home);
    let day = logged_day(&home);
    let out = home.run(&["--export", "json", &day]).ok().out;
    assert_eq!(conforms(&schema, &out), 2);
}

#[test]
fn the_validator_rejects_what_the_schema_does_not_say() {
    // A check that cannot fail is not one. Each of these is a real sample
    // with one thing changed.
    let home = Home::new();
    let schema = schema(&home);
    let line = home.run(&["--export=json", "--interval=200ms"]).ok().out;
    let good = Json::parse(line.trim());
    let Json::Obj(kv) = &good else { panic!() };
    let with = |key: &str, v: Json| {
        let mut kv = kv.clone();
        kv.iter_mut().find(|(k, _)| k == key).unwrap().1 = v;
        Json::Obj(kv)
    };
    for (what, bad) in [
        ("null where required", with("cpu_total", Json::Null)),
        ("a number as a record", with("mem", Json::Num("1".into()))),
        (
            "a string as a number",
            with("cpu_total", Json::Str("1".into())),
        ),
        (
            "a load of two",
            with("load", Json::Arr(vec![Json::Num("1".into()); 2])),
        ),
        ("a missing field", {
            let mut kv = kv.clone();
            kv.retain(|(k, _)| k != "mem");
            Json::Obj(kv)
        }),
        ("an extra field", {
            let mut kv = kv.clone();
            kv.push(("surprise".into(), Json::Null));
            Json::Obj(kv)
        }),
    ] {
        let mut found = Vec::new();
        check(&schema, "sample", "Sample", &bad, &mut found);
        assert!(!found.is_empty(), "{what} passed");
    }
}

#[test]
fn the_schema_is_the_one_in_review() {
    // A consumer's parser is written against this. Changing it is allowed
    // and is a decision: regenerate with
    //
    //     POPTOP_BLESS=1 cargo test --test export the_schema
    //
    // and the diff to tests/golden/schema.json is the change a reviewer sees.
    // See "Stability" in the README for which changes break a consumer.
    let golden = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/schema.json");
    let now = Home::new().run(&["--schema"]).ok().out;
    if std::env::var_os("POPTOP_BLESS").is_some() {
        std::fs::create_dir_all(golden.parent().unwrap()).unwrap();
        std::fs::write(&golden, &now).unwrap();
    }
    let was = std::fs::read_to_string(&golden).unwrap_or_default();
    assert!(
        was == now,
        "--schema changed. If that is meant, run POPTOP_BLESS=1 cargo test --test export \
         the_schema and commit tests/golden/schema.json"
    );
}

/// A line-format stream, read by its stated grammar. Panics where the stream
/// breaks it.
///
///   stream  = { header | row }
///   header  = "#" label { TAB column } NL      once per label, before its rows
///   row     = label { TAB value } NL           as many values as its header
///   label   = "sample" { "." field }
///   value   = "-" (absent) | a number | "1" / "0" (a boolean) | text
///
/// Text holds no TAB, CR or LF. Every sample ends with its own `sample` row,
/// after the rows of what it contains.
fn read_lines(text: &str) -> Vec<Vec<(String, BTreeMap<String, String>)>> {
    let mut headers: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    let mut samples = Vec::new();
    let mut rows = Vec::new();
    for line in text.lines() {
        assert!(!line.contains('\r'), "a CR inside a line: {line:?}");
        if let Some(h) = line.strip_prefix('#') {
            let mut cols = h.split('\t');
            let label = cols.next().unwrap();
            let cols: Vec<&str> = cols.collect();
            assert!(!cols.is_empty(), "`{label}` has a header and no columns");
            assert!(
                headers.insert(label, cols).is_none(),
                "`{label}` has two headers"
            );
            continue;
        }
        let mut vals = line.split('\t');
        let label = vals.next().unwrap();
        assert!(
            label == "sample" || label.starts_with("sample."),
            "{line:?}"
        );
        let cols = headers
            .get(label)
            .unwrap_or_else(|| panic!("a `{label}` row before its header"));
        let vals: Vec<&str> = vals.collect();
        assert_eq!(vals.len(), cols.len(), "`{label}`: {line:?}");
        let row = cols
            .iter()
            .map(|c| c.to_string())
            .zip(vals.iter().map(|v| v.to_string()));
        rows.push((label.to_string(), row.collect()));
        if label == "sample" {
            samples.push(std::mem::take(&mut rows));
        }
    }
    assert!(rows.is_empty(), "rows after the last sample");
    samples
}

/// The JSON node a line-format row describes: the label's path under the
/// sample, through the row's `i` wherever a list is crossed.
fn node<'a>(sample: &'a Json, label: &str, i: Option<&str>) -> &'a Json {
    let mut at = sample;
    for part in label.split('.').skip(1) {
        at = at
            .get(part)
            .unwrap_or_else(|| panic!("no `{part}` in {label}"));
        if let (Json::Arr(items), Some(i)) = (at, i)
            && items.first().is_some_and(|x| matches!(x, Json::Obj(_)))
        {
            at = &items[i.parse::<usize>().unwrap()];
        }
    }
    at
}

/// Whether a line-format value says what the JSON value says.
fn same(line: &str, json: &Json) -> bool {
    match json {
        Json::Null => line == "-",
        Json::Bool(b) => line == if *b { "1" } else { "0" },
        Json::Num(n) => line.parse::<f64>().ok() == n.parse::<f64>().ok(),
        // Lossy on purpose, and documented: a TAB, CR or LF becomes a space.
        Json::Str(s) => line == s.replace(['\t', '\r', '\n'], " "),
        _ => false,
    }
}

#[test]
fn the_line_format_reads_back_to_what_the_json_says() {
    let home = Home::new();
    log_a_sample(&home);
    log_a_sample(&home);
    let day = logged_day(&home);
    let json: Vec<Json> = home
        .run(&["--export", "json", &day])
        .ok()
        .out
        .lines()
        .map(Json::parse)
        .collect();
    let lines = read_lines(&home.run(&["--export", "line", &day]).ok().out);
    assert_eq!(lines.len(), json.len(), "a different number of samples");
    let mut compared = 0;
    for (rows, sample) in lines.iter().zip(&json) {
        for (label, row) in rows {
            let at = node(sample, label, row.get("i").map(String::as_str));
            for (col, v) in row {
                if col == "i" {
                    continue;
                }
                let want = match at {
                    Json::Arr(items) => &items[col.parse::<usize>().unwrap()],
                    obj => obj
                        .get(col)
                        .unwrap_or_else(|| panic!("{label}.{col} is not in the JSON")),
                };
                assert!(
                    same(v, want),
                    "{label}.{col}: line says {v:?}, JSON says {want:?}"
                );
                compared += 1;
            }
        }
    }
    assert!(compared > 50, "only {compared} values compared");
}

// ── a live feed ───────────────────────────────────────────────────────────
// `--export --follow` is the one output path with a clock in it, so these
// read it as a consumer does: from the pipe, while it runs, and stopped the
// three ways it can be stopped.

unsafe extern "C" {
    fn kill(pid: i32, sig: i32) -> i32;
}

const SIGTERM: i32 = 15;
const SIGHUP: i32 = 1;

/// Lines from a running feed, one at a time, with a bound on the wait.
///
/// A thread and a channel rather than a blocking read, so a feed that stops
/// writing fails the test with what it had written instead of hanging until
/// the harness gives up.
struct Feed {
    child: std::process::Child,
    lines: std::sync::mpsc::Receiver<String>,
}

impl Feed {
    fn start(home: &Home, args: &[&str]) -> Feed {
        let mut child = home
            .cmd(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("cannot start poptop");
        let out = child.stdout.take().unwrap();
        let (tx, lines) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            for line in std::io::BufRead::lines(std::io::BufReader::new(out)) {
                let Ok(line) = line else { return };
                if tx.send(line).is_err() {
                    return;
                }
            }
        });
        Feed { child, lines }
    }

    /// The next line, or what the feed had done instead of writing one.
    fn line(&mut self) -> String {
        match self.lines.recv_timeout(std::time::Duration::from_secs(30)) {
            Ok(l) => l,
            Err(_) => panic!("the feed stopped writing: {:?}", self.child.try_wait()),
        }
    }

    /// Send a signal to the feed.
    ///
    /// The call rather than `kill(1)`: the binary is not in every container a
    /// test runs in, and a test that fails because an image is slim is a test
    /// nobody trusts.
    fn signal(&self, sig: i32) {
        // SAFETY: a signal to our own child, which has not been waited on, so
        // its pid is still its own.
        let sent = unsafe { kill(self.child.id() as i32, sig) };
        assert_eq!(sent, 0, "could not signal the feed");
    }

    /// Wait for it to end, and say how.
    fn ends(&mut self) -> std::process::ExitStatus {
        // Bounded, so a feed that ignores a signal is a failure rather than a
        // test that never returns.
        let until = std::time::Instant::now() + std::time::Duration::from_secs(30);
        loop {
            if let Some(s) = self.child.try_wait().unwrap() {
                return s;
            }
            assert!(std::time::Instant::now() < until, "the feed did not end");
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }
}

#[test]
fn a_live_feed_writes_a_record_an_interval_until_it_is_signalled() {
    let home = Home::new();
    let mut feed = Feed::start(&home, &["--export=json", "--follow", "--interval=200ms"]);
    let at: Vec<f64> = (0..5)
        .map(|_| {
            // A record, not a fragment: whole, parseable JSON, flushed as it
            // was taken rather than left in the buffer until 8 KB had piled up.
            record_at(&feed.line())
        })
        .collect();
    // The schedule it claims. Measured across the whole run rather than
    // between neighbours: one late sample on a loaded runner is noise, and a
    // cadence that drifts shows up in the total.
    let each = (at[4] - at[0]) / 4.0;
    assert!(
        (0.15..0.45).contains(&each),
        "200ms samples arrived {each:.3}s apart: {at:?}"
    );
    feed.signal(SIGTERM);
    let s = feed.ends();
    assert_eq!(s.code(), Some(0), "SIGTERM did not end the feed cleanly");
}

#[test]
fn a_feed_ends_quietly_when_its_reader_goes_away() {
    // `poptop --export=json --follow | head -3`, which is how anybody looks
    // at a feed for the first time. The pipe closing is not an error.
    let home = Home::new();
    let mut feed = Feed::start(&home, &["--export=json", "--follow", "--interval=200ms"]);
    for _ in 0..2 {
        feed.line();
    }
    drop(std::mem::replace(
        &mut feed.lines,
        std::sync::mpsc::channel().1,
    ));
    let s = feed.ends();
    assert_eq!(s.code(), Some(0), "a closed pipe was not a clean end");
    let mut err = String::new();
    std::io::Read::read_to_string(feed.child.stderr.as_mut().unwrap(), &mut err).unwrap();
    assert!(
        err.is_empty(),
        "it complained about the reader leaving:\n{err}"
    );
}

#[test]
fn a_feed_stops_itself_after_for() {
    let home = Home::new();
    let began = std::time::Instant::now();
    let out = home
        .cmd(&[
            "--export=line",
            "--follow",
            "--interval=200ms",
            "--for",
            "1s",
        ])
        .output()
        .expect("cannot start poptop");
    let took = began.elapsed();
    assert_eq!(out.status.code(), Some(0));
    assert!(
        (std::time::Duration::from_millis(900)..std::time::Duration::from_secs(20)).contains(&took),
        "--for 1s took {took:?}"
    );
    let text = String::from_utf8(out.stdout).unwrap();
    let rows = text
        .lines()
        .filter(|l| l.starts_with("sample.cpu_per_core\t"))
        .count();
    assert!(
        (2..=8).contains(&rows),
        "{rows} records in a second:\n{text}"
    );
    // The header rule under `--follow`: once, at the top of the stream, as a
    // recorded day writes it — not once a sample, which would be four copies
    // of every header in a second and a reader with four column maps to
    // choose between.
    for label in ["#sample.cpu_per_core", "#sample.mem"] {
        let n = text.lines().filter(|l| l.starts_with(label)).count();
        assert_eq!(n, 1, "{n} copies of `{label}`");
    }
    let first = text.lines().next().unwrap();
    assert!(first.starts_with('#'), "the stream opens with `{first}`");
}

#[test]
fn a_hangup_without_a_terminal_reopens_rather_than_stopping() {
    // 0150. SIGHUP means two opposite things: on a terminal it is the
    // terminal going away, and off one it is what `logrotate` means by it —
    // "I have moved your file, open it again". A recorder that exited on the
    // second would die at 03:00 on the night the rotation runs.
    let home = Home::new();
    log_a_sample(&home);
    let day = logged_day(&home);
    let path = home
        .state()
        .join("poptop")
        .join("log")
        .join(format!("poptop-{}", day.replace('-', "")));
    let mut feed = Feed::start(&home, &["--export=json", &day, "--follow"]);
    let first = record_at(&feed.line());

    // The rotation: the file this follower is reading is moved away, and a
    // new one appears under the same name.
    std::fs::rename(&path, path.with_extension("1")).unwrap();
    feed.signal(SIGHUP);
    log_a_sample(&home);

    let next = record_at(&feed.line());
    assert!(
        next > first,
        "the follower repeated a sample across the rotation: {next} after {first}"
    );
    assert!(
        feed.child.try_wait().unwrap().is_none(),
        "a hangup off a terminal stopped the feed instead of reopening"
    );
    // And SIGTERM still ends it.
    feed.signal(SIGTERM);
    assert_eq!(feed.ends().code(), Some(0));
}

#[test]
fn a_hangup_tells_a_live_feed_there_is_nothing_to_reopen() {
    // The other half: a feed of the machine writes to stdout, which poptop
    // does not own and cannot reopen. It carries on, and says so once rather
    // than leaving the operator to wonder what their rotation did.
    let home = Home::new();
    let mut feed = Feed::start(&home, &["--export=json", "--follow", "--interval=200ms"]);
    feed.line();
    feed.signal(SIGHUP);
    let after = record_at(&feed.line());
    assert!(after > 0.0);
    assert!(
        feed.child.try_wait().unwrap().is_none(),
        "a hangup stopped a live feed"
    );
    feed.signal(SIGTERM);
    assert_eq!(feed.ends().code(), Some(0));
    let mut err = String::new();
    std::io::Read::read_to_string(feed.child.stderr.as_mut().unwrap(), &mut err).unwrap();
    assert!(err.contains("nothing to reopen"), "it said: {err:?}");
}

#[test]
fn a_day_is_followed_from_one_process_while_another_writes_it() {
    // The shape this exists for: something subscribes to today's log, and
    // whatever is doing the logging is a different process entirely.
    let home = Home::new();
    log_a_sample(&home);
    let day = logged_day(&home);
    let mut feed = Feed::start(&home, &["--export=json", &day, "--follow"]);

    // What was already recorded comes first: the file is a day, and a
    // consumer that attached at noon wanting the morning has no other way of
    // asking for it.
    let mut at = vec![record_at(&feed.line())];
    for _ in 0..3 {
        log_a_sample(&home);
        at.push(record_at(&feed.line()));
    }
    // In order, and none of them twice.
    for pair in at.windows(2) {
        assert!(
            pair[1] > pair[0],
            "a follower repeated or reordered a sample: {at:?}"
        );
    }
    feed.signal(SIGTERM);
    assert_eq!(
        feed.ends().code(),
        Some(0),
        "SIGTERM did not end the follower"
    );
}

#[test]
fn following_a_day_that_has_nothing_in_it_yet_waits_rather_than_failing() {
    // A follower may be started before the writer is. `--export DATE` on its
    // own is exit 1 with "nothing recorded on", because there is nothing to
    // print and never will be; a follower's answer is to wait.
    let home = Home::new();
    log_a_sample(&home);
    let today = logged_day(&home);
    let home = Home::new();
    let mut feed = Feed::start(&home, &["--export=json", &today, "--follow", "--for", "1s"]);
    let s = feed.ends();
    assert_eq!(s.code(), Some(0), "an empty day was not waited for");
    let mut err = String::new();
    std::io::Read::read_to_string(feed.child.stderr.as_mut().unwrap(), &mut err).unwrap();
    assert!(err.is_empty(), "it complained about an empty day:\n{err}");
}

/// The `at` of one JSON record.
fn record_at(line: &str) -> f64 {
    match Json::parse(line).get("at").expect("a record with no `at`") {
        Json::Num(n) => n.parse().unwrap(),
        other => panic!("`at` is {other:?}"),
    }
}
