//! Measuring a theme against the guarantee the built-in palettes are held to.
//!
//! poptop is the only monitor that measures its own palette: CI fails if any
//! pair of meaning-bearing hues drops below ΔE 8 under Machado 2009 simulation,
//! or if a colour falls below 3:1 against a background it is actually drawn
//! over. That guarantee is the whole point of the colour work.
//!
//! The moment users can supply themes it evaporates — unless the validator is
//! turned outward. So this module is what `--check-theme` prints *and* what the
//! palette tests assert: one implementation, so a contributed theme is measured
//! by the same instrument as the built-ins, and neither can drift from the
//! other by having its own copy.

use crate::cvd::{self, CVD_TARGET, Cvd};
use crate::theme::{Theme, Token};
use std::fmt;

/// Below this a colour carrying information is not reliably legible.
///
/// WCAG's threshold for large text and graphical objects, which is what a bar
/// glyph and a two-decimal figure are.
pub const MIN_CONTRAST: f64 = 3.0;

/// The floor for the parts that are meant to recede.
///
/// Chrome and dim text are deliberately quiet — a border that competes with
/// the numbers inside it is a worse border — so holding them to the
/// information threshold would fail every theme including the built-ins, and a
/// check that fails everything says nothing. They still have a floor: a border
/// nobody can find is worse than no border. The visual hierarchy itself
/// (chrome < dim < data) is asserted separately in `theme.rs`; this only asks
/// that the quiet end stays visible.
pub const MIN_RECESSIVE: f64 = 1.5;

/// The panel background poptop draws over.
///
/// A constant rather than a reading of the terminal: poptop never sets a
/// background, so the real one belongs to the user's terminal theme and cannot
/// be known. This is a dark surface typical of the terminals poptop is designed
/// against, and a figure measured against a stated assumption beats no figure.
pub const SURFACE: [u8; 3] = [0x1a, 0x1a, 0x19];

/// The tokens that carry meaning, and so must be told apart from each other.
///
/// Chrome and text are excluded: they are not asked to be distinguished, only
/// to recede. `live` is excluded because it deliberately shares the `ok` hue.
const MEANINGFUL: [Token; 5] = [
    Token::Ok,
    Token::Warn,
    Token::Critical,
    Token::SeriesCpu,
    Token::SeriesMem,
];

/// Every token drawn as foreground, all of which have to be *visible* even
/// where they do not have to be distinct.
///
/// Separation and legibility are different questions and were wrongly sharing
/// one list: scoping contrast to [`MEANINGFUL`] meant a theme could set `text`
/// and `text_dim` to 1.01:1 against the surface — the entire interface
/// invisible — and be reported as passing.
/// Each with the floor its job asks for, and the backgrounds it is drawn over.
///
/// The second is not a detail: chrome is dividers and titles, and the process
/// table has had no side borders since the panel became a full-width content
/// line — so chrome never crosses a selected row. Measuring it there would
/// report a real-looking 1.37:1 for a combination that never appears on
/// screen, and a check that fails on things that cannot happen trains people
/// to ignore it.
const DRAWN: [(Token, f64, bool); 9] = [
    (Token::Ok, MIN_CONTRAST, true),
    (Token::Warn, MIN_CONTRAST, true),
    (Token::Critical, MIN_CONTRAST, true),
    (Token::SeriesCpu, MIN_CONTRAST, true),
    (Token::SeriesMem, MIN_CONTRAST, true),
    (Token::Text, MIN_CONTRAST, true),
    (Token::Live, MIN_CONTRAST, true),
    (Token::TextDim, MIN_RECESSIVE, true),
    (Token::Chrome, MIN_RECESSIVE, false),
];

/// What a check concluded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Pass,
    Fail,
    /// Something could not be measured, so nothing can be concluded.
    ///
    /// Its own outcome rather than a quiet pass. ANSI names and the low indices
    /// are *slots*, and what they look like belongs to the user's terminal
    /// theme — poptop genuinely cannot know. Treating that as "no problems found"
    /// meant `--check-theme` certified anything on a terminal without 256
    /// colours, which is most CI jobs: exactly where the README says to run it.
    Incomplete,
}

impl Verdict {
    /// Anything but a pass leaves the shell non-zero. A check that could not
    /// see the colours has not passed them, and a script asking "is this theme
    /// legible" must not be told yes by silence.
    pub fn exit_code(self) -> i32 {
        i32::from(self != Verdict::Pass)
    }

    fn label(self) -> &'static str {
        match self {
            Verdict::Pass => "PASS",
            Verdict::Fail => "FAIL",
            Verdict::Incomplete => "INCOMPLETE",
        }
    }
}

