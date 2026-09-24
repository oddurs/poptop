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
    /// Degrees Celsius.
    Celsius,
    /// Revolutions a minute.
    Rpm,
    /// Watts.
    Watts,
    /// Minutes.
    Minutes,
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
            Unit::Celsius => "celsius",
            Unit::Rpm => "rpm",
            Unit::Watts => "watts",
            Unit::Minutes => "minutes",
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
            // Sentences. What poptop had to assume at this moment, which is
            // text by nature: a unit on it would invite arithmetic on prose.
            "notes" => None,
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
        | ("Sample", "tasks" | "exited" | "cgroups" | "nodes" | "nfs" | "temps" | "fans")
        | ("Sample", "power" | "gpus")
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
        ("Temp", "celsius" | "crit") => Some(Celsius),
        ("Temp", "group" | "sensor") | ("Fan", "label") => Some(None),
        ("Fan", "rpm") => Some(Rpm),
        ("Power", "charge") | ("Gpu", "util") => Some(Percent),
        ("Power", "state") | ("Gpu", "name") => Some(None),
        ("Power", "watts") => Some(Watts),
        ("Power", "minutes") => Some(Minutes),
        ("Gpu", "mem_used" | "mem_total") => Some(Bytes),
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
    /// `want` narrows it to the fields asked for; the header block then names
    /// exactly the columns the rows carry, as it always did.
    pub fn add(&mut self, s: &Sample, want: Option<&Fields>) {
        match want {
            Some(want) => {
                let mut only = Only::new(self, want);
                s.emit("sample", &mut only);
            }
            None => s.emit("sample", self),
        }
        while let Some(f) = self.stack.pop() {
            self.emit_frame(f);
        }
    }

    /// What has been written since the last call, and nothing twice.
    ///
    /// For `--export --follow`, which writes each sample as it is taken rather
    /// than the whole stream at the end. Only the output is taken: the labels
    /// already headed stay, so a feed running for a week writes the header
    /// block once, at the top, exactly as a day does.
    pub fn take(&mut self) -> String {
        std::mem::take(&mut self.out)
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
        // had in a process name, and neither can a line ending: a CR is one
        // to every reader that splits on universal newlines.
        self.push(name, v.replace([SEP, '\n', '\r'], " "));
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
pub fn sample_json(s: &Sample, want: Option<&Fields>) -> String {
    let mut j = Json::default();
    match want {
        Some(want) => s.emit("sample", &mut Only::new(&mut j, want)),
        None => s.emit("sample", &mut j),
    }
    let mut out = j.finish();
    out.push('\n');
    out
}

/// Many samples as one stream, with one header block for all of them.
pub fn lines_of(samples: &[Sample], want: Option<&Fields>) -> String {
    let mut l = Lines::default();
    for s in samples {
        l.add(s, want);
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
            threads: Some(4).into(),
            state: 'S',
            started: Some(7).into(),
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
        let json = sample_json(&s, None);
        assert!(json.contains("\"steal\":null"), "{json}");
        assert!(json.contains("\"iowait\":1.5"), "{json}");
        // …and a real zero stays a zero.
        let mut zeroed = fixture();
        zeroed.steal = Some(0.0);
        assert!(sample_json(&zeroed, None).contains("\"steal\":0"));

        let lines = lines_of(std::slice::from_ref(&s), None);
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
        let out = sample_json(&fixture(), None);
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
        let out = lines_of(&[fixture()], None);
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
                io: (i != 1)
                    .then_some(crate::sample::IoRates { read: 1, write: 2 })
                    .into(),
                ..ProcSample::default()
            })
            .collect();
        let out = lines_of(std::slice::from_ref(&s), None);

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
                io: (i % 2 == 0)
                    .then_some(crate::sample::IoRates { read: 1, write: 2 })
                    .into(),
                ..ProcSample::default()
            })
            .collect();
        let out = lines_of(std::slice::from_ref(&s), None);

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
        assert_eq!(sample_json(&s, None).matches("\"io\":null").count(), 2);
    }

    #[test]
    fn a_day_is_one_stream_with_one_header_block() {
        // A fresh writer per sample re-emitted the whole header block for
        // every one of them — a hundred and forty-four copies at the default
        // logging interval, and a reader building a column map has to decide
        // which copy it meant.
        let day: Vec<Sample> = (0..5).map(|_| fixture()).collect();
        let out = lines_of(&day, None);
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
        let json = sample_json(&s, None);
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
            io: Some(crate::sample::IoRates { read: 1, write: 2 }).into(),
            ..ProcSample::default()
        });
        let out = lines_of(std::slice::from_ref(&s), None);
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

