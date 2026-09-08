//! A small query language for the process filter.
//!
//! "Why is this slow" is usually a question about a *predicate*, not a name.
//! The useful questions are the ones a substring match cannot ask, and each of
//! them is a header figure the reader is already looking at:
//!
//! ```text
//! state = D              everything stuck in uninterruptible sleep, which the
//!                        BLOCKED figure counts and cannot itemise
//! write > 1mb            who is causing the disk saturation just reported
//! threads > 100          the thing leaking threads
//! cpu > 5 and user = root
//! ```
//!
//! It is worth more here than in the tools it is borrowed from, because of the
//! buffer: a predicate evaluated while scrubbing answers "what was in D-state
//! at the moment of the spike", which is a question no live-only tool can be
//! asked.
//!
//! # What this deliberately is not
//!
//! bottom's version has `or`, `!`, parentheses and regex. This has `and` and
//! nothing else. The full grammar is a parser, a precedence table, a test suite
//! and a syntax to document, and `field op value` joined by `and` answers every
//! question in the list above. The line is drawn here rather than at "whatever
//! was easy", and it is drawn in writing so the next person can move it on
//! purpose.
//!
//! A bare word is still a substring match, so every existing use of `/` keeps
//! working and nobody has to learn this to use the filter at all.

use crate::sample::ProcSample;

/// A column a query can ask about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Cpu,
    Rss,
    Threads,
    State,
    Pid,
    Read,
    Write,
    User,
    Name,
}

/// Every field, with the spellings accepted for it.
///
/// The aliases are the words people reach for — `mem` for memory, `thr` for the
/// column header — rather than a second name for its own sake.
const FIELDS: &[(&str, Field)] = &[
    ("cpu", Field::Cpu),
    ("mem", Field::Rss),
    ("rss", Field::Rss),
    ("threads", Field::Threads),
    ("thr", Field::Threads),
    ("state", Field::State),
    ("pid", Field::Pid),
    ("read", Field::Read),
    ("write", Field::Write),
    ("user", Field::User),
    ("name", Field::Name),
    ("command", Field::Name),
];

impl Field {
    fn parse(word: &str) -> Option<Field> {
        FIELDS.iter().find(|(n, _)| *n == word).map(|(_, f)| *f)
    }

    /// Whether its values are byte quantities, and so take units.
    fn is_bytes(self) -> bool {
        matches!(self, Field::Rss | Field::Read | Field::Write)
    }

    /// Whether it is compared as text rather than as a number.
    fn is_text(self) -> bool {
        matches!(self, Field::State | Field::User | Field::Name)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Gt,
    Ge,
    Lt,
    Le,
    Eq,
    Ne,
}

/// Longest first, so `>=` is not read as `>` followed by a stray `=`.
const OPS: &[(&str, Op)] = &[
    (">=", Op::Ge),
    ("<=", Op::Le),
    ("!=", Op::Ne),
    (">", Op::Gt),
    ("<", Op::Lt),
    ("=", Op::Eq),
];

#[derive(Clone, Debug, PartialEq)]
enum Term {
    /// A bare word: matches name, command line, user or pid, as it always did.
    Substring(String),
    Number {
        field: Field,
        op: Op,
        value: f64,
    },
    Text {
        field: Field,
        op: Op,
        value: String,
    },
}

/// A parsed filter.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Query {
    terms: Vec<Term>,
}

