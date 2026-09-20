//! The documentation, held against the code it documents.
//!
//! Two pages under `docs/reference/` are lists that already exist in the
//! source — the keys in [`crate::ui::HELP`], the settings in
//! [`crate::config::KEYS`]. A reference page is worth having only if it is
//! right, and a page nobody can check goes wrong within a release: a key is
//! added, the overlay and `--help` are updated because a test holds those
//! three to each other, and the page under `docs/` is not, because nothing
//! was watching it.
//!
//! These read the shipped markdown at compile time and compare the tables in
//! it to the tables in the source. They check the *rows*, not the prose: the
//! prose is the part a person writes and a person reviews.

/// The first cell of every row of the first markdown table in `page`.
///
/// A reference page is tables and prose, and only the tables are mechanical.
/// The first table is the list; anything after it is commentary with tables of
/// its own, which are not this list.
fn first_table_keys(page: &str) -> Vec<String> {
    let mut rows = Vec::new();
    let mut seen_header = false;
    for line in page.lines() {
        let line = line.trim();
        if !line.starts_with('|') {
            // A blank line inside a table is impossible; one after it ends it.
            if seen_header && !rows.is_empty() {
                break;
            }
            continue;
        }
        let first = line
            .trim_matches('|')
            .split('|')
            .next()
            .unwrap_or("")
            .trim()
            .to_string();
        // The `|---|---|` rule under the header, and the header itself.
        if first.chars().all(|c| c == '-' || c == ':') && !first.is_empty() {
            seen_header = true;
            continue;
        }
        if !seen_header {
            continue;
        }
        rows.push(first);
    }
    rows
}

/// `` `q` `` → `q`, and `` `←/→` `` → `←/→`. The page writes keys as code.
fn unbacktick(s: &str) -> String {
    s.replace('`', "").trim().to_string()
}

#[test]
fn the_key_reference_lists_exactly_the_keys_poptop_takes() {
    const PAGE: &str = include_str!("../docs/reference/keys.md");
    let documented: Vec<String> = first_table_keys(PAGE)
        .iter()
        .map(|k| unbacktick(k))
        .collect();
    let real: Vec<String> = crate::ui::HELP
        .iter()
        .map(|(key, _, _)| (*key).to_string())
        .collect();

    assert!(
        !documented.is_empty(),
        "no table found in docs/reference/keys.md — did its shape change?"
    );
    // Order too: the page is read top to bottom beside the `?` overlay, and a
    // list in a different order is one a reader cannot check against the
    // screen.
    assert_eq!(
        documented, real,
        "docs/reference/keys.md and ui::HELP disagree.\n\
         documented: {documented:?}\nin HELP:    {real:?}"
    );
}

#[test]
fn the_key_reference_lists_every_action_and_its_default_keys() {
    // The second table on the page: the names a config file binds. A new
    // action that nobody can look up is one nobody can rebind.
    const PAGE: &str = include_str!("../docs/reference/keys.md");
    let table = section(PAGE, "## Moving them");
    let documented = first_table_keys(table);
    let map = crate::keys::Keymap::default();
    for bound in crate::keys::ACTIONS {
        let row = table
            .lines()
            .find(|l| {
                l.starts_with('|')
                    && unbacktick(l.trim_matches('|').split('|').next().unwrap_or("")) == bound.name
            })
            .unwrap_or_else(|| panic!("no row for `{}` in docs/reference/keys.md", bound.name));
        for key in map.keys(bound.action).split(", ") {
            assert!(
                row.contains(&format!("`{key}`")),
                "the row for `{}` does not give `{key}`:\n  {row}",
                bound.name
            );
        }
    }
    assert_eq!(
        documented.len(),
        crate::keys::ACTIONS.len(),
        "docs/reference/keys.md lists {} actions and poptop has {}",
        documented.len(),
        crate::keys::ACTIONS.len()
    );
}

#[test]
fn the_key_reference_says_what_each_key_does() {
    const PAGE: &str = include_str!("../docs/reference/keys.md");
    // Not word for word — the page has room to say more than a footer does.
    // But a row whose description shares nothing with the one on screen is a
    // row that was written about a different key.
    for (key, _, what) in crate::ui::HELP {
        let row = PAGE
            .lines()
            .find(|l| {
                l.starts_with('|')
                    && unbacktick(l.trim_matches('|').split('|').next().unwrap_or("")) == *key
            })
            .unwrap_or_else(|| panic!("no row for `{key}` in docs/reference/keys.md"));
        // The longest word of the on-screen description, which is the one
        // least likely to be shared by accident.
        let anchor = what
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| w.len() > 4)
            .max_by_key(|w| w.len());
        if let Some(anchor) = anchor {
            assert!(
                row.to_lowercase().contains(&anchor.to_lowercase()),
                "the row for `{key}` does not mention `{anchor}`:\n  {row}\n  on screen: {what}"
            );
        }
    }
}

/// One `##` section of a page, without the sections that follow it.
///
/// A page has several tables and only one of them is the list being checked.
fn section<'a>(page: &'a str, heading: &str) -> &'a str {
    let rest = page
        .split_once(heading)
        .unwrap_or_else(|| panic!("no `{heading}` in the page"))
        .1;
    match rest.find("\n## ") {
        Some(end) => &rest[..end],
        None => rest,
    }
}

#[test]
fn the_configuration_reference_lists_every_setting() {
    const PAGE: &str = include_str!("../docs/reference/configuration.md");
    // The settings table is not the first table on the page — the file format
    // is described first — so find it by its header rather than by position.
    let table = section(PAGE, "## Every setting");
    let documented: Vec<String> = first_table_keys(table)
        .iter()
        // `theme` is written as `` `theme` ``; a few rows name the flag too.
        .map(|k| unbacktick(k.split('/').next().unwrap_or(k)))
        .collect();
    let mut real: Vec<String> = crate::config::KEYS
        .iter()
        .map(|s| s.name.to_string())
        .collect();

    for key in &real {
        assert!(
            documented.iter().any(|d| d == key),
            "`{key}` is a setting and is not in docs/reference/configuration.md"
        );
    }
    for key in &documented {
        assert!(
            real.iter().any(|r| r == key),
            "docs/reference/configuration.md documents `{key}`, which is not a setting"
        );
    }
    real.sort();
    let mut documented = documented;
    documented.sort();
    documented.dedup();
    assert_eq!(
        documented.len(),
        real.len(),
        "docs/reference/configuration.md lists a setting twice"
    );
}

#[test]
fn the_configuration_reference_states_a_default_for_each() {
    const PAGE: &str = include_str!("../docs/reference/configuration.md");
    let table = section(PAGE, "## Every setting");
    for line in table.lines().filter(|l| l.trim().starts_with("| `")) {
        let cells: Vec<&str> = line.trim().trim_matches('|').split('|').collect();
        // key | values | default | what it does
        assert!(
            cells.len() >= 4,
            "a settings row is missing a column:\n  {line}"
        );
        let key = unbacktick(cells[0]);
        assert!(
            !unbacktick(cells[2]).is_empty(),
            "`{key}` is documented with no default"
        );
    }
}
