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
    /// The state of any *task* of this process — see `matches_in`.
    Thread,
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
    // `task`, not `thread`: `threads` is already the *count* of them, and two
    // fields one letter apart with different meanings is a filter box that
    // punishes typing. The kernel calls them tasks, and so does the header the
    // predicate exists to itemise.
    ("task", Field::Thread),
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
        matches!(
            self,
            Field::State | Field::Thread | Field::User | Field::Name
        )
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
        self.matches_in(p, None)
    }

    /// The same, with the sample's threads available.
    ///
    /// `task = D` asks whether *any* thread of this process is in that state,
    /// which is what makes the header's task-level figures findable: `BLOCKED`
    /// counts tasks, so a box with two blocked threads inside one
    /// healthy-looking process reports a number the process table cannot
    /// itemise. This is how you get from the number to the row.
    ///
    /// With no threads collected the predicate matches nothing rather than
    /// everything. It is a question about data that was not gathered, and the
    /// panel title says so — inventing a `false` for every process would be
    /// indistinguishable from an honest empty result, but inventing a `true`
    /// would claim every process has such a thread.
    pub fn matches_in(
        &self,
        p: &ProcSample,
        tasks: Option<&[crate::sample::ThreadSample]>,
    ) -> bool {
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
                Some(have) => compare(have, *op, *value, *field),
                None => false,
            },
            Term::Text { field, op, value } => {
                // A question about data that was not gathered, refused in both
                // directions. `!hit` below would otherwise turn "no threads
                // were collected" into `task != R` matching every process, as
                // if all their threads had been inspected and none was
                // running. The numeric path refuses both directions for an
                // unreadable figure for exactly this reason; the em dash in the
                // column is making the same refusal.
                if *field == Field::Thread && tasks.is_none() {
                    return false;
                }
                let hit = match field {
                    // Identity, so `=` means equality. `user = root` matching
                    // `rootless` and `root-ci` is not what anyone typing it
                    // means, and `user != root` excluding them is worse.
                    Field::User => p.user.to_lowercase() == *value,
                    // One letter, compared as one letter. A lookup table over
                    // the states this tool happens to have seen collapses `t`,
                    // `W`, `X`, `K` and `P` into a single `?`, so a traced
                    // process could never be found and unrelated states
                    // compared equal to each other.
                    Field::State => {
                        value.chars().count() == 1
                            && p.state
                                .eq_ignore_ascii_case(&value.chars().next().unwrap_or(' '))
                    }
                    // Any thread, not every thread. The question this answers
                    // is "which process is the blocked task inside", and a
                    // process with thirty-nine idle threads and one in `D` is
                    // the answer to it.
                    Field::Thread => {
                        value.chars().count() == 1 && {
                            let want = value.chars().next().unwrap_or(' ');
                            let mut mine = tasks
                                .unwrap_or(&[])
                                .iter()
                                .filter(|t| t.pid == p.pid)
                                .peekable();
                            if mine.peek().is_none() && p.threads == Some(1) {
                                // A single-threaded process is not collected —
                                // it *is* its only thread, so a row for it
                                // would repeat the process one column narrower.
                                // Its state is that thread's state, and without
                                // this the commonest contributor to `BLOCKED`
                                // is the one process `task = D` can never find.
                                p.state.eq_ignore_ascii_case(&want)
                            } else {
                                mine.any(|t| t.state.eq_ignore_ascii_case(&want))
                            }
                        }
                    }
                    // Containment, because `name = node` should find
                    // `node /srv/api/server.js` — which is what anyone typing
                    // it means, and the reason the command line is worth
                    // showing at all.
                    _ => p.command().to_lowercase().contains(value),
                };
                match op {
                    Op::Eq => hit,
                    Op::Ne => !hit,
                    // `parse_term` refuses an ordering on a text field, so this
                    // cannot fire. Stated rather than left to a lexicographic
                    // fallback: if an operator is ever added, `state < x`
                    // should be an error and not a nonsense answer.
                    _ => false,
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
        Field::State | Field::Thread | Field::User | Field::Name => return None,
    })
}

fn compare(have: f64, op: Op, want: f64, field: Field) -> bool {
    let band = eq_band(field, want);
    match op {
        Op::Gt => have > want,
        Op::Ge => have >= want,
        Op::Lt => have < want,
        Op::Le => have <= want,
        Op::Eq => (have - want).abs() <= band,
        Op::Ne => (have - want).abs() > band,
    }
}

