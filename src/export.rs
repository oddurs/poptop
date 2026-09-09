//! Machine-readable output: JSON, a line format, and the schema itself.
//!
//! `--once` prints a fixed set of lines for a human who is scripting around
//! them. This is the other thing — a tool you can build on rather than one you
//! watch. atop has `-P` and `-J` over a documented label set; the documentation
//! is its man page, and the format is positional.
//!
//! poptop's advantage here is that it already has a schema. Every record and
//! field is declared once for the store's codec, the walk over them is
//! generated from the same list, and this module is two visitors over that
//! walk. A field added to a sample reaches both formats or fails to compile —
//! machine-readable output that silently stops mentioning a metric is worse
//! than none, and a hand-written list of what to print is exactly that failure
//! waiting to happen.
//!
//! So the schema can be *emitted* rather than described, and a consumer can ask
//! poptop what it reports instead of being told in prose.

use crate::persist::{Emit, Ty, Visit};
use crate::sample::Sample;
use std::fmt::Write as _;

/// What a field is measured in.
///
/// Not derivable from the type — `u64` is bytes here and a count there — so it
/// is a table, and the table is checked against the schema by a test rather
/// than by hope. A unit that says nothing useful is [`Unit::None`]; a field
/// with no entry at all is a test failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
    /// A count of things: processes, calls, nodes.
    Count,
    /// A count over the interval, already divided by it.
    PerSecond,
    Bytes,
    BytesPerSecond,
    /// 0..100, not 0..1.
    Percent,
    Seconds,
    Milliseconds,
    /// Seconds since the epoch.
    Epoch,
    /// Text, or something with no dimension at all.
    None,
}

impl Unit {
    pub fn name(self) -> &'static str {
        match self {
            Unit::Count => "count",
            Unit::PerSecond => "per_second",
            Unit::Bytes => "bytes",
            Unit::BytesPerSecond => "bytes_per_second",
            Unit::Percent => "percent",
            Unit::Seconds => "seconds",
            Unit::Milliseconds => "milliseconds",
            Unit::Epoch => "epoch_seconds",
            Unit::None => "none",
        }
    }
}

/// The unit of `record.field`, or `None` if the table has no entry.
///
/// Matched on the field name within its record, which is what the schema is
/// keyed by. Several records share a field name — `read` is bytes a second on
/// a mount and bytes since boot on a disk — so the record has to be part of
/// the key.
pub fn unit_of(record: &str, field: &str) -> Option<Unit> {
    use Unit::*;
    // Grouped by record, in the order the records are declared. A field
    // reachable from `Sample` and missing here fails
    // `every_field_has_a_unit`, which is the whole reason this is a table and
    // not a guess from the type.
    let by_name = |f: &str| -> Option<Unit> {
        Some(match f {
            // Anything named like this means the same thing in every record it
            // appears in. Listed once rather than repeated per record, because
            // a table with the same answer written eleven times is a table
            // that will eventually disagree with itself.
            "at" => Epoch,
            "uptime" | "age" => Seconds,
            "cpu" | "cpu_total" | "iowait" | "steal" | "guest" | "irq" | "softirq" | "util"
            | "clock_ceiling" | "cpu_max" | "some" | "full" => Percent,
            "rss" | "vsize" | "pss" | "total" | "used" | "free" | "available" | "swap_total"
            | "swap_used" | "dirty" | "slab" | "slab_reclaimable" | "shmem" | "page_tables"
            | "huge_total" | "huge_used" | "file" | "avail" | "memory" | "memory_max" => Bytes,
            "read" | "write" | "rx" | "tx" | "server_read" | "server_write" => BytesPerSecond,
            "await_ms" | "rtt_ms" => Milliseconds,
            "queue" => Count,
            "pid" | "ppid" | "threads" | "nice" | "running" | "blocked" | "procs" | "id"
            | "io_denied" | "tid" | "nprocs" => Count,
            // Deliberately dimensionless. `started` is an opaque token — clock
            // ticks since boot on Linux, epoch microseconds on macOS — and its
            // only job is to tell two processes on a recycled pid apart.
            // Giving it a unit would invite somebody to do arithmetic on it.
            "started" => None,
            "minflt" | "majflt" | "forks" | "ctxt" | "intr" | "pgin" | "pgout" | "swin"
            | "swout" | "oom_kills" | "errors" | "drops" | "retrans" | "listen_drops" | "ops"
            | "client_calls" | "client_retrans" | "server_calls" | "server_hits"
            | "server_misses" | "server_badauth" => PerSecond,
            "name" | "user" | "cmd" | "state" | "container" | "mount" | "server" | "path"
            | "device" | "fstype" => None,
            "io_supported" | "io_collected" => None,
            _ => return Option::None,
        })
    };
    // A handful mean different things in different records, and those are the
    // ones a units table exists for.
    match (record, field) {
        // A container has no unit of its own — its fields have theirs.
        ("NetStat", "links")
        | ("NfsStat", "mounts")
        | ("Sample", "mem" | "disks" | "pressure" | "net" | "filesystems")
        | ("Sample", "tasks" | "exited" | "cgroups" | "nodes" | "nfs")
        | ("ProcSample", "io")
        | ("CgroupStat", "pressure") => Some(None),
        // Pressure's three are the resource each stall is about.
        ("Pressure", "cpu" | "io" | "mem") => Some(None),
        ("CgroupStat", "mem" | "mem_max") => Some(Bytes),
        ("Link", "rx_packets" | "tx_packets") => Some(PerSecond),
        ("DiskStat", "reads" | "writes") => Some(PerSecond),
        ("DiskStat", "read" | "write") => Some(BytesPerSecond),
        ("Sample", "load") => Some(Count),
        ("Sample", "cpu_per_core") => Some(Percent),
        ("CgroupStat", "depth") => Some(Count),
        ("NodeStat", "cpu") => Some(Percent),
        _ => by_name(field),
    }
}