impl Query {
    /// Whether a process satisfies every term.
    ///
    /// An unreadable figure satisfies *nothing* — not `>`, and not `<` either.
    /// A process whose IO could not be read is not one doing no IO, so it must
    /// not answer a question about its IO in either direction. That is the same
    /// refusal the `—` in the column is making, one layer up.
    pub fn matches(&self, p: &ProcSample) -> bool {
        self.terms.iter().all(|t| match t {
            Term::Substring(needle) => {
                p.name.to_lowercase().contains(needle)
                    || p.cmd
                        .as_ref()
                        .is_some_and(|c| c.to_lowercase().contains(needle))
                    || p.user.to_lowercase().contains(needle)
                    || p.pid.to_string().contains(needle)
            }
            Term::Number { field, op, value } => match number_of(p, *field) {
                Some(have) => compare(have, *op, *value),
                None => false,
            },
            Term::Text { field, op, value } => {
                let have = text_of(p, *field).to_lowercase();
                match op {
                    // Text is matched by containment, not equality: `name =
                    // node` should find `node /srv/api/server.js`, which is what
                    // anyone typing it means.
                    Op::Eq => have.contains(value),
                    Op::Ne => !have.contains(value),
                    _ => have.as_str() > value.as_str(),
                }
            }
        })
    }

    /// Whether this query would keep every process.
    pub fn is_empty(&self) -> bool {
        self.terms.is_empty()
    }
}

fn number_of(p: &ProcSample, f: Field) -> Option<f64> {
    Some(match f {
        Field::Cpu => p.cpu as f64,
        Field::Rss => p.rss as f64,
        Field::Threads => p.threads? as f64,
        Field::Pid => p.pid as f64,
        Field::Read => p.io?.read as f64,
        Field::Write => p.io?.write as f64,
        Field::State | Field::User | Field::Name => return None,
    })
}

fn text_of(p: &ProcSample, f: Field) -> &str {
    match f {
        Field::User => &p.user,
        Field::Name => p.command(),
        // A `char` needs somewhere to live; the states are all ASCII.
        Field::State => match p.state {
            'R' => "r",
            'S' => "s",
            'D' => "d",
            'T' => "t",
            'Z' => "z",
            'I' => "i",
            _ => "?",
        },
        _ => "",
    }
}

fn compare(have: f64, op: Op, want: f64) -> bool {
    match op {
        Op::Gt => have > want,
        Op::Ge => have >= want,
        Op::Lt => have < want,
        Op::Le => have <= want,
        // Floats, so equality is a band rather than a point: `cpu = 5` should
        // find a process at 5.04, which is what the column renders as `5.0`.
        Op::Eq => (have - want).abs() < 0.05,
        Op::Ne => (have - want).abs() >= 0.05,
    }
}

/// Parse a filter, or say what is wrong with it.
///
/// The error names the fields, because a query language nobody knows the
/// keywords for is worse than a substring match. It is the only discovery
/// mechanism a one-line filter box can carry.
pub fn parse(input: &str) -> Result<Query, String> {
    let mut terms = Vec::new();
    for raw in split_and(input) {
        let term = raw.trim();
        if term.is_empty() {
            continue;
        }
        terms.push(parse_term(term)?);
    }
    Ok(Query { terms })
}

/// Split on the word `and`, which is a separator only when it stands alone.
///
/// Otherwise `/android` would parse as two empty terms, and the commonest thing
/// anyone types — a bare word — would be the thing most likely to break.
fn split_and(input: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = input;
    while let Some(i) = find_and(rest) {
        out.push(&rest[..i]);
        rest = &rest[i + 3..];
    }
    out.push(rest);
    out
}

fn find_and(s: &str) -> Option<usize> {
    let lower = s.to_lowercase();
    let mut from = 0;
    while let Some(i) = lower[from..].find("and") {
        let at = from + i;
        let before = s[..at].chars().next_back();
        let after = s[at + 3..].chars().next();
        if before.is_none_or(char::is_whitespace) && after.is_none_or(char::is_whitespace) {
            return Some(at);
        }
        from = at + 3;
    }
    None
}