// ── a narrower feed ───────────────────────────────────────────────────────
// 0149. `--export` writes every field of every record, which is what "every
// metric by name" means and which is hundreds of kilobytes a sample at four
// hundred processes. A dashboard reading four numbers a second pays for the
// whole process table each time, and piping it through `jq` only means poptop
// built it first.

/// The fields a feed was asked for, as dotted paths into the schema.
///
/// `cpu_total`, `mem.used`, `procs.name`. A list is transparent: `procs.name`
/// is that field of every process, because the alternative — naming an index —
/// is a question nobody asks of a process table.
#[derive(Debug, Clone, PartialEq)]
pub struct Fields(Vec<String>);

impl Fields {
    /// Parse a comma-separated list, checked against the schema.
    ///
    /// Checked, and refused with the closest thing the schema does have: a
    /// filter that silently dropped a name the caller spelled wrong would
    /// produce a feed missing exactly the figure they were watching for, and
    /// nothing in it would say so.
    pub fn parse(spec: &str) -> Result<Fields, String> {
        let known = paths();
        let mut want = Vec::new();
        for name in spec.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            // `sample.cpu_total` is how the line format labels it, and it is
            // the spelling somebody copies out of the output; `cpu_total` is
            // what the flag documents. Both mean the field.
            let name = name.strip_prefix("sample.").unwrap_or(name);
            if !known.iter().any(|k| k == name) {
                return Err(match closest(name, &known) {
                    Some(near) => format!("no field `{name}` — did you mean `{near}`?"),
                    None => format!("no field `{name}`. `poptop --schema` lists them"),
                });
            }
            if !want.iter().any(|w| w == name) {
                want.push(name.to_string());
            }
        }
        if want.is_empty() {
            return Err("--fields needs at least one field name".into());
        }
        // Always the time. A record that does not say when it was taken is not
        // a sample of anything, and a consumer that asked for `cpu_total`
        // meant the series and not the number.
        if !want.iter().any(|w| w == "at") {
            want.insert(0, "at".to_string());
        }
        Ok(Fields(want))
    }

    /// Whether a scalar at this path was asked for: the path itself, or
    /// anything under a record that was.
    fn wants(&self, path: &str) -> bool {
        self.0.iter().any(|w| {
            w == path
                || path
                    .strip_prefix(w.as_str())
                    .is_some_and(|r| r.starts_with('.'))
        })
    }

    /// Whether anything under this record or list was asked for.
    fn into(&self, path: &str) -> bool {
        self.wants(path)
            || self
                .0
                .iter()
                .any(|w| w.strip_prefix(path).is_some_and(|r| r.starts_with('.')))
    }
}

/// Every dotted path the schema has, records included.
fn paths() -> Vec<String> {
    let schemas = crate::sample::schemas();
    let mut out = Vec::new();
    walk("", "Sample", &schemas, &mut out, 0);
    out
}

fn walk(
    prefix: &str,
    record: &str,
    schemas: &[(&'static str, Vec<crate::persist::Field>)],
    out: &mut Vec<String>,
    depth: usize,
) {
    // The schema is a tree of records that can, in principle, refer to one
    // another: a bound rather than a visited set, because the depth of the
    // real one is four and a cycle would be a bug in the declaration.
    if depth > 8 {
        return;
    }
    let Some((_, fields)) = schemas.iter().find(|(n, _)| *n == record) else {
        return;
    };
    for f in fields {
        let path = format!("{prefix}{}", f.name);
        out.push(path.clone());
        if let Some(rec) = record_of(&f.ty) {
            walk(&format!("{path}."), rec, schemas, out, depth + 1);
        }
    }
}

/// The record a type is, or holds: `Option<Vec<DiskStat>>` is `DiskStat`.
fn record_of(t: &Ty) -> Option<&str> {
    match t {
        Ty::Rec(name) => Some(name),
        Ty::Opt(inner) | Ty::List(inner) | Ty::Arr(inner, _) => record_of(inner),
        _ => None,
    }
}

/// The known path nearest a misspelling, if anything is near enough.
///
/// Edit distance, with a bound of a third of the name: "did you mean" is only
/// useful while it is usually right, and a suggestion drawn from across the
/// schema is worse than none.
fn closest<'a>(name: &str, known: &'a [String]) -> Option<&'a str> {
    let bound = (name.len() / 3).max(1);
    known
        .iter()
        .map(|k| (distance(name, k), k))
        .filter(|(d, _)| *d <= bound)
        .min_by_key(|(d, k)| (*d, k.len()))
        .map(|(_, k)| k.as_str())
}

/// Levenshtein distance, two rows at a time.
fn distance(a: &str, b: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut row = vec![0; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            row[j + 1] = (prev[j] + usize::from(ca != cb))
                .min(prev[j + 1] + 1)
                .min(row[j] + 1);
        }
        std::mem::swap(&mut prev, &mut row);
    }
    prev[b.len()]
}

