//! What each key does, and how to say it differently.
//!
//! Every key in the monitor names an [`Action`], and the map from keys to
//! actions is [`Keymap`]. The defaults are exactly the keys poptop has always
//! had; a config file can move any of them:
//!
//! ```conf
//! key.quit          = q, Q
//! key.filter        = /, f
//! key.kernel-threads = k
//! ```
//!
//! The boxes are not in here. Inside the filter and the jump box every key is
//! text or editing — Enter, Backspace, Esc — and a map that could rebind those
//! would be a map that could stop you typing a `q`. `Ctrl-C` is not in here
//! either: it is the reflex for leaving a full-screen program, and poptop
//! installs no SIGINT handler, so raw mode makes this the only path there is.

use crossterm::event::{KeyCode, KeyModifiers};

/// Something a key press asks for.
///
/// One per row of the key list, so anything a reader can see themselves doing
/// has a name they can bind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Quit,
    Back,
    Help,
    ScrubBack,
    ScrubForward,
    Jump,
    ZoomIn,
    ZoomOut,
    Pause,
    Oldest,
    Live,
    SelectUp,
    SelectDown,
    PageUp,
    PageDown,
    SortNext,
    SortConstraint,
    ViewNext,
    Tree,
    Group,
    Detail,
    Threads,
    Cgroups,
    KernelThreads,
    IoColumns,
    Filter,
    SignalTerm,
    SignalKill,
    /// Read the theme file again, for trying a colour without restarting.
    ReloadTheme,
}

/// One action: the name a config file uses, and the keys it has by default.
pub struct Bound {
    pub action: Action,
    pub name: &'static str,
    pub default: &'static str,
}

/// Every action, named once, with the keys poptop has always used.
///
/// The order is the key list's: `--keys` prints it in the order a reader sees
/// on screen rather than alphabetically, so the two can be read side by side.
pub const ACTIONS: &[Bound] = &[
    b(Action::Quit, "quit", "q"),
    b(Action::Back, "back", "esc"),
    b(Action::ScrubBack, "scrub-back", "left, h"),
    b(Action::ScrubForward, "scrub-forward", "right, l"),
    b(Action::Jump, "jump", "b"),
    b(Action::ZoomIn, "zoom-in", "+, ="),
    b(Action::ZoomOut, "zoom-out", "-, _"),
    b(Action::Pause, "pause", "space"),
    b(Action::Oldest, "oldest", "home"),
    b(Action::Live, "live", "end"),
    b(Action::SelectUp, "select-up", "up, k"),
    b(Action::SelectDown, "select-down", "down, j"),
    b(Action::PageUp, "page-up", "pageup"),
    b(Action::PageDown, "page-down", "pagedown"),
    b(Action::SortNext, "sort", "s"),
    b(Action::SortConstraint, "sort-constraint", "S"),
    b(Action::Filter, "filter", "/"),
    b(Action::SignalTerm, "signal-term", "x"),
    b(Action::SignalKill, "signal-kill", "X"),
    b(Action::Tree, "tree", "t"),
    b(Action::Group, "group", "g"),
    b(Action::Detail, "detail", "d"),
    b(Action::Threads, "threads", "y"),
    b(Action::ViewNext, "view", "v"),
    b(Action::Cgroups, "cgroups", "C"),
    b(Action::KernelThreads, "kernel-threads", "K"),
    b(Action::IoColumns, "io-columns", "i"),
    b(Action::ReloadTheme, "reload-theme", "R"),
    b(Action::Help, "help", "?"),
];

const fn b(action: Action, name: &'static str, default: &'static str) -> Bound {
    Bound {
        action,
        name,
        default,
    }
}

/// A key as the map holds it: the code, and the modifiers that are part of the
/// binding rather than part of the character.
///
/// Shift is not among them. On a character it is already in the character —
/// `X` is Shift and `x` is not — and on an arrow it means "ten at a time",
/// which the action decides rather than the map.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Key {
    pub code: KeyCode,
    pub mods: KeyModifiers,
}

/// Which keys ask for which actions.
#[derive(Clone, Debug, PartialEq)]
pub struct Keymap {
    binds: Vec<(Key, Action)>,
}

impl Default for Keymap {
    fn default() -> Self {
        let mut map = Keymap { binds: Vec::new() };
        for bound in ACTIONS {
            map.bind(bound.action, bound.default)
                .expect("a default binding poptop cannot parse");
        }
        map
    }
}

/// Why a binding was not taken.
#[derive(Debug, PartialEq)]
pub enum Refused {
    /// A key name nothing could be made of.
    NoSuchKey(String),
    /// Already another action's, which keeps it. Named so the message can say
    /// which one, since the reader has to choose between them.
    Taken { key: String, by: &'static str },
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refused::NoSuchKey(k) => write!(
                f,
                "`{k}` is not a key: a character, or one of {}",
                NAMED.iter().map(|(n, _)| *n).collect::<Vec<_>>().join(", ")
            ),
            Refused::Taken { key, by } => {
                write!(f, "`{key}` is already `{by}`, which keeps it")
            }
        }
    }
}

