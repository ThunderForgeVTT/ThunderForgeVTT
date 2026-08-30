//! Interactive elements, server side: the registry, and the rules the
//! GraphQL layer obeys.
//!
//! # What is here and what is deliberately not
//!
//! The *rules* live in `thunderforge_canvas_core::interaction` — which effect
//! may attach to which subject, what configuration is valid, and the
//! activation truth table. They live there because that crate's tests execute
//! and the engine crate's do not, and because the engine, the server and the
//! web app must all agree on one definition rather than three.
//!
//! What is here is the part that needs a database: assembling the registry
//! from what this build compiled in, loading an interactive and everything the
//! decision depends on, and the approval lifecycle.
//!
//! The distinction matters when reading `resolve_activation` below. This
//! module's job is to gather facts and obey the answer, never to re-derive it.
//! A second copy of the truth table here would be a second thing to keep
//! right.
//!
//! See `docs/adrs/20260830-054-interaction_effect_contribution_seam.md`.

use std::sync::OnceLock;

use diesel::prelude::*;
use uuid::Uuid;

use thunderforge_canvas_core::interaction::{
    Activation, ActivationContext, ActivationOutcome, EffectDeclaration, EffectRegistry, FireMode,
    SubjectKind,
};

/// Every effect this build can perform.
///
/// # Why this list is code and not configuration
///
/// An effect is a capability of the build, not content that varies per world.
/// Expressed as data — a manifest, a table — a deployment could declare an
/// effect no code performs, which is the dead-option problem arriving from the
/// other direction: a Game Master offered something that silently does
/// nothing.
///
/// Adding a contributor is one line here plus a declaration set in the module
/// that owns the subsystem. Nothing in the interaction core changes, which is
/// the property `scripts/verify.mjs` checks textually.
fn contributions() -> Vec<Vec<EffectDeclaration>> {
    vec![
        // Contributors are added here as their subsystems gain the ability to
        // be triggered. An empty list is a legitimate build: the seam then
        // offers nothing, which is correct rather than broken.
        thunderforge_canvas_core::lore_link::effects(),
    ]
}

/// The assembled registry, built once.
///
/// # Why a collision is a panic
///
/// Two contributors declaring one id is a programming error in *this build*,
/// discovered at startup, with a fix that is a source change. There is no
/// runtime recovery that makes sense: serving with one of the two silently
/// dropped would mean a Game Master's authored interactive stops working for
/// reasons nothing reports.
///
/// A collision found later — when a GM happens to author one of the two — is a
/// collision found at the table, mid-session, by the people least able to do
/// anything about it. Failing loudly at boot is the kinder end of that trade.
pub fn registry() -> &'static EffectRegistry {
    static REGISTRY: OnceLock<EffectRegistry> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        EffectRegistry::assemble(contributions())
            .expect("effect declarations collide — this is a build error, not a runtime one")
    })
}

/// Whether a stored `effect_id` is something this build can still perform.
///
/// Absence is detected *here*, by comparing against the registry — never by
/// dispatching and observing that nothing happened. A Bevy event is
/// fire-and-forget and cannot report that nobody listened (FR-041).
pub fn is_available(effect_id: Option<&str>) -> bool {
    match effect_id {
        // No effect is not the same as an absent one. Scenery is available in
        // the only sense that matters: nothing is missing.
        None => true,
        Some(id) => registry().contains(id),
    }
}

/// An interactive and everything the activation decision depends on.
pub struct LoadedInteractive {
    pub row: crate::models::Interactive,
    /// Whether the subject refuses player state changes — a locked door.
    pub subject_locked: bool,
}

impl LoadedInteractive {
    pub fn subject_kind(&self) -> Option<SubjectKind> {
        SubjectKind::from_str_loose(&self.row.subject_kind)
    }

    pub fn activation(&self) -> Activation {
        // An unrecognised stored spelling falls back to the *most* restrictive
        // mode rather than the least. A corrupt row must not become an open
        // door.
        Activation::from_str_loose(&self.row.activation).unwrap_or(Activation::GmOnly)
    }