/// How far apart two meaning-bearing colours are, and for whom they are worst.
pub struct Pair {
    pub a: &'static str,
    pub b: &'static str,
    pub delta_e: f64,
    /// The vision under which the pair is closest, or `None` for normal vision.
    ///
    /// Normal vision is included because the tritan matrix has entries above 1,
    /// so a simulation can *increase* separation: a pair too close for everyone
    /// could otherwise pass a deficiency-only check.
    pub worst_for: Option<Cvd>,
}

impl Pair {
    pub fn passes(&self) -> bool {
        self.delta_e >= CVD_TARGET
    }
}

/// Whether a colour can be seen at all on a background it is drawn over.
pub struct Legibility {
    pub token: &'static str,
    pub background: &'static str,
    pub ratio: f64,
    /// What this token has to clear, which depends on what it is for.
    pub floor: f64,
}

impl Legibility {
    pub fn passes(&self) -> bool {
        self.ratio >= self.floor
    }
}

/// Everything measurable about one theme, and what could not be measured.
pub struct Report {
    pub name: String,
    pub pairs: Vec<Pair>,
    pub legibility: Vec<Legibility>,
    /// Tokens whose colour poptop cannot know.
    ///
    /// An ANSI name or an index below 16 is a *slot*; what it looks like
    /// belongs to the user's terminal theme. Reported rather than dropped: a
    /// shorter table that still said PASS could not be read as coverage, and
    /// `--check-theme` is the artifact a contributed theme arrives with.
    pub unmeasured: Vec<&'static str>,
    /// Whether the selected-row background is itself an unknowable slot.
    ///
    /// Previously this fell back to the surface colour, so the report printed
    /// the surface figures under the "selected row" label — a false PASS on
    /// fabricated data, which is the one thing this module exists not to do.
    pub selection_unmeasured: bool,
    /// Why a failure is nonetheless intended, when it is.
    ///
    /// `classic` fails on purpose — it exists to restore the green/yellow
    /// convention, and green/yellow is the pair that convention gets wrong.
    /// Reporting that as a bare FAIL would read as poptop failing its own check
    /// rather than as the choice it is.
    pub caveat: Option<&'static str>,
}

impl Report {
    pub fn with_caveat(mut self, caveat: Option<&'static str>) -> Self {
        self.caveat = caveat;
        self
    }

    pub fn of(name: &str, theme: &Theme) -> Self {
        // Split rather than filtered: what could not be measured is part of
        // the result, not something to drop on the way to one.
        let mut unmeasured = Vec::new();
        let resolve = |t: Token| cvd::to_rgb(t.get(theme)).map(|rgb| (t.name(), rgb));

        let meaningful: Vec<(&'static str, [u8; 3])> =
            MEANINGFUL.iter().filter_map(|&t| resolve(t)).collect();

        let mut drawn = Vec::new();
        for (token, floor, over_selection) in DRAWN {
            match resolve(token) {
                Some((name, rgb)) => drawn.push((name, rgb, floor, over_selection)),
                None => unmeasured.push(token.name()),
            }
        }

        let mut pairs = Vec::new();
        for (i, &(an, a)) in meaningful.iter().enumerate() {
            for &(bn, b) in &meaningful[i + 1..] {
                let (delta_e, worst_for) = cvd::worst_cvd(a, b);
                pairs.push(Pair {
                    a: an,
                    b: bn,
                    delta_e,
                    worst_for,
                });
            }
        }

        // Both backgrounds, because a colour is drawn over both and clearing
        // one says nothing about the other. The first 256-colour palette
        // cleared ΔE comfortably while sitting at 2.03:1 on the selected row.
        let selected = cvd::to_rgb(theme.selection_bg);
        let mut legibility = Vec::new();
        for &(name, rgb, floor, over_selection) in &drawn {
            legibility.push(Legibility {
                token: name,
                background: "surface",
                ratio: cvd::contrast(rgb, SURFACE),
                floor,
            });
            if let Some(bg) = selected.filter(|_| over_selection) {
                legibility.push(Legibility {
                    token: name,
                    background: "selected row",
                    ratio: cvd::contrast(rgb, bg),
                    floor,
                });
            }
        }

        Self {
            name: name.to_string(),
            pairs,
            legibility,
            unmeasured,
            selection_unmeasured: selected.is_none(),
            caveat: None,
        }
    }

    /// The closest pair, which is the number that decides the whole theme.
    pub fn worst_pair(&self) -> Option<&Pair> {
        self.pairs
            .iter()
            .min_by(|x, y| x.delta_e.total_cmp(&y.delta_e))
    }

    pub fn worst_contrast(&self) -> Option<&Legibility> {
        self.legibility
            .iter()
            .min_by(|x, y| x.ratio.total_cmp(&y.ratio))
    }

    pub fn verdict(&self) -> Verdict {
        // A real failure outranks an incomplete check: something measurable is
        // definitely wrong, and that is the more useful thing to say.
        if self.pairs.iter().any(|p| !p.passes()) || self.legibility.iter().any(|l| !l.passes()) {
            return Verdict::Fail;
        }
        if !self.unmeasured.is_empty() || self.selection_unmeasured || self.pairs.is_empty() {
            return Verdict::Incomplete;
        }
        Verdict::Pass
    }

    /// One line for a theme that loads anyway.
    ///
    /// A failing theme still loads. It is the user's terminal and their choice;
    /// poptop's job is to have the number and say it, not to refuse — the same
    /// principle as rendering `—` rather than a fabricated zero.
    ///
    /// Only a real failure is worth a line at startup. An incomplete check is
    /// the normal state of a theme written in ANSI names, and saying so on
    /// every run would be noise about something the user cannot fix.
    pub fn warning(&self) -> Option<String> {
        if self.verdict() != Verdict::Fail {
            return None;
        }
        let mut why = Vec::new();
        if let Some(p) = self.worst_pair().filter(|p| !p.passes()) {
            why.push(format!(
                "{} and {} are only ΔE {:.1} apart{}",
                p.a,
                p.b,
                p.delta_e,
                p.worst_for
                    .map_or(String::new(), |k| format!(" ({k:?})"))
                    .to_lowercase()
            ));
        }
        if let Some(l) = self.worst_contrast().filter(|l| !l.passes()) {
            why.push(format!(
                "{} is {:.2}:1 on the {}",
                l.token, l.ratio, l.background
            ));
        }
        Some(format!(
            "theme `{}`: {} — run `poptop --check-theme {}` for the rest",
            self.name,
            why.join(", "),
            self.name
        ))
    }
}

impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{}: {}", self.name, self.verdict().label())?;