impl Keymap {
    /// Bind `keys` — a comma-separated list — to `action`, replacing whatever
    /// that action had.
    ///
    /// A key another action holds is refused and left where it was: two
    /// actions on one key is a map where one of them can never be reached, and
    /// guessing which the reader meant is worse than saying so.
    pub fn bind(&mut self, action: Action, keys: &str) -> Result<(), Refused> {
        let mut parsed = Vec::new();
        for name in keys.split(',').map(str::trim).filter(|k| !k.is_empty()) {
            let key = parse_key(name).ok_or_else(|| Refused::NoSuchKey(name.to_string()))?;
            if let Some((_, other)) = self.binds.iter().find(|(k, a)| *k == key && *a != action) {
                return Err(Refused::Taken {
                    key: name.to_string(),
                    by: name_of(*other),
                });
            }
            parsed.push((key, action));
        }
        self.binds.retain(|(_, a)| *a != action);
        self.binds.extend(parsed);
        Ok(())
    }

    /// What this key press asks for, if anything.
    ///
    /// Shift is ignored on everything but a character, where it is already
    /// part of the character: `X` and `x` are different keys and stay so.
    pub fn action(&self, code: KeyCode, mods: KeyModifiers) -> Option<Action> {
        // Shift never distinguishes a binding: on a character it is already
        // in the character, and on an arrow it is the fast scrub.
        let wanted = mods.difference(KeyModifiers::SHIFT);
        self.binds
            .iter()
            .find(|(k, _)| k.code == code && k.mods == wanted)
            .map(|(_, a)| *a)
    }

    /// The keys bound to an action, as a config file would write them.
    pub fn keys(&self, action: Action) -> String {
        self.binds
            .iter()
            .filter(|(_, a)| *a == action)
            .map(|(k, _)| show_key(*k))
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Whether this is the map poptop ships with, for saying so on screen.
    pub fn is_default(&self) -> bool {
        *self == Keymap::default()
    }
}

/// The action a config file's `key.<name>` means.
pub fn action_named(name: &str) -> Option<Action> {
    ACTIONS.iter().find(|b| b.name == name).map(|b| b.action)
}

/// The config name of an action, for a message about it.
pub fn name_of(action: Action) -> &'static str {
    ACTIONS
        .iter()
        .find(|b| b.action == action)
        .map_or("?", |b| b.name)
}

/// The keys with names rather than characters.
const NAMED: &[(&str, KeyCode)] = &[
    ("esc", KeyCode::Esc),
    ("enter", KeyCode::Enter),
    ("space", KeyCode::Char(' ')),
    ("tab", KeyCode::Tab),
    ("backspace", KeyCode::Backspace),
    ("delete", KeyCode::Delete),
    ("insert", KeyCode::Insert),
    ("left", KeyCode::Left),
    ("right", KeyCode::Right),
    ("up", KeyCode::Up),
    ("down", KeyCode::Down),
    ("home", KeyCode::Home),
    ("end", KeyCode::End),
    ("pageup", KeyCode::PageUp),
    ("pagedown", KeyCode::PageDown),
];

/// A key from the way a config file writes it: `q`, `X`, `/`, `esc`, `ctrl-r`.
///
/// Case is the character's own: `X` is Shift-x on every keyboard poptop runs
/// on, and treating them as one key would take one of the two signal keys
/// away. A named key is lower case, because `Esc` is not a character anyone
/// types.
pub fn parse_key(text: &str) -> Option<Key> {
    let text = text.trim();
    let (mods, rest) = match text.split_once('-') {
        // `-` is itself a key, so a one-character prefix is a character.
        Some((prefix, rest)) if !rest.is_empty() && prefix.len() > 1 => {
            let m = match prefix.to_ascii_lowercase().as_str() {
                "ctrl" => KeyModifiers::CONTROL,
                "alt" => KeyModifiers::ALT,
                _ => return None,
            };
            (m, rest)
        }
        _ => (KeyModifiers::NONE, text),
    };
    let lower = rest.to_ascii_lowercase();
    if let Some((_, code)) = NAMED.iter().find(|(n, _)| *n == lower) {
        return Some(Key { code: *code, mods });
    }
    let mut chars = rest.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) => Some(Key {
            code: KeyCode::Char(c),
            mods,
        }),
        _ => None,
    }
}