/// A [`Visit`] that passes on only the fields that were asked for.
///
/// Between the sample and the writer, rather than inside each writer: both
/// formats then narrow the same way, and neither had to learn what a field
/// name is.
pub struct Only<'a> {
    to: &'a mut dyn Visit,
    want: &'a Fields,
    /// One per open container: the logical path it contributes to, and whether
    /// its children are list elements.
    ///
    /// A list is transparent: an element's name is its index, and a path
    /// through it would be `procs.17.name` — a different path for every
    /// process, which is not a thing anyone can ask for.
    stack: Vec<Level>,
    /// How deep inside a subtree nobody asked for. Everything is dropped while
    /// this is above zero, including the `close` that ends it.
    skipping: usize,
}

struct Level {
    path: String,
    list: bool,
}

impl<'a> Only<'a> {
    pub fn new(to: &'a mut dyn Visit, want: &'a Fields) -> Only<'a> {
        Only {
            to,
            want,
            stack: Vec::new(),
            skipping: 0,
        }
    }

    /// The logical path of a field named `name` at this depth.
    fn path(&self, name: &str) -> String {
        match self.stack.last() {
            // An element of a list: the field belongs to the list's own path.
            Some(f) if f.list => f.path.clone(),
            Some(f) if f.path.is_empty() => name.to_string(),
            Some(f) => format!("{}.{name}", f.path),
            None => name.to_string(),
        }
    }

    fn scalar(&mut self, name: &str, write: impl FnOnce(&mut dyn Visit, &str)) {
        if self.skipping > 0 {
            return;
        }
        let path = self.path(name);
        if self.want.wants(&path) {
            write(self.to, name);
        }
    }
}

impl Visit for Only<'_> {
    fn num(&mut self, name: &str, v: f64) {
        self.scalar(name, |to, n| to.num(n, v));
    }
    fn int(&mut self, name: &str, v: i128) {
        self.scalar(name, |to, n| to.int(n, v));
    }
    fn text(&mut self, name: &str, v: &str) {
        self.scalar(name, |to, n| to.text(n, v));
    }
    fn flag(&mut self, name: &str, v: bool) {
        self.scalar(name, |to, n| to.flag(n, v));
    }
    fn absent(&mut self, name: &str) {
        // Absence passes the filter exactly as a value does. A narrow feed
        // that turned a figure nobody reported into a missing column would
        // undo the one rule this format is built on.
        self.scalar(name, |to, n| to.absent(n));
    }
    fn absent_nested(&mut self, name: &str) {
        if self.skipping > 0 {
            return;
        }
        let path = self.path(name);
        if self.want.into(&path) {
            self.to.absent_nested(name);
        }
    }
    fn open(&mut self, name: &str, list: bool) {
        if self.skipping > 0 {
            self.skipping += 1;
            return;
        }
        // The root record contributes nothing: the flag's names are
        // `cpu_total`, not `sample.cpu_total`.
        let path = match self.stack.last() {
            None => String::new(),
            Some(f) if f.list => f.path.clone(),
            Some(f) if f.path.is_empty() => name.to_string(),
            Some(f) => format!("{}.{name}", f.path),
        };
        if !path.is_empty() && !self.want.into(&path) {
            self.skipping = 1;
            return;
        }
        self.stack.push(Level { path, list });
        self.to.open(name, list);
    }
    fn close(&mut self) {
        if self.skipping > 0 {
            self.skipping -= 1;
            return;
        }
        self.stack.pop();
        self.to.close();
    }
}

#[cfg(test)]
mod narrow_tests {
    use super::*;
    use crate::sample::{MemStat, ProcSample};
    use std::sync::Arc;
    use std::time::{Duration, UNIX_EPOCH};

    fn sample() -> Sample {
        let mut s = Sample::unknown();
        s.at = UNIX_EPOCH + Duration::from_secs(1_800_000_000);
        s.cpu_total = 12.5;
        // Absent on purpose: the pair every format here keeps apart.
        s.iowait = None;
        s.mem = MemStat {
            total: 16 << 30,
            used: 8 << 30,
            available: 8 << 30,
            ..MemStat::default()
        };
        s.procs = ["rustc", "node"]
            .into_iter()
            .enumerate()
            .map(|(i, name)| ProcSample {
                pid: 100 + i as i32,
                name: Arc::from(name),
                rss: 1 << 20,
                cpu: 5.0,
                ..ProcSample::default()
            })
            .collect();
        s
    }

