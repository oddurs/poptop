//! Variations on real input, for tests whose question is "can anything here
//! panic".
//!
//! Every parser in poptop is tested against the files its author thought to
//! write. That shows it reads those; it says nothing about the file a kernel
//! three versions from now writes, the line a crash cut in half, or the name an
//! unprivileged user chose so that it would not parse. These tests take a real
//! input and bend it every way that has broken a parser before — a line
//! missing, a line doubled, a number at the edge of its type, a byte that is
//! not UTF-8, the file cut at every line — and then some ways at random.
//!
//! Deterministic, and deliberately not a dependency. The seed is fixed, so a
//! failure reproduces exactly from the test name; and a property-testing crate
//! would be the first dev-dependency this crate has, for a job forty lines do.
//! Coverage-guided search is `fuzz/`'s job. This is the part that runs on
//! every `cargo test`, on both platforms, with no nightly compiler.

/// A small, fixed-seed generator: xorshift64*.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        // Zero is xorshift's one fixed point.
        Self(seed.max(1))
    }

    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    /// A number in `0..n`; zero when `n` is.
    pub fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next() % n as u64) as usize
        }
    }
}

/// What a number is replaced with. Each has broken a parser somewhere: an
/// overflow, a sign where none was expected, a float where an integer was, an
/// empty field that `split_whitespace` quietly removes.
const EXTREMES: &[&str] = &[
    "0",
    "-1",
    "18446744073709551615",
    "18446744073709551616",
    "340282366920938463463374607431768211456",
    "-9223372036854775808",
    "4294967295",
    "4294967296",
    "65535",
    "nan",
    "inf",
    "1e308",
    "",
];

/// Bytes worth inserting: the separators parsers split on, the brackets `comm`
/// is wrapped in, and bytes that are not UTF-8 at all.
const AWKWARD: &[u8] = b"()\n \t:=,#\\\x00\xff\xc3\x80";

/// Deterministic variants of `seed`, then `random` random ones.
///
/// The deterministic ones are the cases worth naming: the file cut short at
/// every line, each line removed, each line doubled, and each number in turn
/// replaced by each extreme. The random ones stack a few byte-level changes.
pub fn variants(seed: &[u8], random: usize) -> Vec<Vec<u8>> {
    let mut out = vec![Vec::new(), seed.to_vec()];

    let lines: Vec<&[u8]> = seed.split_inclusive(|b| *b == b'\n').collect();
    for i in 0..lines.len() {
        out.push(lines[..i].concat());
        out.push(
            lines[..i]
                .concat()
                .into_iter()
                .chain(lines[i].iter().copied().take(lines[i].len() / 2))
                .collect(),
        );
        let mut without = lines.clone();
        without.remove(i);
        out.push(without.concat());
        let mut doubled = lines.clone();
        doubled.insert(i, lines[i]);
        out.push(doubled.concat());
    }

    for (from, to) in numbers(seed) {
        for e in EXTREMES {
            let mut v = seed[..from].to_vec();
            v.extend_from_slice(e.as_bytes());
            v.extend_from_slice(&seed[to..]);
            out.push(v);
        }
    }

    let mut rng = Rng::new(0x5eed ^ seed.len() as u64);
    for _ in 0..random {
        let mut v = seed.to_vec();
        for _ in 0..1 + rng.below(4) {
            mutate(&mut v, &mut rng);
        }
        out.push(v);
    }
    out
}

/// Where the runs of ASCII digits are, as `(start, end)`.
fn numbers(s: &[u8]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < s.len() {
        if s[i].is_ascii_digit() {
            let start = i;
            while i < s.len() && s[i].is_ascii_digit() {
                i += 1;
            }
            out.push((start, i));
        } else {
            i += 1;
        }
    }
    out
}

fn mutate(v: &mut Vec<u8>, rng: &mut Rng) {
    let at = rng.below(v.len() + 1);
    match rng.below(6) {
        0 if !v.is_empty() => {
            let i = rng.below(v.len());
            v[i] ^= 1 << rng.below(8);
        }
        1 => v.insert(at, AWKWARD[rng.below(AWKWARD.len())]),
        2 => {
            let len = rng.below(v.len() - at + 1);
            v.drain(at..at + len);
        }
        3 => {
            let len = rng.below((v.len() - at).min(64) + 1);
            let copy = v[at..at + len].to_vec();
            let to = rng.below(v.len() + 1);
            v.splice(to..to, copy);
        }
        4 => v.truncate(at),
        _ => {
            let e = EXTREMES[rng.below(EXTREMES.len())].as_bytes();
            v.splice(at..at, e.iter().copied());
        }
    }
}

/// The variants as text, the way a reader that decodes lossily sees them.
pub fn text_variants(seed: &str, random: usize) -> Vec<String> {
    variants(seed.as_bytes(), random)
        .into_iter()
        .map(|v| String::from_utf8_lossy(&v).into_owned())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_variants_are_the_same_every_run() {
        // A failure has to reproduce from the test name alone.
        assert_eq!(variants(b"a 1\nb 22\n", 50), variants(b"a 1\nb 22\n", 50));
    }

    #[test]
    fn every_number_meets_every_extreme() {
        let v = variants(b"x 7\n", 0);
        for e in EXTREMES {
            let want = format!("x {e}\n");
            assert!(
                v.iter().any(|x| x == want.as_bytes()),
                "no variant with {e:?}"
            );
        }
    }
}