/// The inverse of [`parse_key`], for `--keys` and for a message.
pub fn show_key(key: Key) -> String {
    let base = match key.code {
        KeyCode::Char(' ') => "space".to_string(),
        KeyCode::Char(c) => c.to_string(),
        code => NAMED.iter().find(|(_, k)| *k == code).map_or_else(
            || format!("{code:?}").to_lowercase(),
            |(n, _)| n.to_string(),
        ),
    };
    let mut out = String::new();
    if key.mods.contains(KeyModifiers::CONTROL) {
        out.push_str("ctrl-");
    }
    if key.mods.contains(KeyModifiers::ALT) {
        out.push_str("alt-");
    }
    out + &base
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_map_is_the_keys_poptop_has_always_had() {
        let map = Keymap::default();
        assert_eq!(
            map.action(KeyCode::Char('q'), KeyModifiers::NONE),
            Some(Action::Quit)
        );
        assert_eq!(
            map.action(KeyCode::Esc, KeyModifiers::NONE),
            Some(Action::Back)
        );
        assert_eq!(
            map.action(KeyCode::Char('h'), KeyModifiers::NONE),
            Some(Action::ScrubBack)
        );
        assert_eq!(
            map.action(KeyCode::Left, KeyModifiers::NONE),
            Some(Action::ScrubBack)
        );
        // Shift is the fast scrub, not another binding.
        assert_eq!(
            map.action(KeyCode::Left, KeyModifiers::SHIFT),
            Some(Action::ScrubBack)
        );
        // A capital is its own key: the two signal keys depend on it.
        assert_eq!(
            map.action(KeyCode::Char('x'), KeyModifiers::NONE),
            Some(Action::SignalTerm)
        );
        assert_eq!(
            map.action(KeyCode::Char('X'), KeyModifiers::SHIFT),
            Some(Action::SignalKill)
        );
        assert_eq!(map.action(KeyCode::Char('z'), KeyModifiers::NONE), None);
        assert!(map.is_default());
    }

    #[test]
    fn an_action_can_be_moved_and_given_several_keys() {
        let mut map = Keymap::default();
        map.bind(Action::Quit, "Q, ctrl-q").unwrap();
        assert_eq!(
            map.action(KeyCode::Char('Q'), KeyModifiers::SHIFT),
            Some(Action::Quit)
        );
        assert_eq!(
            map.action(KeyCode::Char('q'), KeyModifiers::CONTROL),
            Some(Action::Quit)
        );
        // The key it used to have is free, not still quitting.
        assert_eq!(map.action(KeyCode::Char('q'), KeyModifiers::NONE), None);
        assert_eq!(map.keys(Action::Quit), "Q, ctrl-q");
        assert!(!map.is_default());
    }

    #[test]
    fn a_key_another_action_holds_is_refused_naming_it() {
        let mut map = Keymap::default();
        let refused = map.bind(Action::Filter, "t").unwrap_err();
        assert_eq!(
            refused,
            Refused::Taken {
                key: "t".to_string(),
                by: "tree"
            }
        );
        // And nothing moved: the whole binding is refused, not half of it.
        assert_eq!(
            map.action(KeyCode::Char('t'), KeyModifiers::NONE),
            Some(Action::Tree)
        );
        assert_eq!(
            map.action(KeyCode::Char('/'), KeyModifiers::NONE),
            Some(Action::Filter)
        );
        assert!(refused.to_string().contains("already `tree`"), "{refused}");
    }

    #[test]
    fn a_key_that_is_not_one_says_what_a_key_looks_like() {
        let mut map = Keymap::default();
        let refused = map.bind(Action::Quit, "escape").unwrap_err();
        assert_eq!(refused, Refused::NoSuchKey("escape".to_string()));
        assert!(refused.to_string().contains("esc"), "{refused}");
        assert_eq!(map.keys(Action::Quit), "q", "the old binding was lost");
    }

    #[test]
    fn every_key_writes_back_the_way_it_was_read() {
        for text in [
            "q", "X", "/", "-", "esc", "space", "left", "pagedown", "ctrl-r", "alt-x",
        ] {
            let key = parse_key(text).unwrap_or_else(|| panic!("`{text}` did not parse"));
            assert_eq!(show_key(key), text, "`{text}` came back differently");
        }
        // Every action's default is a list this parses and writes back.
        let map = Keymap::default();
        for bound in ACTIONS {
            let want: Vec<String> = bound
                .default
                .split(',')
                .map(|k| k.trim().to_string())
                .collect();
            assert_eq!(map.keys(bound.action), want.join(", "), "{}", bound.name);
        }
    }

    #[test]
    fn every_action_has_a_name_and_a_key() {
        let mut names: Vec<&str> = ACTIONS.iter().map(|b| b.name).collect();
        names.sort();
        let before = names.len();
        names.dedup();
        assert_eq!(names.len(), before, "two actions share a name");
        for bound in ACTIONS {
            assert_eq!(action_named(bound.name), Some(bound.action));
            assert!(
                !Keymap::default().keys(bound.action).is_empty(),
                "{}",
                bound.name
            );
        }
    }
}
