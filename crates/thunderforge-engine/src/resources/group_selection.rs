//! Several things selected at once (spec 085).
//!
//! The single selections — `SelectedToken`, `SelectedWall`, `SelectedLight`,
//! `SelectedShape` — stay what every existing tool reads. A group is the
//! larger set a box or a shift-click makes, and writing one also writes the
//! singles: every token of the group into `SelectedToken`, and each other
//! kind's first id as that kind's primary. So a tool that knows nothing of
//! groups still shows, and acts on, a sensible one.

use bevy::prelude::*;

use super::{SelectedLight, SelectedShape, SelectedToken, SelectedWall};

/// Which kind of thing an id names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupKind {
    Token,
    Wall,
    Light,
    Shape,
}

/// The ids selected together, per kind, in the order they were taken.
#[derive(Resource, Debug, Clone, Default, PartialEq, Eq)]
pub struct GroupSelection {
    pub tokens: Vec<String>,
    pub walls: Vec<String>,
    pub lights: Vec<String>,
    pub shapes: Vec<String>,
}

/// The single selections a group writes through to.
pub struct Singles<'a> {
    pub token: &'a mut SelectedToken,
    pub wall: &'a mut SelectedWall,
    pub light: &'a mut SelectedLight,
    pub shape: &'a mut SelectedShape,
}

impl Singles<'_> {
    /// The group the singles describe on their own: every selected token,
    /// and each kind's primary.
    pub fn as_group(&self) -> GroupSelection {
        GroupSelection {
            tokens: self.token.selected_ids().to_vec(),
            walls: self.wall.0.iter().cloned().collect(),
            lights: self.light.0.iter().cloned().collect(),
            shapes: self.shape.0.iter().cloned().collect(),
        }
    }
}

impl GroupSelection {
    /// Replace the group, and write it through to the singles.
    pub fn set_group(&mut self, group: GroupSelection, singles: Singles<'_>) {
        *self = group;
        self.write_singles(singles);
    }

    /// Select exactly one item, of `kind`.
    pub fn select_one(&mut self, kind: GroupKind, id: String, singles: Singles<'_>) {
        let mut group = GroupSelection::default();
        group.list_mut(kind).push(id);
        self.set_group(group, singles);
    }

    /// Select nothing.
    pub fn clear(&mut self, singles: Singles<'_>) {
        self.set_group(GroupSelection::default(), singles);
    }

    /// Drop `id` from whichever kind holds it. The singles are not touched:
    /// the caller removing a deleted item has already let go of it there.
    pub fn remove(&mut self, id: &str) {
        for list in [
            &mut self.tokens,
            &mut self.walls,
            &mut self.lights,
            &mut self.shapes,
        ] {
            list.retain(|held| held != id);
        }
    }