/// The schema this build reports, as JSON.
///
/// Emitted rather than documented. atop's label set lives in its man page,
/// which is a second thing to keep up to date; this is the same list the store
/// writes into every file, so it cannot drift from what is actually collected.
pub fn schema_json() -> String {
    let mut out = String::from("{\n  \"version\": ");
    let _ = write!(out, "{:?},\n  \"records\": {{\n", env!("CARGO_PKG_VERSION"));
    let records = crate::sample::schemas();
    for (i, (name, fields)) in records.iter().enumerate() {
        let _ = writeln!(out, "    {name:?}: [");
        for (j, f) in fields.iter().enumerate() {
            let unit = unit_of(name, &f.name).map_or("unknown", Unit::name);
            let _ = writeln!(
                out,
                "      {{\"name\": {:?}, \"type\": {:?}, \"unit\": {:?}, \"optional\": {}}}{}",
                &*f.name,
                ty_name(&f.ty),
                unit,
                matches!(f.ty, Ty::Opt(_)),
                if j + 1 == fields.len() { "" } else { "," }
            );
        }
        let _ = writeln!(
            out,
            "    ]{}",
            if i + 1 == records.len() { "" } else { "," }
        );
    }
    out.push_str("  }\n}\n");
    out
}

/// A type as a name a consumer can switch on.
fn ty_name(t: &Ty) -> String {
    match t {
        Ty::U8 | Ty::U16 | Ty::U32 | Ty::U64 | Ty::Usize => "integer".into(),
        Ty::I32 | Ty::I64 => "integer".into(),
        Ty::F32 | Ty::F64 => "number".into(),
        Ty::Bool => "boolean".into(),
        Ty::Char | Ty::Str => "string".into(),
        Ty::Time => "epoch".into(),
        Ty::Dur => "duration".into(),
        Ty::Opt(inner) => ty_name(inner),
        Ty::List(inner) => format!("[{}]", ty_name(inner)),
        Ty::Arr(inner, n) => format!("[{}; {n}]", ty_name(inner)),
        Ty::Rec(name) => name.to_string(),
    }
}