fn parse_term(term: &str) -> Result<Term, String> {
    let Some((at, op)) = OPS
        .iter()
        .filter_map(|(sym, op)| term.find(sym).map(|i| (i, *op, sym.len())))
        .min_by_key(|(i, _, _)| *i)
        .map(|(i, op, len)| ((i, len), op))
    else {
        // No operator: the bare word it always was.
        return Ok(Term::Substring(term.to_lowercase()));
    };
    let (i, len) = at;
    let field_word = term[..i].trim().to_lowercase();
    let value = term[i + len..].trim();

    let Some(field) = Field::parse(&field_word) else {
        return Err(format!(
            "no field called `{field_word}` — try {}",
            field_names()
        ));
    };
    if value.is_empty() {
        return Err(format!("`{field_word}` is compared to nothing"));
    }

    if field.is_text() {
        if !matches!(op, Op::Eq | Op::Ne) {
            return Err(format!("`{field_word}` can only be = or !=, not < or >"));
        }
        return Ok(Term::Text {
            field,
            op,
            value: value.to_lowercase(),
        });
    }

    let n = parse_number(value, field).ok_or_else(|| {
        if field.is_bytes() {
            format!("`{value}` is not a size — try 1mb, 500k, 2g")
        } else {
            format!("`{value}` is not a number")
        }
    })?;
    Ok(Term::Number {
        field,
        op,
        value: n,
    })
}

fn field_names() -> String {
    let mut seen: Vec<&str> = Vec::new();
    for (name, _) in FIELDS {
        if !seen.contains(name) {
            seen.push(name);
        }
    }
    seen.join(", ")
}