        // Every table that has anything in it, unconditionally. An early
        // return on "no pairs" used to swallow the legibility table too, so a
        // theme that failed on contrast printed "nothing to measure" and
        // exited non-zero with no reason given — pointing the user at a
        // command that contradicted the warning that sent them to it.
        //
        // Every pair, not only the failures: a theme passing at ΔE 8.1 is a
        // different thing from one passing at 30, and the number is the point.
        for p in &self.pairs {
            let vision = p
                .worst_for
                .map_or("normal".to_string(), |k| format!("{k:?}").to_lowercase());
            let row = format!(
                "  {:<11} ↔ {:<11} ΔE {:5.1}  {vision:<8}",
                p.a, p.b, p.delta_e
            );
            match p.passes() {
                true => writeln!(f, "{}", row.trim_end())?,
                false => writeln!(f, "{row}  below the target of {CVD_TARGET:.0}")?,
            }
        }
        for l in &self.legibility {
            let row = format!(
                "  {:<11} on {:<13} {:>6.2}:1        ",
                l.token, l.background, l.ratio
            );
            match l.passes() {
                true => writeln!(f, "{}", row.trim_end())?,
                false => writeln!(f, "{row}  below {:.1}:1", l.floor)?,
            }
        }

        if !self.pairs.is_empty() || !self.legibility.is_empty() {
            let worst_pair = self.worst_pair().map_or(f64::NAN, |p| p.delta_e);
            let worst_contrast = self.worst_contrast().map_or(f64::NAN, |l| l.ratio);
            writeln!(
                f,
                "  worst pair: ΔE {worst_pair:.1}   worst contrast: {worst_contrast:.2}:1"
            )?;
        }

        // What was not measured, named. A shorter table that still said PASS
        // could not be read as coverage, which is the one job this output has.
        if !self.unmeasured.is_empty() || self.selection_unmeasured {
            writeln!(f)?;
            let mut what: Vec<&str> = self.unmeasured.clone();
            if self.selection_unmeasured {
                what.push("selection_bg");
            }
            for line in wrap(
                &format!(
                    "not measured: {}. An ANSI name or an index below 16 is a slot, and \
                     what it looks like belongs to your terminal theme rather than to poptop — \
                     there is no hue here to measure. Spell these as `#rrggbb` or a \
                     256-colour index to have them checked.",
                    what.join(", ")
                ),
                74,
            ) {
                writeln!(f, "  {line}")?;
            }
        }