/// A sample as JSON.
///
/// Absence is `null`, never a zero. Every other part of poptop keeps "nobody
/// said" and "none happened" apart on screen; a format meant to be computed on
/// has more reason to, not less.
#[derive(Default)]
pub struct Json {
    out: String,
    /// Whether the container being written wants a comma before the next item.
    first: Vec<bool>,
    /// Whether the container being written is a list, whose items have no key.
    list: Vec<bool>,
}

impl Json {
    fn key(&mut self, name: &str) {
        if !self.first.last().copied().unwrap_or(true) {
            self.out.push(',');
        }
        if let Some(f) = self.first.last_mut() {
            *f = false;
        }
        // No key at the root, so the document is an object rather than a
        // dangling pair, and none inside a list, whose items are positional.
        if !self.list.is_empty() && !self.list.last().copied().unwrap_or(false) {
            let _ = write!(self.out, "{name:?}:");
        }
    }

    pub fn finish(self) -> String {
        self.out
    }
}

impl Visit for Json {
    fn num(&mut self, name: &str, v: f64) {
        self.key(name);
        // A NaN or an infinity is not JSON. It is also not a measurement, and
        // `null` is what poptop says for a figure it does not have.
        if v.is_finite() {
            let _ = write!(self.out, "{v}");
        } else {
            self.out.push_str("null");
        }
    }
    fn int(&mut self, name: &str, v: i128) {
        self.key(name);
        let _ = write!(self.out, "{v}");
    }
    fn text(&mut self, name: &str, v: &str) {
        self.key(name);
        self.out.push('"');
        for c in v.chars() {
            match c {
                '"' => self.out.push_str("\\\""),
                '\\' => self.out.push_str("\\\\"),
                '\n' => self.out.push_str("\\n"),
                '\r' => self.out.push_str("\\r"),
                '\t' => self.out.push_str("\\t"),
                // A process name is whatever the kernel had; a control
                // character in it must not end up raw in a JSON string.
                c if (c as u32) < 0x20 => {
                    let _ = write!(self.out, "\\u{:04x}", c as u32);
                }
                c => self.out.push(c),
            }
        }
        self.out.push('"');
    }
    fn flag(&mut self, name: &str, v: bool) {
        self.key(name);
        self.out.push_str(if v { "true" } else { "false" });
    }
    fn absent(&mut self, name: &str) {
        self.key(name);
        self.out.push_str("null");
    }
    fn open(&mut self, name: &str, list: bool) {
        self.key(name);
        self.out.push(if list { '[' } else { '{' });
        self.first.push(true);
        self.list.push(list);
    }
    fn close(&mut self) {
        let list = self.list.pop().unwrap_or(false);
        self.first.pop();
        self.out.push(if list { ']' } else { '}' });
    }
}

/// What a line-format field separator is.
///
/// Tab, not a space: a process name can contain spaces, and a format whose
/// separator appears inside its values is one every consumer gets wrong once.
pub const SEP: char = '\t';

/// The token for a figure the platform did not report.
///
/// Not empty, and not a zero. A bare `-` cannot be confused with a number,
/// because every negative number poptop emits has digits after the sign.
pub const ABSENT: &str = "-";

/// A sample as tab-separated lines, one per record.
///
/// atop's shape — a label, then the fields — because that is what existing
/// tooling parses. The label is the dotted path to the record, so a nested list
/// is `procs.4`, and a consumer can `grep '^procs'` for the process table
/// without parsing anything.
#[derive(Default)]
pub struct Lines {
    out: String,
    /// One frame per open record or list: its label, and the columns and values
    /// collected for it so far.
    ///
    /// A stack rather than one row, because a record's scalar fields are
    /// interleaved with its nested ones — `Sample` has `cpu_total`, then a
    /// list, then `iowait`. Flushing when a child opened split one record into
    /// two rows with different columns under one label and one header, which is
    /// a format no consumer could read.
    stack: Vec<Frame>,
    /// Labels whose column header has already been written.
    seen: Vec<String>,
}