    pub fn fire_mode(&self) -> FireMode {
        FireMode::from_str_loose(&self.row.fire_mode).unwrap_or(FireMode::Always)
    }

    /// Gather the facts and let `canvas-core` decide.
    pub fn context(&self, actor_is_gm: bool) -> ActivationContext {
        ActivationContext {
            actor_is_gm,
            has_effect: self.row.effect_id.is_some(),
            effect_available: is_available(self.row.effect_id.as_deref()),
            subject_locked: self.subject_locked,
            activation: self.activation(),
            fire_mode: self.fire_mode(),
            has_fired: self.row.fired_at.is_some(),
        }
    }

    pub fn outcome(&self, actor_is_gm: bool) -> ActivationOutcome {
        thunderforge_canvas_core::interaction::resolve_activation(self.context(actor_is_gm))
    }
}

/// Load one interactive, and whatever its subject says about permission.
pub fn load(
    conn: &mut PgConnection,
    interactive_id: Uuid,
) -> Result<LoadedInteractive, diesel::result::Error> {
    use crate::schema::{interactives, walls};

    let row: crate::models::Interactive = interactives::table
        .filter(interactives::interactive_id.eq(interactive_id))
        .select(crate::models::Interactive::as_select())
        .first(conn)?;

    // A door's lock lives on the wall, because the lock is a property of the
    // door rather than of the interactive pointing at it. Two interactives on
    // one door therefore cannot disagree about whether it is locked.
    let subject_locked = match (row.subject_kind.as_str(), row.subject_ref) {
        ("door", Some(wall_id)) => walls::table
            .filter(walls::wall_id.eq(wall_id))
            .select(walls::locked)
            .first::<bool>(conn)
            .optional()?
            .unwrap_or(false),
        _ => false,
    };

    Ok(LoadedInteractive {
        row,
        subject_locked,
    })
}

/// Every interactive on a scene.
pub fn for_scene(
    conn: &mut PgConnection,
    scene_id: Uuid,
) -> Result<Vec<crate::models::Interactive>, diesel::result::Error> {
    use crate::schema::interactives;

    interactives::table
        .filter(interactives::scene_id.eq(scene_id))
        // Stable order, so a token crossing two overlapping regions at once
        // fires them reproducibly rather than however the planner felt.
        .order(interactives::interactive_id.asc())
        .select(crate::models::Interactive::as_select())
        .load(conn)
}

/// Delete every interactive whose subject has gone.
///
/// A door on a deleted wall is not a thing. Done explicitly rather than by a
/// foreign key because `subject_ref` points at two tables and which one is
/// decided by `subject_kind`.
pub fn drop_for_subject(
    conn: &mut PgConnection,
    subject_ref: Uuid,
) -> Result<usize, diesel::result::Error> {
    use crate::schema::interactives;

    diesel::delete(interactives::table.filter(interactives::subject_ref.eq(subject_ref)))
        .execute(conn)
}

// ---------------------------------------------------------------------------
// Approval requests
// ---------------------------------------------------------------------------

pub const REQUEST_PENDING: &str = "pending";
pub const REQUEST_APPROVED: &str = "approved";
pub const REQUEST_REFUSED: &str = "refused";
pub const REQUEST_CANCELLED: &str = "cancelled";

/// Raise a request for a Game Master to decide.
pub fn raise_request(
    conn: &mut PgConnection,
    interactive_id: Uuid,
    scene_id: Uuid,
    requested_by: Uuid,
) -> Result<Uuid, diesel::result::Error> {
    use crate::schema::interaction_requests as r;

    let now = chrono::Utc::now().naive_utc();
    let request_id = Uuid::now_v7();
    diesel::insert_into(r::table)
        .values((
            r::request_id.eq(request_id),
            r::interactive_id.eq(interactive_id),
            r::scene_id.eq(scene_id),
            r::requested_by.eq(requested_by),
            r::state.eq(REQUEST_PENDING),
            r::created_by.eq(requested_by),
            r::updated_by.eq(requested_by),
            r::created_at.eq(now),
            r::updated_at.eq(now),
        ))
        .execute(conn)?;
    Ok(request_id)
}

