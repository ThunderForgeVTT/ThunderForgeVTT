use super::*;

fn ids() -> (Uuid, Uuid) {
    (Uuid::new_v4(), Uuid::new_v4())
}

fn player(user_id: Uuid) -> Viewer {
    Viewer {
        user_id,
        is_gm: false,
        is_admin: false,
    }
}

fn gm(user_id: Uuid) -> Viewer {
    Viewer {
        user_id,
        is_gm: true,
        is_admin: false,
    }
}

fn admin(user_id: Uuid) -> Viewer {
    Viewer {
        user_id,
        is_gm: false,
        is_admin: true,
    }
}

fn roll(roller: Uuid, visibility: Visibility, revealed: bool) -> RollFacts {
    RollFacts {
        roller,
        visibility,
        revealed,
    }
}

#[test]
fn who_may_roll_what() {
    assert!(may_roll(Visibility::Everyone, false).is_ok());
    assert!(may_roll(Visibility::Everyone, true).is_ok());
    assert!(may_roll(Visibility::GmEyes, false).is_ok());
    assert!(may_roll(Visibility::GmEyes, true).is_err());
    assert!(may_roll(Visibility::GmOnly, true).is_ok());
    assert!(may_roll(Visibility::GmOnly, false).is_err());
}

#[test]
fn everyone_sees_an_everyone_roll() {
    let (roller, other) = ids();
    let r = roll(roller, Visibility::Everyone, false);
    for viewer in [player(roller), player(other), gm(other), admin(other)] {
        assert_eq!(view_of(r, viewer), RollView::Whole);
    }
}

#[test]
fn a_gm_eyes_roll_is_masked_for_the_other_players_only() {
    let (roller, other) = ids();
    let r = roll(roller, Visibility::GmEyes, false);
    assert_eq!(view_of(r, player(roller)), RollView::Whole);
    assert_eq!(view_of(r, gm(other)), RollView::Whole);
    assert_eq!(view_of(r, admin(other)), RollView::Whole);
    assert_eq!(view_of(r, player(other)), RollView::Masked);
}

#[test]
fn a_gm_only_roll_is_hidden_from_the_players() {
    let (roller, other) = ids();
    let r = roll(roller, Visibility::GmOnly, false);
    assert_eq!(view_of(r, gm(roller)), RollView::Whole);
    assert_eq!(view_of(r, admin(other)), RollView::Whole);
    assert_eq!(view_of(r, player(other)), RollView::Hidden);
}

#[test]
fn a_revealed_roll_is_whole_for_everyone() {
    let (roller, other) = ids();
    for visibility in [Visibility::GmEyes, Visibility::GmOnly] {
        assert_eq!(
            view_of(roll(roller, visibility, true), player(other)),
            RollView::Whole
        );
    }
}

#[test]
fn only_a_gm_only_event_is_withheld_and_only_from_players() {
    assert!(event_reaches(Visibility::Everyone, false));
    assert!(event_reaches(Visibility::GmEyes, false));
    assert!(!event_reaches(Visibility::GmOnly, false));
    assert!(event_reaches(Visibility::GmOnly, true));
}

#[test]
fn an_unknown_stored_value_reads_as_the_most_hidden() {
    assert_eq!(Visibility::parse("everyone"), Visibility::Everyone);
    assert_eq!(Visibility::parse("gm_eyes"), Visibility::GmEyes);
    assert_eq!(Visibility::parse("gm_only"), Visibility::GmOnly);
    assert_eq!(Visibility::parse("whisper"), Visibility::GmOnly);
}
