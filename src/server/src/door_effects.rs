//! Doors: designation, lock, secrecy, and performing the effects they
//! contribute to the interaction seam.
//!
//! Spec 030, US2 and US4. This module is a *contributor*. It declares its
//! effects in `thunderforge_canvas_core::wall`, performs them here, and
//! nothing in the interaction core knows it exists — remove it and the seam
//! goes on working with three fewer effects.
//!
//! # What open, closed and locked mean
//!
//! Open blocks neither vision nor movement. Closed blocks exactly what the
//! wall's own profile says, so a closed window stays see-through and a closed
//! stone door does not. `locked` is a separate property governing *who may
//! change the state*, never the state itself.
//!
//! The rule itself lives in `canvas-core` (`Wall::blocking`), where it is
//! tested. This module moves rows.
//!
//! # One decision worth stating rather than burying
//!
//! A lock refuses a player who clicks *the door*. It does not refuse an effect
//! a Game Master authored — a lever that opens a barred portcullis is exactly
//! the puzzle a GM builds, and a lock that silently defeated their own lever
//! would be a trap in the authoring tool rather than a rule at the table.
//!
//! So: locked governs the hand on the door. What the GM wired up is the GM's
//! own instrument.

use diesel::prelude::*;
use uuid::Uuid;

use thunderforge_canvas_core::wall::{
    DoorState, REVEAL, SET_LOCK, SET_STATE, requested_lock, requested_state, target_of,
};

/// Whether this module declared the effect.
pub fn handles(effect_id: &str) -> bool {
    matches!(effect_id, SET_STATE | SET_LOCK | REVEAL)
}

/// Perform one door effect, authoritatively.
///
/// Returns the wall it changed, so the caller can announce it. `None` means
/// the effect asked for something that no longer makes sense — a wall that has
/// been deleted, or a state nothing recognises — which is reported rather than
/// guessed at.
pub fn perform(
    conn: &mut PgConnection,
    effect_id: &str,
    config: &serde_json::Value,
    scene_id: Uuid,
) -> Result<Option<Uuid>, diesel::result::Error> {
    // A transaction, because the read below takes a row lock and a lock
    // outside one is released immediately — which would make it decoration
    // rather than protection.
    conn.transaction(|conn| perform_locked(conn, effect_id, config, scene_id))
}

fn perform_locked(
    conn: &mut PgConnection,
    effect_id: &str,
    config: &serde_json::Value,
    scene_id: Uuid,
) -> Result<Option<Uuid>, diesel::result::Error> {
    use crate::schema::walls;

    let Some(target) = target_of(config).and_then(|t| Uuid::parse_str(t).ok()) else {
        return Ok(None);
    };

    // Scoped to the scene the interactive is on. An effect must not reach
    // across scenes, whatever a stored configuration claims — the interactive
    // belongs to one scene and so does everything it may touch.
    //
    // Locked for update, and the reason is `toggle`: it reads the current
    // state and writes the other one, so two players clicking the same door at
    // the same moment would otherwise both read "closed" and both write
    // "open" — or, worse, interleave into a state neither of them asked for.
    // Serialising the pair here is what makes concurrent activation resolve to
    // one answer (SC-005), and it is one line rather than a reconciliation
    // protocol.
    let existing: Option<(String, bool, bool)> = walls::table
        .filter(walls::wall_id.eq(target))
        .filter(walls::scene_id.eq(scene_id))
        .select((walls::door_state, walls::locked, walls::secret))
        .for_update()
        .first(conn)
        .optional()?;

    let Some((door_state, _locked, _secret)) = existing else {
        return Ok(None);
    };
    let current = DoorState::from_str_loose(&door_state);
    let now = chrono::Utc::now().naive_utc();

    match effect_id {
        SET_STATE => {
            let Some(next) = requested_state(config, current) else {
                return Ok(None);
            };
            // A wall that is not a door has no state to set. Turning one into
            // a door as a side effect of an activation would be an edit
            // nobody asked for.
            if current == DoorState::None {
                return Ok(None);
            }
            diesel::update(walls::table.filter(walls::wall_id.eq(target)))
                .set((
                    walls::door_state.eq(next.as_str()),
                    walls::updated_at.eq(now),
                ))
                .execute(conn)?;
        }
        SET_LOCK => {
            let Some(locked) = requested_lock(config) else {
                return Ok(None);
            };
            diesel::update(walls::table.filter(walls::wall_id.eq(target)))
                .set((walls::locked.eq(locked), walls::updated_at.eq(now)))
                .execute(conn)?;
        }
        REVEAL => {
            // One-way, deliberately. Re-hiding a door the table has already
            // seen is a fiction problem, not a state problem, and no scenario
            // asks for it.
            diesel::update(walls::table.filter(walls::wall_id.eq(target)))
                .set((walls::secret.eq(false), walls::updated_at.eq(now)))
                .execute(conn)?;
        }
        _ => return Ok(None),
    }

    Ok(Some(target))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn this_module_claims_exactly_what_it_declared() {
        // If these ever disagree, an authored effect validates and then
        // nothing performs it — the silent failure the whole seam is built to
        // avoid, arriving from the one direction the registry cannot catch.
        for declaration in thunderforge_canvas_core::wall::interaction_effects() {
            assert!(
                handles(&declaration.id),
                "{} is declared but not performed",
                declaration.id
            );
        }
    }

    #[test]
    fn nothing_else_is_claimed() {
        assert!(!handles(thunderforge_canvas_core::lore_link::OPEN));
        assert!(!handles("audio.play"));
    }
}