/// Everything still waiting on a decision in this scene.
pub fn pending_for_scene(
    conn: &mut PgConnection,
    scene_id: Uuid,
) -> Result<Vec<crate::models::InteractionRequest>, diesel::result::Error> {
    use crate::schema::interaction_requests as r;

    r::table
        .filter(r::scene_id.eq(scene_id))
        .filter(r::state.eq(REQUEST_PENDING))
        .order(r::created_at.asc())
        .select(crate::models::InteractionRequest::as_select())
        .load(conn)
}

/// Move a pending request to a decided state.
///
/// Returns whether anything changed. The filter on `pending` is what makes a
/// second decision on the same request a no-op rather than an overwrite: two
/// Game Masters clicking approve and refuse must not race into whichever
/// transaction committed last.
pub fn decide(
    conn: &mut PgConnection,
    request_id: Uuid,
    state: &str,
    decided_by: Uuid,
) -> Result<bool, diesel::result::Error> {
    use crate::schema::interaction_requests as r;

    let now = chrono::Utc::now().naive_utc();
    let changed = diesel::update(
        r::table
            .filter(r::request_id.eq(request_id))
            .filter(r::state.eq(REQUEST_PENDING)),
    )
    .set((
        r::state.eq(state),
        r::decided_by.eq(decided_by),
        r::decided_at.eq(now),
        r::updated_by.eq(decided_by),
        r::updated_at.eq(now),
    ))
    .execute(conn)?;
    Ok(changed > 0)
}

/// Cancel every pending request raised by someone who has left.
///
/// Cancelled rather than left pending: a Game Master should not be asked to
/// decide something for a player who is no longer at the table, and a queue
/// that accumulates those is a queue nobody trusts.
pub fn cancel_for_requester(
    conn: &mut PgConnection,
    scene_id: Uuid,
    requested_by: Uuid,
) -> Result<usize, diesel::result::Error> {
    use crate::schema::interaction_requests as r;

    let now = chrono::Utc::now().naive_utc();
    diesel::update(
        r::table
            .filter(r::scene_id.eq(scene_id))
            .filter(r::requested_by.eq(requested_by))
            .filter(r::state.eq(REQUEST_PENDING)),
    )
    .set((
        r::state.eq(REQUEST_CANCELLED),
        r::updated_by.eq(requested_by),
        r::updated_at.eq(now),
    ))
    .execute(conn)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_registry_assembles() {
        // If two contributors ever collide, this fails here rather than at
        // somebody's table.
        let _ = registry();
    }

    #[test]
    fn scenery_is_available_because_nothing_is_missing() {
        assert!(is_available(None));
    }

    #[test]
    fn the_lore_contributor_is_in_this_build() {
        assert!(is_available(Some(
            thunderforge_canvas_core::lore_link::OPEN
        )));
    }

    #[test]
    fn an_effect_no_contributor_declares_is_unavailable_rather_than_dispatched() {
        // FR-041. Detected by asking the registry — never by dispatching and
        // watching nothing happen, which an event cannot report.
        assert!(!is_available(Some("audio.play")));
    }

    #[test]
    fn a_corrupt_activation_spelling_closes_rather_than_opens() {
        let row = crate::models::Interactive {
            interactive_id: Uuid::nil(),
            scene_id: Uuid::nil(),
            subject_kind: String::from("prop"),
            subject_ref: Some(Uuid::nil()),
            geometry: None,
            effect_id: Some(String::from("thing.do")),
            effect_config: None,
            trigger: String::from("click"),
            activation: String::from("ANYONE"),
            fire_mode: String::from("always"),
            fired_at: None,
            created_by: Uuid::nil(),
            updated_by: Uuid::nil(),
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: chrono::Utc::now().naive_utc(),
        };
        let loaded = LoadedInteractive {
            row,
            subject_locked: false,
        };
        // Wrong case is not "anyone". A row this module cannot read must not
        // become the permissive answer.
        assert_eq!(loaded.activation(), Activation::GmOnly);
    }
}
