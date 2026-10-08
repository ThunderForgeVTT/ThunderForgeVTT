//! Spec 085: the engine knows who owns each token, so a player's box takes
//! only their own.

use super::*;
use crate::payloads::WorldTokenPayload;

fn payload(owner_json: &str) -> WorldTokenPayload {
    let json = format!(r#"{{"id":"t1","x":0,"y":0,"z":0,"label":null{owner_json}}}"#);
    serde_json::from_str(&json).expect("a token payload")
}

fn owned(id: &str) -> TokenOwner {
    TokenOwner(Some(id.to_string()))
}

#[test]
fn a_payload_without_an_owner_keeps_the_current_one() {
    let sent = payload("").owner_user_id;
    assert_eq!(sent, None);
    assert_eq!(next_owner(Some(&owned("aria")), &sent), owned("aria"));
}

#[test]
fn a_null_owner_clears_it() {
    let sent = payload(r#","ownerUserId":null"#).owner_user_id;
    assert_eq!(sent, Some(None));
    assert_eq!(next_owner(Some(&owned("aria")), &sent), TokenOwner(None));
}

#[test]
fn a_user_id_sets_it() {
    let sent = payload(r#","ownerUserId":"bram""#).owner_user_id;
    assert_eq!(next_owner(Some(&owned("aria")), &sent), owned("bram"));
    assert_eq!(next_owner(None, &sent), owned("bram"));
}

fn group_of(tokens: &[&str]) -> crate::resources::GroupSelection {
    crate::resources::GroupSelection {
        tokens: tokens.iter().map(|t| t.to_string()).collect(),
        ..Default::default()
    }
}

#[test]
fn a_token_given_to_someone_else_leaves_a_players_group_and_selection() {
    let mut group = group_of(&["t1", "t2"]);
    let mut selected = SelectedToken(vec!["t1".into(), "t2".into()]);
    assert!(release_disowned(
        false,
        Some("aria"),
        "t1",
        &owned("bram"),
        &mut group,
        &mut selected
    ));
    assert_eq!(group.tokens, vec!["t2".to_string()]);
    assert_eq!(selected.0, vec!["t2".to_string()]);
}

#[test]
fn a_token_still_theirs_stays_and_a_gms_group_is_never_trimmed() {
    let mut group = group_of(&["t1", "t2"]);
    let mut selected = SelectedToken(vec!["t1".into(), "t2".into()]);
    assert!(!release_disowned(
        false,
        Some("aria"),
        "t1",
        &owned("aria"),
        &mut group,
        &mut selected
    ));
    assert!(!release_disowned(
        true,
        Some("gm"),
        "t1",
        &owned("bram"),
        &mut group,
        &mut selected
    ));
    assert_eq!(group.tokens.len(), 2);
    assert_eq!(selected.0.len(), 2);
}

#[test]
fn a_single_selection_outside_the_group_is_not_the_groups_to_take() {
    let mut group = group_of(&[]);
    let mut selected = SelectedToken(vec!["npc".into()]);
    assert!(!release_disowned(
        false,
        Some("aria"),
        "npc",
        &TokenOwner(None),
        &mut group,
        &mut selected
    ));
    assert_eq!(selected.0, vec!["npc".to_string()]);
}
