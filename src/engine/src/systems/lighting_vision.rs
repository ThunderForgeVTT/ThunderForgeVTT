//! Who sees what: whose eyes a client looks through, and what it reports back.
//!
//! Split out of `lighting.rs`, which does the seeing. These are the *inputs
//! and outputs* of that pass rather than part of it — the viewer token and the
//! party's eyes going in, the hidden and marked lists coming out — and they
//! are what the application and its tests talk to.
//!
//! All four are session-local. Which token is mine to see through, and which
//! tokens the table sees through, are facts about this client; they are never
//! synced and never broadcast.

use bevy::prelude::*;

/// The token this client sees the board through — the local player's own,
/// named by the application (`set_viewer_token`). `None` for a Game Master,
/// and for anyone the application has not named one for.
#[derive(Resource, Default, Debug, Clone, PartialEq)]
pub(crate) struct ViewerToken(pub Option<String>);

type ViewerRequest = Option<Option<String>>;

static REQUESTED_VIEWER: std::sync::OnceLock<std::sync::Mutex<ViewerRequest>> =
    std::sync::OnceLock::new();

/// Name the token this client sees the board through; `""` for none.
///
/// Queued and applied on the next frame, like the engine's other web
/// commands. Local session state, not world state: which token is "mine" is
/// a fact about this viewer, so it is never synced or broadcast.
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn set_viewer_token(token_id: &str) -> bool {
    let request = (!token_id.is_empty()).then(|| token_id.to_string());
    let slot = REQUESTED_VIEWER.get_or_init(|| std::sync::Mutex::new(None));
    if let Ok(mut pending) = slot.lock() {
        *pending = Some(request);
        return true;
    }
    false
}

pub(crate) fn apply_requested_viewer(mut viewer: ResMut<ViewerToken>) {
    let Some(slot) = REQUESTED_VIEWER.get() else {
        return;
    };
    let Ok(mut pending) = slot.lock() else {
        return;
    };
    if let Some(request) = pending.take() {
        viewer.set_if_neq(ViewerToken(request));
    }
}

/// The tokens the table's players see through — the party's eyes.
///
/// Only a Game Master's client is told these, and only to answer FR-033: which
/// tokens can the table not currently see? Every other client sees through one
/// token, its own, which is `ViewerToken`.
///
/// A list of ids rather than something the engine derives, because "whose
/// token is this" is a question about accounts and world membership, which the
/// engine has never known and should not learn. The application resolves each
/// player to their token exactly as FR-001 defines it and names the result.
#[derive(Resource, Default, Debug, Clone, PartialEq)]
pub(crate) struct PartyEyes(pub Vec<String>);

static REQUESTED_PARTY_EYES: std::sync::OnceLock<std::sync::Mutex<Option<Vec<String>>>> =
    std::sync::OnceLock::new();

/// Name the tokens the table's players see through, as a JSON array of ids.
///
/// Local session state, like the viewer token: it is the Game Master's client
/// working out what to mark, never world state, and it is never broadcast.
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn set_party_eyes(token_ids_json: &str) -> bool {
    let Ok(ids) = serde_json::from_str::<Vec<String>>(token_ids_json) else {
        // A malformed list means the application is confused about who is at
        // the table. Marking nothing is the safe reading: a Game Master who
        // sees no marks looks at the board, while one shown wrong marks
        // believes something false about what their players can see.
        return false;
    };
    let slot = REQUESTED_PARTY_EYES.get_or_init(|| std::sync::Mutex::new(None));
    if let Ok(mut pending) = slot.lock() {
        *pending = Some(ids);
        return true;
    }
    false
}

pub(crate) fn apply_requested_party_eyes(mut eyes: ResMut<PartyEyes>) {
    let Some(slot) = REQUESTED_PARTY_EYES.get() else {
        return;
    };
    let Ok(mut pending) = slot.lock() else {
        return;
    };
    if let Some(request) = pending.take() {
        eyes.set_if_neq(PartyEyes(request));
    }
}

static HIDDEN_TOKENS: std::sync::OnceLock<std::sync::Mutex<Vec<String>>> =
    std::sync::OnceLock::new();

pub(crate) fn mirror_hidden_tokens(mut hidden: Vec<String>) {
    hidden.sort_unstable();
    let slot = HIDDEN_TOKENS.get_or_init(|| std::sync::Mutex::new(Vec::new()));
    if let Ok(mut current) = slot.lock()
        && *current != hidden
    {
        *current = hidden;
    }
}

/// The ids of the tokens this client currently hides, as a JSON array.
///
/// Read-only, and here so a test can ask what a player's canvas withholds —
/// a token out of their sight — rather than infer it from pixels.
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hidden_tokens() -> String {
    let list = HIDDEN_TOKENS
        .get()
        .and_then(|slot| slot.lock().ok().map(|l| l.clone()))
        .unwrap_or_default();
    serde_json::Value::from(list).to_string()
}

static MARKED_TOKENS: std::sync::OnceLock<std::sync::Mutex<Vec<String>>> =
    std::sync::OnceLock::new();

pub(crate) fn mirror_marked_tokens(mut marked: Vec<String>) {
    marked.sort_unstable();
    let slot = MARKED_TOKENS.get_or_init(|| std::sync::Mutex::new(Vec::new()));
    if let Ok(mut current) = slot.lock()
        && *current != marked
    {
        *current = marked;
    }
}

/// The ids of the tokens marked on a Game Master's board, as a JSON array.
///
/// FR-033's other half. `hidden_tokens` answers "what does this player's
/// canvas withhold"; this answers "which of these can the table not see" —
/// the same question from the other chair, and always empty for a player,
/// who is never told what anyone else can see.
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn marked_tokens() -> String {
    let list = MARKED_TOKENS
        .get()
        .and_then(|slot| slot.lock().ok().map(|l| l.clone()))
        .unwrap_or_default();
    serde_json::Value::from(list).to_string()
}