    #[test]
    fn a_named_subset_is_emitted_in_schema_order() {
        // Schema order, not the order they were asked for: the format's order
        // is the schema's everywhere else, and a consumer building a column
        // map from the header would otherwise get a different map per caller.
        let want = Fields::parse("procs.name,cpu_total,mem.used").unwrap();
        let json = sample_json(&sample(), Some(&want));
        assert_eq!(
            json.trim_end(),
            "{\"at\":1800000000,\"cpu_total\":12.5,\"mem\":{\"used\":8589934592},\
             \"procs\":[{\"name\":\"rustc\"},{\"name\":\"node\"}]}"
        );

        let mut lines = Lines::default();
        lines.add(&sample(), Some(&want));
        let text = lines.finish();
        assert!(text.contains("#sample\tat\tcpu_total\n"), "{text}");
        assert!(text.contains("#sample.mem\tused\n"), "{text}");
        // A list stays a table of its own, one row an element, with the index
        // that joins it back to its neighbours.
        assert_eq!(
            text.lines()
                .filter(|l| l.starts_with("sample.procs\t"))
                .count(),
            2,
            "{text}"
        );
        assert!(!text.contains("rss"), "a field nobody asked for: {text}");
    }

    #[test]
    fn the_time_is_always_there() {
        // A record that does not say when it was taken is not a sample of
        // anything, and somebody asking for `cpu_total` meant the series.
        for spec in ["cpu_total", "procs.name", "at,cpu_total"] {
            let json = sample_json(&sample(), Some(&Fields::parse(spec).unwrap()));
            assert!(json.starts_with("{\"at\":"), "`{spec}` gave {json}");
            assert_eq!(json.matches("\"at\":").count(), 1, "`{spec}` gave {json}");
        }
    }

    #[test]
    fn absence_survives_the_filter() {
        // The one rule this format is built on. A filter that turned a figure
        // nobody reported into a missing column would undo it, and the caller
        // would read "poptop did not ask" as "the kernel does not publish".
        let want = Fields::parse("iowait,cpu_total").unwrap();
        let json = sample_json(&sample(), Some(&want));
        assert!(json.contains("\"iowait\":null"), "{json}");

        let mut lines = Lines::default();
        lines.add(&sample(), Some(&want));
        let text = lines.finish();
        let row = text
            .lines()
            .find(|l| l.starts_with("sample\t"))
            .expect("no sample row");
        assert_eq!(
            row.split(SEP).count(),
            text.lines()
                .find(|l| l.starts_with("#sample\t"))
                .unwrap()
                .split(SEP)
                .count(),
            "the row and its header disagree: {text}"
        );
        assert!(row.contains(ABSENT), "{row}");
    }

    #[test]
    fn a_whole_record_can_be_asked_for() {
        let json = sample_json(&sample(), Some(&Fields::parse("mem").unwrap()));
        assert!(json.contains("\"total\":17179869184"), "{json}");
        assert!(json.contains("\"used\":8589934592"), "{json}");
        assert!(!json.contains("cpu_total"), "{json}");
    }

    #[test]
    fn an_unknown_field_is_refused_with_the_closest_the_schema_has() {
        // Silently dropping it would produce a feed missing exactly the figure
        // the caller was watching for, with nothing in it saying so.
        for (spec, want) in [
            ("cpu_totl", "did you mean `cpu_total`"),
            ("mem.usedd", "did you mean `mem.used`"),
            ("procs.nme", "did you mean `procs.name`"),
        ] {
            let e = Fields::parse(spec).expect_err("accepted a name the schema does not have");
            assert!(e.contains(want), "`{spec}` said: {e}");
        }
        // Nothing near enough: the schema itself, rather than a wild guess.
        let e = Fields::parse("temperature").expect_err("accepted a field poptop has no idea of");
        assert!(e.contains("--schema"), "{e}");
        assert!(Fields::parse(" , ").is_err(), "an empty list was accepted");
        // The spelling the line format prints is the spelling that works.
        assert_eq!(
            Fields::parse("sample.cpu_total").unwrap(),
            Fields::parse("cpu_total").unwrap()
        );
    }

    #[test]
    #[ignore = "a measurement, not an assertion"]
    fn measure_a_narrow_feed() {
        // What a dashboard reading four numbers saves by not being handed a
        // process table. `cargo test --release -- --ignored --nocapture
        // measure_a_narrow_feed`.
        let s = crate::store::tests_support::big_sample(5.0, 400);
        let want = Fields::parse("cpu_total,mem.used,load").unwrap();
        let (mut whole, mut narrow) = (std::time::Duration::MAX, std::time::Duration::MAX);
        let (mut wb, mut nb) = (0, 0);
        for _ in 0..7 {
            let t = std::time::Instant::now();
            wb = sample_json(&s, None).len();
            whole = whole.min(t.elapsed());
            let t = std::time::Instant::now();
            nb = sample_json(&s, Some(&want)).len();
            narrow = narrow.min(t.elapsed());
        }
        eprintln!(
            "400 processes: whole {wb} bytes in {whole:?}, narrowed {nb} bytes in {narrow:?}"
        );
    }
}
