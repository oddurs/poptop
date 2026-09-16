//! The menu bar: File, Edit, View, Go, Process, Help.
//!
//! A discoverability surface, not a second set of commands. Every item holds an
//! [`Action`], and pressing a key and choosing an item go through the same
//! [`Action::apply`] — see `command.rs` for why that matters.
//!
//! poptop has around thirty single-key bindings. That is fine once you know
//! them and impenetrable before: the footer can list six, and the rest are in a
//! manual nobody has open. A menu states all of them, beside the keys that do
//! the same thing, so it teaches itself out of use.

use crate::app::App;
use crate::command::Action;
use crate::glyphs::{Axis, GlyphSet};

/// One row of a dropdown.
pub enum Item {
    /// A command, its label, and the key that also does it.
    Do(&'static str, &'static str, Action),
    /// A horizontal rule between groups.
    Rule,
}

impl Item {
    pub fn action(&self) -> Option<Action> {
        match self {
            Self::Do(_, _, a) => Some(*a),
            Self::Rule => None,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Do(l, _, _) => l,
            Self::Rule => "",
        }
    }

    pub fn key(&self) -> &'static str {
        match self {
            Self::Do(_, k, _) => k,
            Self::Rule => "",
        }
    }
}

/// One dropdown and the title that opens it.
pub struct Title {
    pub name: &'static str,
    /// The letter `Alt` opens it with, which is also the one drawn underlined.
    pub hotkey: char,
    pub items: Vec<Item>,
}

/// The whole bar.
///
/// Built rather than a const, because `Item` holds an `Action` and the lists
/// are long enough that a table reads better than a nest of arrays.
pub fn bar() -> Vec<Title> {
    use Action::*;
    use Item::{Do, Rule};
    vec![
        Title {
            name: "File",
            hotkey: 'F',
            items: vec![Do("Quit", "q", Quit)],
        },
        Title {
            name: "Edit",
            hotkey: 'E',
            items: vec![
                Do("Filter…", "/", BeginFilter),
                Do("Clear filter", "", ClearFilter),
            ],
        },
        Title {
            name: "View",
            hotkey: 'V',
            items: vec![
                Do("CPU", "1", SetView(crate::app::View::Cpu)),
                Do("Memory", "2", SetView(crate::app::View::Memory)),
                Do("Disk", "3", SetView(crate::app::View::Disk)),
                Rule,
                Do("Bars (block)", "", SetGlyphs(GlyphSet::Block)),
                Do("Bars (braille)", "", SetGlyphs(GlyphSet::Braille)),
                Do("Line", "", SetGlyphs(GlyphSet::Line)),
                Do("ASCII", "", SetGlyphs(GlyphSet::Ascii)),
                Rule,
                Do("Axis from zero", "", SetAxis(Axis::Zero)),
                Do("Axis fitted to data", "", SetAxis(Axis::Fit)),
                Rule,
                Do("Zoom in", "+", ZoomIn),
                Do("Zoom out", "-", ZoomOut),
                Rule,
                Do("Process detail", "d", ToggleDetail),
                Do("Tree", "t", ToggleTree),
                Do("Group", "g", NextGrouping),
                Rule,
                Do("Threads", "y", ToggleThreads),
                Do("cgroups", "C", ToggleCgroups),
                Do("Kernel threads", "K", ToggleKernel),
            ],
        },
        Title {
            name: "Go",
            hotkey: 'G',
            items: vec![
                Do("Live", "End", GotoLive),
                Do("Oldest", "Home", GotoOldest),
                Do("Pause / resume", "Space", ToggleLive),
                Rule,
                Do("Back one sample", "←", Scrub(-1)),
                Do("Forward one sample", "→", Scrub(1)),
                Rule,
                Do("Jump to a time…", "b", BeginJump),
            ],
        },
        Title {
            name: "Process",
            hotkey: 'P',
            items: vec![
                Do("Inspect…", "⏎", ToggleInspect),
                Rule,
                Do("Select previous", "↑", Select(-1)),
                Do("Select next", "↓", Select(1)),
                Rule,
                Do("Sort by next column", "s", NextSort),
                Do("Sort by what is constrained", "S", AcceptSuggestedSort),
                Rule,
                Do("Send TERM…", "x", Signal(crate::signal::Signal::Term)),
                Do("Send KILL…", "X", Signal(crate::signal::Signal::Kill)),
            ],
        },
    ]
}

/// Which menu is open and where the highlight is.
///
/// Stored rather than derived, unlike almost everything else here: a menu is a
/// mode the reader put the program into, and it has to survive the frames
/// between one keypress and the next.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct MenuState {
    /// Index of the open title, or `None` when the bar is idle.
    pub open: Option<usize>,
    /// Index of the highlighted item within the open dropdown.
    pub item: usize,
}

impl MenuState {
    pub fn is_open(self) -> bool {
        self.open.is_some()
    }

    /// Open the bar at the first title, or close it if it is already open.
    pub fn toggle(&mut self) {
        self.open = match self.open {
            Some(_) => None,
            None => Some(0),
        };
        self.item = 0;
    }

    pub fn close(&mut self) {
        self.open = None;
        self.item = 0;
    }

    /// Move to the next or previous title, wrapping.
    pub fn move_title(&mut self, delta: i32, titles: usize) {
        if titles == 0 {
            return;
        }
        let at = self.open.unwrap_or(0) as i32;
        let n = titles as i32;
        self.open = Some((at + delta).rem_euclid(n) as usize);
        self.item = 0;
    }

    /// Move the highlight, skipping rules and wrapping.
    ///
    /// Skipping is what makes a rule a rule: a separator you can land on is a
    /// blank row that eats a keypress and looks like the menu has stopped
    /// responding.
    pub fn move_item(&mut self, delta: i32, items: &[Item]) {
        let n = items.len() as i32;
        if n == 0 {
            return;
        }
        let mut at = self.item as i32;
        for _ in 0..items.len() {
            at = (at + delta).rem_euclid(n);
            if items[at as usize].action().is_some() {
                self.item = at as usize;
                return;
            }
        }
    }

    /// The title whose hotkey this letter is, if any.
    pub fn title_for(letter: char, titles: &[Title]) -> Option<usize> {
        let letter = letter.to_ascii_uppercase();
        titles.iter().position(|t| t.hotkey == letter)
    }
}

/// The action the highlighted item performs, if the menu is open on one.
pub fn chosen(state: MenuState, titles: &[Title]) -> Option<Action> {
    let t = titles.get(state.open?)?;
    t.items.get(state.item)?.action()
}

/// Width of a dropdown: the widest label plus its key, plus the frame.
pub fn width(title: &Title) -> usize {
    let widest = title
        .items
        .iter()
        .map(|i| {
            let k = i.key();
            i.label().chars().count()
                + if k.is_empty() {
                    0
                } else {
                    k.chars().count() + 3
                }
        })
        .max()
        .unwrap_or(0);
    // Two for the frame, one each side for padding, two for the check column.
    widest + 6
}

/// Where a title's dropdown starts, in columns from the left of the bar.
pub fn title_column(index: usize, titles: &[Title]) -> usize {
    // One leading space, then each name with a space either side.
    titles
        .iter()
        .take(index)
        .fold(1, |at, t| at + t.name.chars().count() + 2)
}

/// Whether this item is ticked right now.
pub fn checked(item: &Item, app: &App) -> Option<bool> {
    item.action().and_then(|a| a.checked(app))
}