#[derive(Default)]
struct Frame {
    label: String,
    cols: Vec<String>,
    row: Vec<String>,
    /// Whether this frame's children are the elements of a list.
    list: bool,
    /// This frame's position, if it is an element of one.
    ///
    /// Carried down into nested records so `sample.procs.io` can be joined
    /// back to `sample.procs` on the same `i`. Without it the io rows arrive
    /// with nothing on them saying which process they belong to.
    index: Option<String>,
}

impl Lines {
    fn push(&mut self, name: &str, v: String) {
        let Some(f) = self.stack.last_mut() else {
            return;
        };
        // A scalar element of a list — `cpu_per_core`, `load` — keeps the
        // list's own label with one column per position, which is the shape
        // those actually are.
        f.cols.push(name.to_string());
        f.row.push(v);
    }

    fn emit_frame(&mut self, f: Frame) {
        if f.row.is_empty() {
            return;
        }
        // The column names once per label, as a comment. A positional format
        // that does not say what its positions are is atop's, and its
        // documentation is a man page somebody has to keep in step.
        let sep = SEP.to_string();
        if !self.seen.contains(&f.label) {
            self.seen.push(f.label.clone());
            let _ = writeln!(self.out, "#{}{SEP}{}", f.label, f.cols.join(&sep));
        }
        let _ = writeln!(self.out, "{}{SEP}{}", f.label, f.row.join(&sep));
    }

    /// One more sample into the same stream.
    ///
    /// The same `Lines`, so the header block is written once for a whole day
    /// rather than once a sample — a hundred and forty-four copies of it at
    /// the default logging interval, and a reader building a column map from
    /// the header would have to decide which copy it meant.
    pub fn add(&mut self, s: &Sample) {
        s.emit("sample", self);
        while let Some(f) = self.stack.pop() {
            self.emit_frame(f);
        }
    }

    pub fn finish(mut self) -> String {
        while let Some(f) = self.stack.pop() {
            self.emit_frame(f);
        }
        self.out
    }
}

impl Visit for Lines {
    fn num(&mut self, name: &str, v: f64) {
        self.push(name, format!("{v}"));
    }
    fn int(&mut self, name: &str, v: i128) {
        self.push(name, v.to_string());
    }
    fn text(&mut self, name: &str, v: &str) {
        // The separator can never appear inside a value, whatever the kernel
        // had in a process name.
        self.push(name, v.replace([SEP, '\n'], " "));
    }
    fn flag(&mut self, name: &str, v: bool) {
        self.push(name, if v { "1" } else { "0" }.into());
    }
    fn absent(&mut self, name: &str) {
        self.push(name, ABSENT.into());
    }
    fn absent_nested(&mut self, _name: &str) {
        // Nothing. A record that *is* there contributes no column to its
        // parent either — it opens a table of its own — so writing one for the
        // absent case made rows disagree with their own header, and every
        // field after the gap read as its neighbour. A process with no `io` is
        // a process with no row under `sample.procs.io`, which is where a
        // reader looks for it.
    }
    fn open(&mut self, name: &str, list: bool) {
        // An element of a list keeps its parent's label and puts its position
        // in a column. Numbering the *label* gave `sample.procs.0`,
        // `sample.procs.1` … — six hundred labels and six hundred headers for
        // one table, which defeats the only thing this format is for:
        // `grep '^sample.procs'` and read the rows.
        let (label, index) = match self.stack.last() {
            Some(f) if f.list => (f.label.clone(), Some(name.to_string())),
            Some(f) => (format!("{}.{name}", f.label), f.index.clone()),
            None => (name.to_string(), None),
        };
        let mut frame = Frame {
            label,
            list,
            index: index.clone(),
            ..Frame::default()
        };
        if let Some(i) = index {
            frame.cols.push("i".into());
            frame.row.push(i);
        }
        self.stack.push(frame);
    }
    fn close(&mut self) {
        if let Some(f) = self.stack.pop() {
            self.emit_frame(f);
        }
    }
}

/// One sample, in whichever format was asked for.
pub fn sample_json(s: &Sample) -> String {
    let mut j = Json::default();
    s.emit("sample", &mut j);
    let mut out = j.finish();
    out.push('\n');
    out
}