/// How close counts as equal, which depends on how the column is written.
///
/// A single band does not work across these fields. Half a unit of the last
/// displayed digit is right for a CPU percentage, and on a byte quantity the
/// same number is exact-byte equality — so `mem = 512m` found nothing unless
/// the RSS was 536870912 exactly, and `mem != 512m` was true of every process
/// on the machine.
fn eq_band(field: Field, want: f64) -> f64 {
    match field {
        // The column shows one decimal, so anything that rounds to the same
        // figure is the same figure.
        Field::Cpu => 0.05,
        // Bytes are shown to three or four significant figures in whichever
        // unit fits, so half a percent is comfortably inside the last digit
        // drawn and comfortably outside "a different process".
        Field::Rss | Field::Read | Field::Write => (want * 0.005).max(1.0),
        // Counts. A pid is exact or it is a different process.
        _ => 0.0,
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
///
/// A split that leaves nothing on either side is not a split either: `/and` on
/// its own is somebody a third of the way through typing `android`, and
/// unfiltering the table under them is a worse answer than matching the letters
/// they have typed so far.
fn split_and(input: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = input;
    while let Some(i) = find_and(rest) {
        out.push(&rest[..i]);
        rest = &rest[i + 3..];
    }
    out.push(rest);
    if out.iter().all(|s| s.trim().is_empty()) {
        return vec![input];
    }
    out
}

/// The byte offset of a standalone `and`, ignoring ASCII case.
///
/// Searched over the original string, never over `to_lowercase()`. Lowercasing
/// can change a string's length — `İ` is one char and two bytes, and its
/// lowercase is two chars and three — so a byte offset found in the lowered
/// copy and used to slice the original runs past the end or lands mid-char. It
/// panicked: four keystrokes into the filter box took the whole tool down, and
/// on the panic path the terminal is left in raw mode.
fn find_and(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    let boundary = |i: usize| -> bool {
        // Byte-indexed, but every check is on an ASCII neighbour or on being
        // at an edge, so a multi-byte char can only ever answer "not
        // whitespace", which is the safe answer.
        let before = s[..i].chars().next_back();
        let after = s[i + 3..].chars().next();
        before.is_none_or(char::is_whitespace) && after.is_none_or(char::is_whitespace)
    };
    (0..bytes.len().saturating_sub(2))
        .filter(|i| s.is_char_boundary(*i) && s.is_char_boundary(i + 3))
        .find(|i| bytes[*i..*i + 3].eq_ignore_ascii_case(b"and") && boundary(*i))
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
        // Not a field, so this was never a comparison. Command lines are full
        // of `=`: `chrome --type=renderer`, `java -Dconfig=/etc/app.conf`,
        // `node --inspect=9229`. Reading those as a malformed query and
        // refusing to filter is a regression on the thing the filter is most
        // used for — finding which of four node processes is the API server,
        // and the answer being in the arguments.
        //
        // A word that *is* a field still gets a real error, so `cpu > lots`
        // says what is wrong rather than silently searching for `cpu > lots` as
        // text and finding nothing.
        if looks_like_a_field(&field_word) {
            return Err(format!(
                "no field called `{field_word}` — try {}",
                field_names()
            ));
        }
        return Ok(Term::Substring(term.to_lowercase()));
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

/// Whether a word was plausibly *meant* as a field name.
///
/// The line between "you misspelled a field" and "that is an argument with an
/// equals sign in it" has to fall somewhere, and neither extreme is right.
/// Erroring on every unknown word refuses `--type=renderer`, which is the thing
/// the filter is most used for. Never erroring makes `cpuu > 5` search silently
/// for the literal text and find nothing, which on a one-line filter box means
/// the reader has no way to learn the field names at all.
///
/// So: a near miss is a misspelling and gets the error; anything else is text.
/// Near enough means one is a prefix of the other with at least three letters
/// in common, which catches the typos people actually make — `cpuu`, `memm`,
/// `thread`, `stat` — and lets `lang=en` and `type=renderer` through.
fn looks_like_a_field(word: &str) -> bool {
    FIELDS.iter().any(|(name, _)| {
        let shared = name.starts_with(word) || word.starts_with(name);
        shared && word.len().min(name.len()) >= 3
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

#[cfg(test)]
mod review_tests {
    use super::*;
    use crate::sample::IoRates;
    use std::sync::Arc;

    fn proc_of(name: &str, user: &str, state: char) -> ProcSample {
        ProcSample {
            pid: 4821,
            ppid: 1,
            name: Arc::from(name),
            user: Arc::from(user),
            cpu: 12.5,
            rss: 512 << 20,
            threads: Some(41),
            state,
            started: Some(1),
            cmd: Some(Arc::from(name)),
            io: Some(IoRates { read: 0, write: 0 }),
        }
    }

    #[test]
    fn no_filter_text_can_panic() {
        // `find_and` searched `to_lowercase()` and sliced the original.
        // Lowercasing can change a string's length — `İ` is one char and two
        // bytes, its lowercase two chars and three — so the offset ran past the
        // end. Four keystrokes into the filter box took the whole tool down,
        // and on the panic path the terminal is left in raw mode.
        let nasty = [
            "İand",
            "İ and",
            "İİ and",
            "ẞand",
            "ǅ and x",
            "\u{130}\u{130}\u{130}and",
            "and",
            "AND",
            " and ",
            "aNd",
            "андроид",
            "日本語 and cpu > 1",
            "…and…",
            "\u{0}and",
            "a\u{300}nd",
        ];
        for s in nasty {
            // Every prefix, because the filter is parsed on every keystroke.
            for i in 0..=s.len() {
                if !s.is_char_boundary(i) {
                    continue;
                }
                let _ = parse(&s[..i]);
            }
        }
    }

    #[test]
    fn a_command_line_with_an_equals_sign_is_still_searched_for() {
        // Command lines are full of them, and reading those as a malformed
        // query and refusing to filter is a regression on the thing the filter
        // is most used for.
        let p = proc_of("chrome --type=renderer --lang=en-US", "deploy", 'S');
        for q in [
            "--type=renderer",
            "-Dconfig=/etc/app.conf",
            "--inspect=9229",
            "lang=en",
        ] {
            let parsed = parse(q).unwrap_or_else(|e| panic!("`{q}` was refused: {e}"));
            assert!(
                !parsed.is_empty(),
                "`{q}` parsed to nothing and filters nothing"
            );
        }
        assert!(parse("--type=renderer").unwrap().matches(&p));
        assert!(!parse("--type=gpu").unwrap().matches(&p));

        // …and a near miss is still a real error, so `cpuu > 5` says so rather
        // than searching for it as text and finding nothing. That is the whole
        // discovery mechanism a one-line filter box has.
        for typo in ["cpuu > 5", "memm > 1g", "thread > 4", "stat = D"] {
            assert!(parse(typo).is_err(), "`{typo}` was read as text");
        }
    }

    #[test]
    fn a_size_is_equal_at_the_precision_the_column_draws() {
        // The 0.05 band is right for a percentage and is exact-byte equality on
        // a byte field: `mem = 512m` found nothing unless the RSS was
        // 536870912 exactly, and `mem != 512m` was true of every process.
        let mut p = proc_of("node", "deploy", 'S');
        p.rss = (512 << 20) + (400 << 10); // 512.4 MiB
        assert!(
            parse("mem = 512m").unwrap().matches(&p),
            "a process the column draws as 512.4M does not equal 512m"
        );
        assert!(!parse("mem != 512m").unwrap().matches(&p));
        // Still not equal to a different figure.
        assert!(!parse("mem = 600m").unwrap().matches(&p));

        // A pid is exact or it is a different process.
        assert!(parse("pid = 4821").unwrap().matches(&p));
        assert!(!parse("pid = 4822").unwrap().matches(&p));
        // …and so is a thread count.
        assert!(parse("threads = 41").unwrap().matches(&p));
        assert!(!parse("threads = 42").unwrap().matches(&p));
    }

    #[test]
    fn every_state_the_kernel_can_report_is_queryable() {
        // A lookup table over the states this tool happens to have seen
        // collapsed `t`, `W`, `X`, `K` and `P` into one `?`, so a traced
        // process could never be found and unrelated states compared equal.
        for s in ['R', 'S', 'D', 'T', 't', 'Z', 'I', 'W', 'X', 'K', 'P', '?'] {
            let p = proc_of("x", "deploy", s);
            let q = format!("state = {s}");
            assert!(
                parse(&q).unwrap().matches(&p),
                "`{q}` does not match a process in state {s}"
            );
            for other in ['R', 'D', 'Z'] {
                if other.eq_ignore_ascii_case(&s) {
                    continue;
                }
                assert!(
                    !parse(&format!("state = {other}")).unwrap().matches(&p),
                    "state {s} compared equal to state {other}"
                );
            }
        }
    }

    #[test]
    fn an_identity_field_is_matched_exactly() {
        // `user = root` matching `rootless` is not what anyone typing it means,
        // and `user != root` excluding them is worse.
        let root = proc_of("x", "root", 'S');
        let other = proc_of("x", "rootless", 'S');
        assert!(parse("user = root").unwrap().matches(&root));
        assert!(!parse("user = root").unwrap().matches(&other));
        assert!(parse("user != root").unwrap().matches(&other));

        // A name is still containment, which is why the command line is worth
        // showing at all.
        let node = proc_of("node /srv/api/server.js", "deploy", 'S');
        assert!(parse("name = node").unwrap().matches(&node));
        assert!(parse("name = server.js").unwrap().matches(&node));
    }

    #[test]
    fn a_bare_and_is_someone_typing_android() {
        // Unfiltering the table a third of the way through a word is a worse
        // answer than matching the letters typed so far.
        let p = proc_of("android-studio", "deploy", 'S');
        let other = proc_of("postgres", "deploy", 'S');
        for q in ["and", "AND", " and "] {
            let parsed = parse(q).unwrap();
            assert!(!parsed.is_empty(), "`{q}` unfiltered the table");
            assert!(parsed.matches(&p), "`{q}` does not match android-studio");
            assert!(!parsed.matches(&other), "`{q}` matched postgres");
        }
    }
}
