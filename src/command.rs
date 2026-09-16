//! Every command poptop can perform, named once.
//!
//! The keyboard and the menu are two surfaces over the same list. Without a
//! list they would be two implementations: a menu item that toggled a field the
//! key handler also toggled would drift the first time either grew a side
//! effect, and the drift would be invisible until somebody used the other one.
//!
//! So a key maps to an [`Action`] and a menu item holds an [`Action`], and
//! [`Action::apply`] is the only place that changes anything.

use crate::app::{self, App, Grouping};
use crate::glyphs::{Axis, GlyphSet};
use crate::signal::Signal;

/// One thing poptop can be asked to do.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    Quit,

    // Time.
    Scrub(i32),
    ToggleLive,
    GotoOldest,
    GotoLive,
    ZoomIn,
    ZoomOut,
    BeginJump,

    // Selection.
    Select(i32),
    /// The row at this index of the visible table. The mouse points at a place
    /// rather than a direction, and there is no key that means "the ninth row".
    SelectRow(usize),
    /// The sample at this index of the buffer, likewise.
    ScrubTo(usize),

    // The table.
    NextSort,
    AcceptSuggestedSort,
    NextView,
    PrevView,
    /// A tab by name, for the strip and the menu — a pointer names a place.
    SetView(crate::app::View),
    NextGrouping,
    ToggleTree,
    ToggleDetail,
    /// Open or close the inspector on the selected process.
    ToggleInspect,
    ToggleThreads,
    ToggleCgroups,
    ToggleKernel,
    BeginFilter,
    ClearFilter,

    // Drawing.
    SetGlyphs(GlyphSet),
    SetAxis(Axis),

    // The process under the cursor.
    Signal(Signal),
}

impl Action {
    /// Perform it. The only place any of these happen.
    pub fn apply(self, app: &mut App) {
        match self {
            Self::Quit => app.should_quit = true,

            Self::Scrub(n) => app.history.scrub(n as isize),
            Self::ToggleLive => {
                // Pause pins the cursor where it is; resume returns to the live
                // edge.
                if app.history.is_live() {
                    app.history.scrub(-1);
                } else {
                    app.history.goto_live();
                }
            }
            Self::GotoOldest => app.history.goto_oldest(),
            Self::GotoLive => app.history.goto_live(),
            Self::ZoomIn => app.zoom_in(),
            Self::ZoomOut => app.zoom_out(),
            Self::BeginJump => {
                app.editing_jump = true;
                app.jump.clear();
                app.jump_note = None;
            }

            Self::Select(n) => app.select_delta(n as isize),
            Self::SelectRow(i) => app.select_row(i),
            Self::ScrubTo(i) => {
                // Relative, because that is the only way the cursor moves — and
                // it is where the "past the newest means live" rule lives.
                let from = app.history.cursor_index() as isize;
                app.history.scrub(i as isize - from);
            }

            Self::NextSort => app.sort = app.sort.next(app.io_collected(), app.view),
            Self::AcceptSuggestedSort => {
                if let Some(c) = app.constraint() {
                    app.sort = c.sort();
                    // Sorting by a column that is not on screen answers the
                    // question invisibly: the rows move and nothing says why.
                    if c.sort() == app::Sort::Disk {
                        app.show_io = true;
                    }
                }
            }
            Self::PrevView | Self::SetView(_) => {
                app.view = match self {
                    Self::PrevView => app.view.prev(),
                    Self::SetView(v) => v,
                    _ => unreachable!("guarded by the arm"),
                };
                app.insist_for_view();
                if !app.view.sorts().contains(&app.sort) {
                    app.sort = app.view.default_sort_for(app.io_collected());
                }
            }
            Self::NextView => {
                app.view = app.view.next();
                app.insist_for_view();
                // A sort the new view cannot show would be an ordering with no
                // visible reason for it.
                if !app.view.sorts().contains(&app.sort) {
                    app.sort = app.view.default_sort_for(app.io_collected());
                }
            }
            Self::NextGrouping => {
                app.group = app.group.next();
                if app.group != Grouping::Off {
                    app.tree = false;
                }
            }
            Self::ToggleTree => {
                app.tree = !app.tree;
                // Grouping destroys parentage by construction, so a grouped
                // tree would be a tree of things that are not processes.
                if app.tree {
                    app.group = Grouping::Off;
                }
            }
            Self::ToggleDetail => app.detail = !app.detail,
            Self::ToggleInspect => app.inspecting = !app.inspecting,
            Self::ToggleThreads => app.toggle_threads(),
            Self::ToggleCgroups => app.toggle_cgroups(),
            Self::ToggleKernel => app.show_kernel = !app.show_kernel,
            Self::BeginFilter => {
                app.editing_filter = true;
                app.filter.clear();
            }
            Self::ClearFilter => {
                app.filter.clear();
                app.editing_filter = false;
            }

            Self::SetGlyphs(g) => app.glyphs = g,
            Self::SetAxis(a) => app.axis = a,

            Self::Signal(s) => app.ask_to_signal(s),
        }
    }

    /// Whether this action is currently *on*, for the menu's tick marks.
    ///
    /// `None` for an action that does something rather than being in a state.
    /// A menu that ticked `Zoom in` would be claiming it is a mode.
    pub fn checked(self, app: &App) -> Option<bool> {
        Some(match self {
            Self::ToggleTree => app.tree,
            Self::ToggleDetail => app.detail,
            Self::ToggleInspect => app.inspecting,
            Self::ToggleThreads => app.show_threads,
            Self::ToggleCgroups => app.show_cgroups,
            Self::ToggleKernel => app.show_kernel,
            Self::SetGlyphs(g) => app.glyphs == g,
            Self::SetAxis(a) => app.axis == a,
            Self::GotoLive => app.history.is_live(),
            Self::SetView(v) => app.view == v,
            _ => return None,
        })
    }
}