/// Many samples as one stream, with one header block for all of them.
pub fn lines_of(samples: &[Sample]) -> String {
    let mut l = Lines::default();
    for s in samples {
        l.add(s);
    }
    l.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sample::{MemStat, ProcSample};
    use std::sync::Arc;
    use std::time::{Duration, UNIX_EPOCH};

    fn fixture() -> Sample {
        let mut s = Sample::unknown();
        s.at = UNIX_EPOCH + Duration::from_secs(1_800_000_000);
        s.cpu_total = 12.5;
        s.cpu_per_core = vec![10.0, 15.0];
        s.uptime = Duration::from_secs(90_000);
        s.iowait = Some(1.5);
        // Absent on purpose: the pair every format here has to keep apart.
        s.steal = None;
        s.mem = MemStat {
            total: 16 << 30,
            used: 8 << 30,
            available: 8 << 30,
            free: Some(5 << 30),
            ..MemStat::default()
        };
        s.procs = vec![ProcSample {
            pid: 42,
            ppid: 1,
            name: Arc::from("post\tgres"),
            user: Arc::from("root"),
            cpu: 3.5,
            rss: 1 << 20,
            threads: Some(4),
            state: 'S',
            started: Some(7),
            ..ProcSample::default()
        }];
        s
    }

    #[test]
    fn every_field_has_a_unit() {
        // The units table is the one thing here that is not generated, so it is
        // the one thing that can drift. A field reachable from `Sample` with no
        // entry is a field a consumer is told nothing about, and this is what
        // stops that being noticed by a user instead of by a test.
        let mut missing = Vec::new();
        for (record, fields) in crate::sample::schemas() {
            for f in fields {
                if unit_of(record, &f.name).is_none() {
                    missing.push(format!("{record}.{}", f.name));
                }
            }
        }
        assert!(
            missing.is_empty(),
            "fields with no unit: {}",
            missing.join(", ")
        );
    }

    #[test]
    fn absence_is_not_a_zero_in_either_format() {
        // The rule the whole tool is built on, in the two formats meant to be
        // computed on. A consumer that cannot tell "nobody said" from "none
        // happened" will average one into the other.
        let s = fixture();
        let json = sample_json(&s);
        assert!(json.contains("\"steal\":null"), "{json}");
        assert!(json.contains("\"iowait\":1.5"), "{json}");
        // …and a real zero stays a zero.
        let mut zeroed = fixture();
        zeroed.steal = Some(0.0);
        assert!(sample_json(&zeroed).contains("\"steal\":0"));

        let lines = lines_of(std::slice::from_ref(&s));
        let cpu_row = lines
            .lines()
            .find(|l| l.starts_with("sample\t"))
            .expect("a sample row");
        let header = lines
            .lines()
            .find(|l| l.starts_with("#sample\t"))
            .expect("a header for it");
        let at = header
            .split(SEP)
            .position(|c| c == "steal")
            .expect("steal is a column");
        assert_eq!(cpu_row.split(SEP).nth(at), Some(ABSENT));
    }

    #[test]
    fn the_json_is_json() {
        let out = sample_json(&fixture());
        // Balanced, quoted, and with the separator inside a name escaped
        // rather than raw.
        // A document, not a dangling key-value pair.
        assert!(
            out.starts_with('{') && out.trim_end().ends_with('}'),
            "{out}"
        );
        assert_eq!(
            out.matches('{').count(),
            out.matches('}').count(),
            "unbalanced braces"
        );
        assert_eq!(out.matches('[').count(), out.matches(']').count());
        assert!(out.contains(r#""name":"post\tgres""#), "{out}");
        assert!(out.contains("\"pid\":42"), "{out}");
        // No comma before a closing brace, which is the one thing a
        // hand-rolled writer gets wrong.
        assert!(!out.contains(",}") && !out.contains(",]"), "{out}");
    }

    #[test]
    fn the_line_format_says_what_its_columns_are() {
        // A positional format that does not name its positions is atop's, and
        // its documentation is a man page somebody has to keep in step.
        let out = lines_of(&[fixture()]);
        let procs: Vec<&str> = out
            .lines()
            .filter(|l| l.starts_with("sample.procs\t"))
            .collect();
        assert_eq!(procs.len(), 1, "{out}");
        let header = out
            .lines()
            .find(|l| l.starts_with("#sample.procs\t"))
            .expect("the process row has no header");
        assert_eq!(
            header.split(SEP).count(),
            procs[0].split(SEP).count(),
            "the header and the row disagree about how many columns there are"
        );
        assert!(header.contains("pid"), "{header}");

        // The separator never appears inside a value.
        assert!(procs[0].contains("post gres"), "{}", procs[0]);
        assert_eq!(
            procs[0].split(SEP).count(),
            header.split(SEP).count(),
            "a tab in a process name became a column"
        );
    }

    #[test]
    fn a_table_is_one_label_with_one_row_a_member() {
        // The only thing this format is for is `grep '^sample.procs'` and read
        // the rows. Numbering the *label* gave `sample.procs.0`,
        // `sample.procs.1` … — six hundred labels and six hundred headers for
        // one table.
        let mut s = fixture();
        s.procs = (0..3)
            .map(|i| ProcSample {
                pid: 40 + i,
                name: Arc::from("p"),
                user: Arc::from("root"),
                io: (i != 1).then_some(crate::sample::IoRates { read: 1, write: 2 }),
                ..ProcSample::default()
            })
            .collect();
        let out = lines_of(std::slice::from_ref(&s));

        let rows: Vec<&str> = out
            .lines()
            .filter(|l| l.starts_with("sample.procs\t"))
            .collect();
        assert_eq!(rows.len(), 3, "{out}");
        assert_eq!(
            out.lines()
                .filter(|l| l.starts_with("#sample.procs\t"))
                .count(),
            1,
            "one table, one header"
        );
        // Each row carries its position, so the nested rows can be joined back.
        let pos: Vec<&str> = rows.iter().map(|r| r.split(SEP).nth(1).unwrap()).collect();
        assert_eq!(pos, ["0", "1", "2"]);

        // And a nested record inside a list element keeps that position. Two
        // of the three have io; without the index those rows arrive with
        // nothing on them saying which process they belong to.
        let io: Vec<&str> = out
            .lines()
            .filter(|l| l.starts_with("sample.procs.io\t"))
            .map(|r| r.split(SEP).nth(1).unwrap())
            .collect();
        assert_eq!(io, ["0", "2"], "{out}");
    }

    #[test]
    fn an_absent_record_does_not_widen_the_row_above_it() {
        // The failure this format cannot have. A record that *is* there
        // contributes no column to its parent — it opens a table of its own —
        // so writing one for the absent case made rows disagree with their own
        // header, and every field after the gap read as its neighbour. On this
        // machine that was 411 rows of eighteen columns and 180 of nineteen,
        // under one header naming eighteen.
        let mut s = fixture();
        s.procs = (0..4)
            .map(|i| ProcSample {
                pid: 40 + i,
                name: Arc::from("p"),
                user: Arc::from("root"),
                // Present on some, absent on others, in one table.
                io: (i % 2 == 0).then_some(crate::sample::IoRates { read: 1, write: 2 }),
                ..ProcSample::default()
            })
            .collect();
        let out = lines_of(std::slice::from_ref(&s));

        let widths: std::collections::HashSet<usize> = out
            .lines()
            .filter(|l| l.starts_with("sample.procs\t"))
            .map(|l| l.split(SEP).count())
            .collect();
        assert_eq!(
            widths.len(),
            1,
            "rows under one label had different widths: {widths:?}\n{out}"
        );

        // The absence is not lost: it is the missing row under the nested
        // label, which is where a reader looks for it.
        let io: Vec<&str> = out
            .lines()
            .filter(|l| l.starts_with("sample.procs.io\t"))
            .map(|l| l.split(SEP).nth(1).unwrap())
            .collect();
        assert_eq!(io, ["0", "2"], "{out}");

        // JSON keeps it as a null, because JSON has room to.
        assert_eq!(sample_json(&s).matches("\"io\":null").count(), 2);
    }

    #[test]
    fn a_day_is_one_stream_with_one_header_block() {
        // A fresh writer per sample re-emitted the whole header block for
        // every one of them — a hundred and forty-four copies at the default
        // logging interval, and a reader building a column map has to decide
        // which copy it meant.
        let day: Vec<Sample> = (0..5).map(|_| fixture()).collect();
        let out = lines_of(&day);
        assert_eq!(
            out.lines().filter(|l| l.starts_with("#sample\t")).count(),
            1,
            "the header block was written more than once"
        );
        assert_eq!(out.lines().filter(|l| l.starts_with("sample\t")).count(), 5);
        assert_eq!(
            out.lines()
                .filter(|l| l.starts_with("#sample.procs\t"))
                .count(),
            1
        );
    }

    #[test]
    fn a_percentage_is_not_widened_into_ten_digits_of_noise() {
        // Every percentage poptop reports is an `f32`. Cast to `f64`,
        // `51.7083` becomes 51.70830535888672 — ten digits of arithmetic that
        // were never measured, and a consumer reading them is reading the
        // width of a float.
        let mut s = fixture();
        s.cpu_total = 51.7083;
        s.iowait = Some(0.1);
        let json = sample_json(&s);
        // The exact token, with a delimiter after it: `contains` on a prefix
        // passes happily against `51.70830154418945`, which is the artefact
        // this exists to catch.
        assert!(json.contains("\"cpu_total\":51.7083,"), "{json}");
        assert!(json.contains("\"iowait\":0.1,"), "{json}");

        // And no figure anywhere in a sample carries more digits than an f32
        // has: seven is its precision, and nine is the most any shortest
        // round-trip representation needs.
        for tok in json.split(|c: char| !(c.is_ascii_digit() || c == '.')) {
            if let Some((_, frac)) = tok.split_once('.') {
                assert!(
                    frac.len() <= 9,
                    "a float was widened by a cast: {tok} in {json}"
                );
            }
        }
    }

    #[test]
    fn a_row_has_exactly_the_columns_its_header_names() {
        // A positional format whose rows and header disagree is worse than no
        // format: every consumer reads the wrong column and none of them
        // notices.
        // Two processes, one with `io` and one without, so the shape that
        // actually broke this is in the fixture rather than only in the test
        // that was written for it.
        let mut s = fixture();
        s.procs.push(ProcSample {
            pid: 43,
            name: Arc::from("q"),
            user: Arc::from("root"),
            io: Some(crate::sample::IoRates { read: 1, write: 2 }),
            ..ProcSample::default()
        });
        let out = lines_of(std::slice::from_ref(&s));
        let mut headers = std::collections::HashMap::new();
        for l in out.lines() {
            if let Some(h) = l.strip_prefix('#') {
                let (label, _) = h.split_once(SEP).unwrap();
                headers.insert(label.to_string(), h.split(SEP).count());
            }
        }
        assert!(!headers.is_empty());
        for l in out.lines().filter(|l| !l.starts_with('#')) {
            let (label, _) = l.split_once(SEP).unwrap();
            let want = headers
                .get(label)
                .unwrap_or_else(|| panic!("`{label}` has rows and no header"));
            assert_eq!(
                l.split(SEP).count(),
                *want,
                "`{label}` row and header disagree:\n{l}"
            );
        }
    }

    #[test]
    fn the_schema_names_every_record_and_its_units() {
        let s = schema_json();
        for (record, _) in crate::sample::schemas() {
            assert!(s.contains(&format!("{record:?}")), "{record} is not in it");
        }
        assert!(s.contains("\"unit\": \"percent\""), "{s}");
        assert!(s.contains("\"unit\": \"bytes\""), "{s}");
        assert!(!s.contains("\"unit\": \"unknown\""), "a field has no unit");
        assert_eq!(s.matches('{').count(), s.matches('}').count());
    }
}