        let Some(why) = self.caveat else {
            return Ok(());
        };
        writeln!(f)?;
        // Wrapped here rather than left as one long line: the rest of the
        // report is a column layout, and a paragraph running off the right of
        // it undoes the reason the columns are there.
        for line in wrap(why, 74) {
            writeln!(f, "  {line}")?;
        }
        Ok(())
    }
}

/// Greedy wrap to `width` columns.
///
/// Hand-rolled because the alternative is a dependency for eight lines, in a
/// project whose `/proc` parser is hand-rolled for the same reason.
fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines = vec![String::new()];
    for word in text.split_whitespace() {
        let line = lines.last_mut().expect("never empty");
        if line.is_empty() {
            *line = word.to_string();
        } else if line.chars().count() + 1 + word.chars().count() <= width {
            line.push(' ');
            line.push_str(word);
        } else {
            lines.push(word.to_string());
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{Palette, Tier, Token};
    use ratatui::style::Color;

    fn themed(overrides: &[(Token, Color)]) -> Theme {
        Theme::new(Palette::Safe, Tier::TrueColor)
            .with_overrides(overrides)
            .0
    }

    #[test]
    fn every_pair_and_both_backgrounds_are_reported() {
        // Five meaning-bearing colours is ten pairs, and each is measured
        // against both surfaces it is drawn over. A report that showed only
        // failures could not tell a theme passing at 8.1 from one at 30, and
        // the number is the point.
        let report = Report::of("safe", &themed(&[]));
        assert_eq!(report.pairs.len(), 10, "five meaning colours is ten pairs");
        // Nine drawn tokens against two backgrounds, less chrome, which is
        // never drawn over a selected row.
        assert_eq!(report.legibility.len(), 17);
        // Counted in the rendered text, not just in the data. Asserting that
        // token names appear is satisfied by the contrast rows alone, so a
        // report that printed only its failures still passed.
        let text = report.to_string();
        assert_eq!(
            text.lines().filter(|l| l.contains('↔')).count(),
            10,
            "not every pair was printed:\n{text}"
        );
        assert_eq!(
            text.lines().filter(|l| l.contains(" on ")).count(),
            17,
            "not every background was printed:\n{text}"
        );
        for token in ["ok", "warn", "critical", "series_cpu", "series_mem"] {
            assert!(text.contains(token), "{token} missing from:\n{text}");
        }
        assert!(
            text.contains("surface") && text.contains("selected row"),
            "{text}"
        );
        assert!(
            text.contains("worst pair") && text.contains("worst contrast"),
            "{text}"
        );
    }

    #[test]
    fn a_theme_that_cannot_be_told_apart_fails_and_names_the_pair() {
        // Two identical hues is the extreme of the thing being measured.
        let report = Report::of(
            "flat",
            &themed(&[
                (Token::Ok, Color::Rgb(0x5c, 0xcf, 0xe6)),
                (Token::Warn, Color::Rgb(0x5c, 0xcf, 0xe6)),
            ]),
        );
        assert_eq!(report.verdict(), Verdict::Fail);
        let worst = report.worst_pair().unwrap();
        assert_eq!((worst.a, worst.b), ("ok", "warn"));
        assert!(
            worst.delta_e < 0.001,
            "identical hues measured {} apart",
            worst.delta_e
        );
        assert!(report.to_string().contains("FAIL"));
    }

    #[test]
    fn an_illegible_colour_fails_on_the_background_it_is_drawn_over() {
        // Separation between hues says nothing about being visible at all: the
        // first 256-colour palette cleared ΔE while sitting at 2.03:1 on the
        // selected row.
        let report = Report::of(
            "dark",
            &themed(&[(Token::Critical, Color::Rgb(0x22, 0x22, 0x22))]),
        );
        assert_eq!(report.verdict(), Verdict::Fail);
        let worst = report.worst_contrast().unwrap();
        assert_eq!(worst.token, "critical");
        assert!(worst.ratio < MIN_CONTRAST);
    }

    #[test]
    fn a_failing_theme_still_loads_and_says_why_once() {
        // It is the user's terminal and their choice; poptop's job is to have the
        // number and say it, not to refuse — the same principle as rendering
        // `—` rather than a fabricated zero.
        let report = Report::of(
            "flat",
            &themed(&[(Token::Warn, Color::Rgb(0x5c, 0xcf, 0xe6))]),
        );
        let warning = report.warning().expect("a failing theme should say so");
        assert!(warning.contains("flat"), "{warning}");
        assert!(warning.contains("--check-theme flat"), "{warning}");
        assert_eq!(warning.lines().count(), 1, "more than one line: {warning}");

        // …and a passing theme says nothing at all.
        assert_eq!(Report::of("safe", &themed(&[])).warning(), None);
    }

    #[test]
    fn normal_vision_can_be_the_worst_case() {
        // The tritan matrix has entries above 1, so a simulation can *increase*
        // separation. These two blues are ΔE 5.1 apart to normal vision and
        // further apart to every deficiency — so a deficiency-only check would
        // pass a pair nobody can tell apart, and a report that could not name
        // normal vision would print a number without its subject.
        let report = Report::of(
            "blues",
            &themed(&[
                (Token::Ok, Color::Rgb(0x00, 0x66, 0xbb)),
                (Token::Warn, Color::Rgb(0x00, 0x77, 0xcc)),
            ]),
        );
        let pair = report
            .pairs
            .iter()
            .find(|p| (p.a, p.b) == ("ok", "warn"))
            .unwrap();
        assert_eq!(pair.worst_for, None, "normal vision was not the worst case");
        assert!(!pair.passes(), "ΔE {:.1} should fail", pair.delta_e);
        assert!(report.to_string().contains("normal"), "{report}");
    }

    #[test]
    fn a_check_that_measured_nothing_is_not_a_pass() {
        // The bug this locks out: `--check-theme` read the *detected* tier, so
        // in CI — where TERM is often unset and detection lands on monochrome —
        // nothing was measurable, `passes()` was vacuously true, and a theme
        // with zero separation and 1:1 contrast was certified by the same
        // command the README tells people to run.
        let report = Report::of("safe", &Theme::new(Palette::Safe, Tier::Mono));
        assert!(report.pairs.is_empty());
        assert_eq!(report.verdict(), Verdict::Incomplete);
        assert_ne!(
            report.verdict().exit_code(),
            0,
            "a check that saw no colours reported success"
        );
        assert!(report.to_string().contains("INCOMPLETE"));
        assert!(report.to_string().contains("not measured"));
    }

    #[test]
    fn a_partly_measurable_theme_says_what_it_skipped() {
        // A shorter table that still said PASS could not be read as coverage,
        // and coverage is the only thing this output is for.
        let report = Report::of("mixed", &themed(&[(Token::Ok, Color::Cyan)]));
        assert!(report.unmeasured.contains(&"ok"), "{report}");
        assert_eq!(report.verdict(), Verdict::Incomplete);
        assert!(report.to_string().contains("not measured: ok"), "{report}");
    }

    #[test]
    fn an_unknowable_selection_background_is_not_quietly_the_surface() {
        // It used to fall back to SURFACE, so the report printed the surface
        // figures under the "selected row" label — a PASS on fabricated data,
        // which is the one thing this module exists not to do. With
        // `selection_bg = white` every meaning colour is far below 3:1 there,
        // and poptop said PASS.
        let report = Report::of("slotted", &themed(&[(Token::SelectionBg, Color::White)]));
        assert!(report.selection_unmeasured);
        assert!(
            !report
                .legibility
                .iter()
                .any(|l| l.background == "selected row"),
            "figures were invented for an unknowable background"
        );
        assert_eq!(report.verdict(), Verdict::Incomplete);
        assert!(report.to_string().contains("selection_bg"), "{report}");
    }

    #[test]
    fn text_that_cannot_be_read_fails_even_though_it_carries_no_status() {
        // Separation and legibility are different questions, and scoping
        // contrast to the meaning-bearing five meant a theme could make the
        // entire interface invisible and pass.
        let invisible = Color::Rgb(0x1b, 0x1b, 0x1a);
        for token in [Token::Text, Token::Live, Token::TextDim] {
            let report = Report::of("invis", &themed(&[(token, invisible)]));
            assert_eq!(
                report.verdict(),
                Verdict::Fail,
                "{} at 1.01:1 was accepted:\n{report}",
                token.name()
            );
        }
    }

    #[test]
    fn the_recessive_parts_are_held_to_their_own_floor() {
        // Chrome is meant to recede — a border competing with the numbers
        // inside it is a worse border — so holding it to the information
        // threshold would fail every theme including the built-ins, and a
        // check that fails everything says nothing.
        let report = Report::of("safe", &themed(&[]));
        let chrome: Vec<&Legibility> = report
            .legibility
            .iter()
            .filter(|l| l.token == "chrome")
            .collect();
        assert_eq!(chrome.len(), 1, "chrome is not drawn over a selected row");
        assert_eq!(chrome[0].floor, MIN_RECESSIVE);
        assert!(chrome[0].ratio < MIN_CONTRAST, "chrome is not recessive");
        assert!(chrome[0].passes(), "chrome is too faint to find");
    }
}