/// A number, with a unit on the fields that are byte quantities.
///
/// `1mb` is 1048576, not 1000000: this is a process monitor, and the column it
/// is filtering renders `1.0M` for the same quantity. Matching the display is
/// worth more than matching the SI.
fn parse_number(s: &str, field: Field) -> Option<f64> {
    let s = s.trim();
    let digits = s.trim_end_matches(|c: char| c.is_ascii_alphabetic()).trim();
    let unit = s[digits.len()..].trim().to_lowercase();
    let n: f64 = digits.parse().ok()?;
    if unit.is_empty() {
        return Some(n);
    }
    if !field.is_bytes() {
        return None;
    }
    let scale = match unit.trim_end_matches('b') {
        "" => 1.0,
        "k" => 1024.0,
        "m" => 1024.0 * 1024.0,
        "g" => 1024.0 * 1024.0 * 1024.0,
        "t" => 1024.0 * 1024.0 * 1024.0 * 1024.0,
        _ => return None,
    };
    Some(n * scale)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sample::IoRates;
    use std::sync::Arc;

    fn proc(name: &str) -> ProcSample {
        ProcSample {
            pid: 4821,
            ppid: 1,
            name: Arc::from(name),
            user: Arc::from("deploy"),
            cpu: 12.5,
            rss: 512 << 20,
            threads: Some(41),
            state: 'S',
            started: Some(1),
            cmd: Some(Arc::from("node /srv/api/server.js --port 3000")),
            io: Some(IoRates {
                read: 2 << 20,
                write: 512 << 10,
            }),
        }
    }

    fn keeps(q: &str, p: &ProcSample) -> bool {
        parse(q).expect("did not parse").matches(p)
    }

    #[test]
    fn a_bare_word_is_still_a_substring_match() {
        // Every existing use of `/` has to keep working, and nobody should have
        // to learn a query language to use the filter at all.
        let p = proc("node");
        assert!(keeps("node", &p));
        assert!(keeps("server.js", &p), "the command line is not searched");
        assert!(keeps("deploy", &p), "the user is not searched");
        assert!(keeps("4821", &p), "the pid is not searched");
        assert!(!keeps("postgres", &p));
        assert!(keeps("", &p), "an empty filter hid something");
    }

    #[test]
    fn the_questions_the_item_asked_for() {
        let p = proc("node");
        // Everything stuck in uninterruptible sleep — the BLOCKED figure the
        // header counts and cannot itemise.
        assert!(!keeps("state = D", &p));
        assert!(keeps("state = S", &p));
        // Who is causing the disk saturation just reported.
        assert!(!keeps("write > 1mb", &p), "512K is not over 1MB");
        assert!(keeps("read > 1mb", &p));
        // The thing leaking threads.
        assert!(!keeps("threads > 100", &p));
        assert!(keeps("threads > 40", &p));
        // And joined.
        assert!(keeps("cpu > 5 and user = deploy", &p));
        assert!(!keeps("cpu > 5 and user = root", &p));
    }

    #[test]
    fn sizes_are_read_the_way_the_column_writes_them() {
        // `1mb` is 1048576, not 1000000: the column this filters renders `1.0M`
        // for that quantity, and matching the display is worth more than
        // matching the SI.
        let p = proc("node");
        assert!(keeps("mem = 512mb", &p), "512 << 20 is not 512mb");
        assert!(keeps("mem > 500m", &p));
        // A `k` case as well, or the scale table is only half tested: `mem >
        // 524287k` is true at binary scale and false at decimal.
        assert!(keeps("mem > 524287k", &p), "512MiB is not over 524287 KiB");
        assert!(!keeps("mem > 524289k", &p));
        assert!(keeps("mem < 1g", &p));
        assert!(!keeps("mem > 1g", &p));
        // Bare numbers are bytes.
        assert!(keeps("mem > 1000", &p));
    }

    #[test]
    fn an_unreadable_figure_answers_nothing_in_either_direction() {
        // A process whose IO could not be read is not one doing no IO. It must
        // not match `write > 1mb`, and it must not match `write < 1mb` either —
        // the same refusal the `—` in the column is making, one layer up.
        let mut p = proc("node");
        p.io = None;
        assert!(!keeps("write > 1mb", &p));
        assert!(!keeps("write < 1mb", &p));
        assert!(!keeps("read >= 0", &p));
        // …and a threads figure the platform would not give.
        p.threads = None;
        assert!(!keeps("threads > 0", &p));
        assert!(!keeps("threads < 999", &p));
    }

    #[test]
    fn a_malformed_query_says_what_is_wrong_and_names_the_fields() {
        let why = parse("cpuu > 5").unwrap_err();
        assert!(why.contains("cpuu"), "{why}");
        for field in ["cpu", "mem", "state", "threads", "read", "write"] {
            assert!(
                why.contains(field),
                "the error does not name {field}: {why}"
            );
        }
        assert!(parse("cpu >").unwrap_err().contains("nothing"));
        assert!(parse("cpu > lots").unwrap_err().contains("not a number"));
        assert!(parse("mem > lots").unwrap_err().contains("not a size"));
        // Text fields cannot be ordered.
        assert!(parse("state > D").unwrap_err().contains("= or !="));
    }

    #[test]
    fn and_is_a_separator_only_when_it_stands_alone() {
        // Otherwise `/android` parses as two empty terms, and the commonest
        // thing anyone types is the thing most likely to break.
        let p = ProcSample {
            name: Arc::from("android-studio"),
            cmd: None,
            ..proc("android-studio")
        };
        assert!(keeps("android", &p));
        assert!(keeps("andr", &p));
        assert_eq!(
            parse("android").unwrap().terms,
            vec![Term::Substring("android".into())],
            "`and` was split out of the middle of a word"
        );
        assert_eq!(parse("cpu > 1 and mem > 1").unwrap().terms.len(), 2);
    }

    #[test]
    fn spaces_around_the_operator_are_optional() {
        let p = proc("node");
        assert!(keeps("cpu>5", &p));
        assert!(keeps("state=S", &p));
        assert!(keeps("mem>=512mb", &p));
    }

    #[test]
    fn text_is_matched_by_containment_so_a_name_finds_its_command_line() {
        // `name = node` should find `node /srv/api/server.js`, which is what
        // anyone typing it means.
        let p = proc("node");
        assert!(keeps("name = node", &p));
        assert!(keeps("name = server.js", &p));
        assert!(!keeps("name = postgres", &p));
        assert!(keeps("name != postgres", &p));
    }
}
