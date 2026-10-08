use super::*;
use crate::resources::ShapeKind;

fn shape(created_by: Option<&str>) -> Shape {
    Shape {
        id: "s1".into(),
        kind: ShapeKind::Rect,
        geometry: json!({ "x": 0.0, "y": 0.0, "w": 10.0, "h": 10.0 }),
        text: None,
        style: None,
        visible_to_players: true,
        created_by: created_by.map(str::to_string),
    }
}

fn only(kind: GroupKind, on: bool) -> SelectionFilter {
    let mut filter = SelectionFilter::default();
    match kind {
        GroupKind::Token => filter.tokens = on,
        GroupKind::Wall => filter.walls = on,
        GroupKind::Light => filter.lights = on,
        GroupKind::Shape => filter.shapes = on,
    }
    filter
}

fn gm_takes(kind: GroupKind, filter: &SelectionFilter) -> bool {
    let drawn = shape(Some("someone"));
    let item = match kind {
        GroupKind::Token => BoxItem::Token {
            owner: Some("someone"),
            seen: false,
        },
        GroupKind::Wall => BoxItem::Wall,
        GroupKind::Light => BoxItem::Light,
        GroupKind::Shape => BoxItem::Shape(&drawn),
    };
    box_candidate(true, Some("gm"), filter, item)
}

#[test]
fn a_gm_box_takes_each_kind_the_filter_allows_and_no_other() {
    for kind in [
        GroupKind::Token,
        GroupKind::Wall,
        GroupKind::Light,
        GroupKind::Shape,
    ] {
        assert!(gm_takes(kind, &only(kind, true)), "{kind:?} on");
        assert!(!gm_takes(kind, &only(kind, false)), "{kind:?} off");
    }
}

#[test]
fn a_gm_box_ignores_ownership_and_sight() {
    let filter = SelectionFilter::default();
    assert!(box_candidate(
        true,
        None,
        &filter,
        BoxItem::Token {
            owner: None,
            seen: false
        }
    ));
}

#[test]
fn a_press_on_empty_board_starts_a_box_with_or_without_shift() {
    assert_eq!(press_outcome(false, None, None), Press::Box);
    assert_eq!(press_outcome(true, None, None), Press::Box);
}

#[test]
fn shift_on_an_item_toggles_it_and_a_member_wins_over_the_token_under_it() {
    assert_eq!(
        press_outcome(true, None, Some("t1".into())),
        Press::Toggle(GroupKind::Token, "t1".into())
    );
    assert_eq!(
        press_outcome(
            true,
            Some((GroupKind::Wall, "w1".into())),
            Some("t1".into())
        ),
        Press::Toggle(GroupKind::Wall, "w1".into())
    );
}

#[test]
fn a_plain_press_on_a_member_is_left_to_the_move_and_elsewhere_to_the_token_drag() {
    assert_eq!(
        press_outcome(
            false,
            Some((GroupKind::Token, "t1".into())),
            Some("t1".into())
        ),
        Press::Member
    );
    assert_eq!(press_outcome(false, None, Some("t9".into())), Press::Other);
}

#[test]
fn toggle_one_takes_an_item_out_and_puts_it_back_at_the_end() {
    let group = GroupSelection {
        tokens: vec!["a".into(), "b".into()],
        walls: vec!["w".into()],
        ..Default::default()
    };
    let out = toggle_one(&group, GroupKind::Token, "a".into());
    assert_eq!(out.tokens, vec!["b".to_string()]);
    assert_eq!(out.walls, vec!["w".to_string()]);
    let back = toggle_one(&out, GroupKind::Token, "a".into());
    assert_eq!(back.tokens, vec!["b".to_string(), "a".to_string()]);
}

#[test]
fn a_box_without_shift_replaces_and_with_shift_toggles_per_kind() {
    let current = GroupSelection {
        tokens: vec!["a".into()],
        walls: vec!["w1".into()],
        ..Default::default()
    };
    let hits = GroupSelection {
        tokens: vec!["a".into(), "b".into()],
        ..Default::default()
    };
    assert_eq!(fold_box(&current, &hits, false), hits);
    let toggled = fold_box(&current, &hits, true);
    assert_eq!(toggled.tokens, vec!["b".to_string()]);
    assert_eq!(toggled.walls, vec!["w1".to_string()]);
}
