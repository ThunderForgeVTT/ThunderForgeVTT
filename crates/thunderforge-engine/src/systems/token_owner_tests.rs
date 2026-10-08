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
