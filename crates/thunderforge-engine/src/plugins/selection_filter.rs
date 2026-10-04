//! What the Select tool is willing to select.
//!
//! # Why this exists
//!
//! A Game Master moving tokens around a finished map keeps catching walls and
//! lights instead. Spec 031 FR-008: let them narrow Select to the kinds they
//! are working with.
//!
//! # Defaults, and why they are what they are
//!
//! Every kind is selectable until someone says otherwise. A filter that
//! started restrictive would make the Select tool look broken on a first
//! visit, and "why does clicking do nothing" is a worse first impression than
//! "I caught the wrong thing".
//!
//! The filter is a working preference of the person at the keyboard, not a
//! property of the world — two Game Masters on one world must not fight over
//! it — so chrome persists it per user and hands it here. This module is only
//! the authority for what a click *does*, which is engine business because
//! selection is engine state.
//!
//! # The empty case
//!
//! Every kind disabled is legitimate, and means Select selects nothing. That
//! is indistinguishable from a broken tool unless the interface says so, which
//! is why the spec requires chrome to make the state obvious (FR-008's edge
//! case). The engine simply obeys.

use bevy::prelude::*;

/// Which kinds the Select tool acts on.
///
/// Not a `HashSet` of an enum: there are four, they are known at compile time,
/// and four bools read better at every call site than set membership.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectionFilter {
    pub tokens: bool,
    pub walls: bool,
    pub lights: bool,
    pub shapes: bool,
}

impl Default for SelectionFilter {
    fn default() -> Self {
        // Everything on. See the module note: a restrictive default reads as a
        // broken tool.
        Self {
            tokens: true,
            walls: true,
            lights: true,
            shapes: true,
        }
    }
}

impl SelectionFilter {
    /// Whether anything at all can be selected.
    ///
    /// Worth naming rather than open-coding: "the filter excludes everything"
    /// is a state chrome has to be able to explain, and a system that wants to
    /// skip work entirely can ask directly.
    pub fn selects_nothing(&self) -> bool {
        !self.tokens && !self.walls && !self.lights && !self.shapes
    }
}

/// What chrome has most recently asked for.
static REQUESTED_FILTER: std::sync::OnceLock<std::sync::Mutex<Option<SelectionFilter>>> =
    std::sync::OnceLock::new();

fn requested_filter_slot() -> &'static std::sync::Mutex<Option<SelectionFilter>> {
    REQUESTED_FILTER.get_or_init(|| std::sync::Mutex::new(None))
}

/// Set which kinds Select acts on.
///
/// Four explicit booleans rather than a list of names: the set is closed, and
/// a caller that forgets a kind should be making a visible choice about it
/// rather than silently inheriting whatever was there.
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn set_selection_filter(tokens: bool, walls: bool, lights: bool, shapes: bool) {
    if let Ok(mut slot) = requested_filter_slot().lock() {
        *slot = Some(SelectionFilter {
            tokens,
            walls,
            lights,
            shapes,
        });
    }
}

fn apply_requested_filter(mut filter: ResMut<SelectionFilter>) {
    let requested = requested_filter_slot()
        .lock()
        .ok()
        .and_then(|mut slot| slot.take());

    if let Some(next) = requested {
        *filter = next;
    }
}

pub struct SelectionFilterPlugin;

impl Plugin for SelectionFilterPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SelectionFilter>()
            .add_systems(Update, apply_requested_filter);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn everything_is_selectable_by_default() {
        let filter = SelectionFilter::default();
        assert!(filter.tokens && filter.walls && filter.lights && filter.shapes);
        assert!(!filter.selects_nothing());
    }

    #[test]
    fn excluding_every_kind_is_a_recognisable_state() {
        // Legitimate, not a bug — but chrome has to be able to say so, and it
        // can only do that if the engine can be asked.
        let filter = SelectionFilter {
            tokens: false,
            walls: false,
            lights: false,
            shapes: false,
        };
        assert!(filter.selects_nothing());
    }
}