    /// How many items the group holds, across kinds.
    pub fn len(&self) -> usize {
        self.tokens.len() + self.walls.len() + self.lights.len() + self.shapes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Whether `id` is a member, of any kind.
    pub fn contains(&self, id: &str) -> bool {
        self.tokens
            .iter()
            .chain(&self.walls)
            .chain(&self.lights)
            .chain(&self.shapes)
            .any(|held| held == id)
    }

    /// The ids of one kind.
    pub fn list(&self, kind: GroupKind) -> &[String] {
        match kind {
            GroupKind::Token => &self.tokens,
            GroupKind::Wall => &self.walls,
            GroupKind::Light => &self.lights,
            GroupKind::Shape => &self.shapes,
        }
    }

    fn list_mut(&mut self, kind: GroupKind) -> &mut Vec<String> {
        match kind {
            GroupKind::Token => &mut self.tokens,
            GroupKind::Wall => &mut self.walls,
            GroupKind::Light => &mut self.lights,
            GroupKind::Shape => &mut self.shapes,
        }
    }

    /// Whether the singles still say what this group wrote to them. A tool
    /// that selects one thing on its own — a wall clicked in Walls — changes a
    /// single, and the group then no longer describes the board.
    pub fn agrees_with(&self, singles: &Singles<'_>) -> bool {
        self.tokens == singles.token.selected_ids()
            && self.walls.first() == singles.wall.0.as_ref()
            && self.lights.first() == singles.light.0.as_ref()
            && self.shapes.first() == singles.shape.0.as_ref()
    }

    fn write_singles(&self, singles: Singles<'_>) {
        singles.token.select_stack(self.tokens.clone());
        singles.wall.0 = self.walls.first().cloned();
        singles.light.0 = self.lights.first().cloned();
        singles.shape.0 = self.shapes.first().cloned();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Fixture {
        token: SelectedToken,
        wall: SelectedWall,
        light: SelectedLight,
        shape: SelectedShape,
    }

    impl Fixture {
        fn singles(&mut self) -> Singles<'_> {
            Singles {
                token: &mut self.token,
                wall: &mut self.wall,
                light: &mut self.light,
                shape: &mut self.shape,
            }
        }
    }

    fn ids(list: &[&str]) -> Vec<String> {
        list.iter().map(|id| (*id).to_string()).collect()
    }

    fn mixed() -> GroupSelection {
        GroupSelection {
            tokens: ids(&["t1", "t2"]),
            walls: ids(&["w1", "w2"]),
            lights: ids(&["l1"]),
            shapes: ids(&["s1", "s2", "s3"]),
        }
    }

    #[test]
    fn set_group_writes_the_tokens_and_each_kinds_primary() {
        let mut fixture = Fixture::default();
        let mut group = GroupSelection::default();
        group.set_group(mixed(), fixture.singles());

        assert_eq!(fixture.token.selected_ids(), ids(&["t1", "t2"]));
        assert_eq!(fixture.wall.0.as_deref(), Some("w1"));
        assert_eq!(fixture.light.0.as_deref(), Some("l1"));
        assert_eq!(fixture.shape.0.as_deref(), Some("s1"));
        assert!(group.agrees_with(&fixture.singles()));
    }

    #[test]
    fn select_one_resets_the_group_to_that_item() {
        let mut fixture = Fixture::default();
        let mut group = GroupSelection::default();
        group.set_group(mixed(), fixture.singles());
        group.select_one(GroupKind::Wall, "w9".into(), fixture.singles());

        assert_eq!(group.len(), 1);
        assert_eq!(group.walls, ids(&["w9"]));
        assert!(fixture.token.selected_ids().is_empty());
        assert_eq!(fixture.wall.0.as_deref(), Some("w9"));
        assert_eq!(fixture.light.0, None);
        assert_eq!(fixture.shape.0, None);
    }

    #[test]
    fn clear_resets_the_group_and_the_singles_to_none() {
        let mut fixture = Fixture::default();
        let mut group = GroupSelection::default();
        group.set_group(mixed(), fixture.singles());
        group.clear(fixture.singles());

        assert!(group.is_empty());
        assert!(fixture.token.selected_ids().is_empty());
        assert_eq!(fixture.wall.0, None);
    }

    #[test]
    fn remove_drops_an_id_from_whichever_kind_holds_it() {
        let mut group = mixed();
        group.remove("w1");
        group.remove("s2");
        group.remove("absent");

        assert_eq!(group.walls, ids(&["w2"]));
        assert_eq!(group.shapes, ids(&["s1", "s3"]));
        assert_eq!(group.tokens, ids(&["t1", "t2"]));
    }

    #[test]
    fn len_counts_across_kinds() {
        assert_eq!(mixed().len(), 8);
        assert_eq!(GroupSelection::default().len(), 0);
    }

    #[test]
    fn a_wall_selected_by_its_own_tool_no_longer_agrees() {
        let mut fixture = Fixture::default();
        let mut group = GroupSelection::default();
        group.set_group(mixed(), fixture.singles());
        fixture.wall.select("w7".into());

        assert!(!group.agrees_with(&fixture.singles()));
        assert_eq!(fixture.singles().as_group().walls, ids(&["w7"]));
    }
}
