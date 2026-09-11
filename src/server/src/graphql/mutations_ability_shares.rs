//! Spec 025 US6: ability share links and Copy-to-World (`sharedAbility`,
//! `createAbilityShareLink`, `revokeAbilityShareLink`,
//! `copySharedAbilityToWorld`). See contracts/ability-share.md.
//!
//! Governed by `docs/adrs/20260825-049-share_link_dmca_repository_determination.md`
//! (Accepted 2026-08-25), whose finding that share links are **not** a
//! centralized public repository is conditional on six invariants. Two of them
//! live in this file and must stay true:
//!
//! * **No enumeration** — there is deliberately no query here that lists shares
//!   by world, by user, or in aggregate. Adding one re-opens the determination.
//! * **Unguessable codes** — v4-derived, never v7.
//!
//! Ownership model (ADR-049): the world owner owns what they author; the
//! platform hosts it and may forward a DMCA notice to that owner. Sharing is
//! non-shared and non-discoverable by default — a link exists only because
//! someone deliberately created one.

use async_graphql::{Context, Error, InputObject, Result as GraphQLResult};
use diesel::prelude::*;
use uuid::Uuid;

use crate::auth::ability_permissions::effective_ability_permission;
use crate::auth::world_membership::is_dm_of_world;
use crate::graphql::anonymous::caller_id;
use crate::graphql::share_codes::generate_link_code;
use crate::graphql::share_rate_limit as rate_limit;
use crate::graphql::types::{
    ActorPermissionLevel, GraphQLAbility, GraphQLAbilityEffect, GraphQLAbilityShareLink,
    SharedAbilityPreview,
};
use crate::graphql::{app_state, authenticated_user};
use crate::models::{
    AbilityEffect, AbilityShare, NewAbilityEffect, NewAbilityShare, NewWorldAbility, WorldAbility,
};
use crate::schema::{world_abilities, world_ability_effects, world_ability_shares};
use crate::state::AppState;

#[derive(InputObject, Debug, Clone)]
pub struct CopySharedAbilityInput {
    pub share_code: String,
    pub destination_world_id: Uuid,
}

/// Newtype so the copy transaction's closure can return one error type —
/// an orphan-rule workaround, matching `mutations_item_shares.rs`.
struct CopyError(String);

impl From<diesel::result::Error> for CopyError {
    fn from(e: diesel::result::Error) -> Self {
        CopyError(e.to_string())
    }
}

impl From<String> for CopyError {
    fn from(s: String) -> Self {
        CopyError(s)
    }
}

/// The one sentence every failed lookup in this module produces.
///
/// ADR-071: an unknown code, a revoked share, a deleted ability and a moderated
/// one must be indistinguishable to an outsider, because distinguishing them is
/// a probe — and that matters more now the caller need not have an account. A
/// constant rather than four string literals so they cannot drift apart later,
/// which is exactly how this kind of leak is usually introduced.
pub const UNAVAILABLE: &str = "This share link is no longer available";

fn load_active_share(
    conn: &mut diesel::PgConnection,
    share_code: &str,
) -> Result<AbilityShare, String> {
    let share = world_ability_shares::table
        .filter(world_ability_shares::share_code.eq(share_code))
        .filter(world_ability_shares::revoked.eq(false))
        .select(AbilityShare::as_select())
        .first::<AbilityShare>(conn)
        .map_err(|_| UNAVAILABLE.to_string())?;
    // Spec 039 FR-038: a disabled owner's links resolve as dead. Fails closed.
    if crate::moderation::standing::is_disabled_sync(conn, share.created_by).unwrap_or(true) {
        return Err(UNAVAILABLE.to_string());
    }
    Ok(share)
}

/// Testable core of `sharedAbility` (FR-033).
///
/// **Unauthenticated** — ADR-071. Do not add `authenticated_user(ctx)?` to the
/// resolver that calls this; the session requirement was removed deliberately,
/// on the same terms ADR-070 set for `sharedCollection`. There is no
/// world-membership check either, which is the point of a share link.
///
/// `caller` is used for one thing: rate limiting, before the lookup. An
/// unguessable code is unguessable only while the number of guesses is bounded,
/// and once no account is needed, nothing else bounds them.
///
/// Blocked entirely for a moderated ability, so a share can never become a
/// moderation bypass.
pub async fn shared_ability_impl(
    state: &AppState,
    caller: &str,
    share_code: String,
) -> GraphQLResult<SharedAbilityPreview> {
    // ADR-071 (and FR-009c's reasoning): before the lookup, never after.
    if !rate_limit::allow_request(caller) {
        return Err(Error::new(rate_limit::rate_limited_message()));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let (ability, effects, world_system_id) = tokio::task::spawn_blocking(move || {
        let share = load_active_share(&mut conn, &share_code)?;
        let ability = world_abilities::table
            .filter(world_abilities::id.eq(share.ability_id))
            .select(WorldAbility::as_select())
            .first::<WorldAbility>(&mut conn)
            .map_err(|_| UNAVAILABLE.to_string())?;
        // The owning world's system, so the label below is the word that
        // world would show rather than the application's default.
        let world_system_id: Option<String> = crate::schema::worlds::table
            .filter(crate::schema::worlds::id.eq(ability.world_id))
            .select(crate::schema::worlds::game_system_id)
            .first::<Option<String>>(&mut conn)
            .map_err(|e| e.to_string())?;
        let effects = world_ability_effects::table
            .filter(world_ability_effects::ability_id.eq(ability.id))
            .order(world_ability_effects::sort_order.asc())
            .select(AbilityEffect::as_select())
            .load::<AbilityEffect>(&mut conn)
            .map_err(|e| e.to_string())?;
        Ok::<_, String>((ability, effects, world_system_id))
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)?;

    if crate::moderation::effective_status(state, "world_ability", ability.id)
        .await?
        .is_some()
    {
        return Err(Error::new(UNAVAILABLE));
    }

    // The label the owning world would show. Resolved here because the viewer
    // is deliberately not a member of that world and cannot read its
    // vocabulary (FR-006).
    let classification_label =
        ability_label_for_world(state, world_system_id.as_deref(), &ability.classification);

    Ok(SharedAbilityPreview {
        name: ability.name,
        description: ability.description,
        // The identity as stored. T037: this used to resolve an unknown value
        // to `Spell`, so a shared Enchantment read as a Spell — the silent
        // mislabelling FR-034 forbids. The label beside it is resolved from the
        // owning world's vocabulary.
        classification: crate::graphql::types::normalise_classification(&ability.classification),
        classification_label,
        effects: effects
            .into_iter()
            .map(GraphQLAbilityEffect::from)
            .collect(),
    })
}

/// One ability's type label, in the words of the world that owns it.
///
/// The vocabulary is assembled with the ability's own type counted as in use,
/// so a type the active system no longer recognises still resolves to itself
/// rather than to another type's name. An unrecognised type reads as the
/// identity it was authored under, which is what FR-035 asks for.
fn ability_label_for_world(
    state: &AppState,
    world_system_id: Option<&str>,
    classification: &str,
) -> String {
    let in_use = [classification.to_string()];
    crate::ability_vocabulary::for_system(&state.directories.systems_dir, world_system_id, &in_use)
        .get(classification)
        .map(|kind| kind.label.clone())
        .unwrap_or_else(|| classification.to_string())
}

/// Testable core of `abilityShareLink` — the active share for a ability the
/// caller owns, or null.
///
/// # Why it exists
///
/// ADR-071's second half. The revoke mutation shipped without a read path, so
/// revoking only worked inside the browser session that minted the link: the
/// code was shown once, and closing the tab removed the owner's ability to
/// recall it permanently. Spec 026 recorded that defect against all three
/// singleton shares and held it under FR-009e; collections answered it with
/// `collectionShareLink` and this is the same answer.
///
/// It matters more now the read is anonymous, not less: a link that reaches the
/// public and cannot be recalled by its owner is exactly what ADR-049's
/// ownership model exists to prevent.
///
/// # Why this is not the enumeration FR-020 forbids
///
/// It is scoped to one ability the caller already has Owner-level authority
/// over — the same authority needed to mint the link in the first place. It
/// reaches nothing by world, by user, or in aggregate, so nothing becomes
/// discoverable that was not already.
pub async fn ability_share_link_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    ability_id: Uuid,
) -> GraphQLResult<Option<AbilityShare>> {
    // The authority to see a link is the authority to have made one.
    let level = effective_ability_permission(state, user_id, is_admin, ability_id).await?;
    if level.rank() < ActorPermissionLevel::Owner.rank() {
        return Err(Error::new(
            "Only an Owner of this ability may see its share link",
        ));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        world_ability_shares::table
            .filter(world_ability_shares::ability_id.eq(ability_id))
            .filter(world_ability_shares::revoked.eq(false))
            .order(world_ability_shares::created_at.desc())
            .select(AbilityShare::as_select())
            .first::<AbilityShare>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|e| Error::new(format!("Failed to load the share link: {e}")))
}

/// Testable core of `createAbilityShareLink` (FR-032). Owner-level only.
pub async fn create_ability_share_link_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    ability_id: Uuid,
    attestation: &crate::publishing::AttestationInput,
) -> GraphQLResult<AbilityShare> {
    // Spec 040 FR-026: an instance with no contact for copyright notices
    // publishes nothing beyond a world. Asked first, before ownership, so a
    // misconfigured instance answers the same sentence to every caller
    // instead of leaking which of them owns what — and asked here, in the
    // impl, so a caller that bypasses the page is refused too (spec 039
    // FR-011, FR-014). Reading an existing share is deliberately not gated.
    crate::readiness::may_publish_beyond_world(state)
        .await
        .map_err(Error::new)?;

    let level = effective_ability_permission(state, user_id, is_admin, ability_id).await?;
    if level.rank() < ActorPermissionLevel::Owner.rank() {
        return Err(Error::new(
            "Only an Owner of this ability may create a share link",
        ));
    }

    // Defensive: a non-DM can never reach a GM-only ability's detail data
    // (FR-025), so should never get here — but sharing is the one path that
    // escapes the world, so it re-checks rather than relying on that.
    if !crate::auth::ability_permissions::is_ability_visible_to(
        state, user_id, is_admin, ability_id,
    )
    .await?
    {
        return Err(Error::new("Ability not found"));
    }

    // Spec 039 FR-011/FR-014. Last of the refusals, and the gate writes
    // nothing — what it returns is written in the transaction below.
    let pending = crate::publishing::require_attestation(
        state,
        user_id,
        crate::attestation::PublishableKind::Ability,
        ability_id,
        // Filled in from the ability's own row inside the transaction, so this
        // stays one round trip.
        None,
        attestation,
    )
    .await?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let new_share = NewAbilityShare {
        id: Uuid::now_v7(),
        ability_id,
        share_code: generate_link_code(),
        created_by: user_id,
    };

    tokio::task::spawn_blocking(move || {
        // One transaction: no interleaving leaves a share link without its
        // attestation, or an attestation for a share that failed (FR-011).
        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            let share = diesel::insert_into(world_ability_shares::table)
                .values(&new_share)
                .returning(AbilityShare::as_returning())
                .get_result::<AbilityShare>(conn)?;

            let world_id: Uuid = world_abilities::table
                .filter(world_abilities::id.eq(ability_id))
                .select(world_abilities::world_id)
                .first(conn)?;

            crate::attestation::record_sync(
                conn,
                &crate::attestation::PendingAttestation {
                    world_id: Some(world_id),
                    ..pending
                },
                share.id,
            )?;
            Ok(share)
        })
        .map_err(|e| format!("Failed to create share link: {e}"))
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)
}

/// Testable core of `revokeAbilityShareLink` (FR-036).
///
/// Soft flag, never a delete: a revoked link must render a distinct "no longer
/// available" state, which a deleted row could not distinguish from a code that
/// never existed.
pub async fn revoke_ability_share_link_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    share_id: Uuid,
) -> GraphQLResult<bool> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let (created_by, ability_id) = tokio::task::spawn_blocking(move || {
        world_ability_shares::table
            .filter(world_ability_shares::id.eq(share_id))
            .select((
                world_ability_shares::created_by,
                world_ability_shares::ability_id,
            ))
            .first::<(Uuid, Uuid)>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to load share link"))?
    .ok_or_else(|| Error::new("Share link not found"))?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let world_id = tokio::task::spawn_blocking(move || {
        world_abilities::table
            .filter(world_abilities::id.eq(ability_id))
            .select(world_abilities::world_id)
            .first::<Uuid>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to load ability"))?
    .ok_or_else(|| Error::new("Ability not found"))?;

    // The link's creator, or a DM of its world, may revoke.
    if created_by != user_id && !is_dm_of_world(state, user_id, is_admin, world_id).await? {
        return Err(Error::new("You may not revoke this share link"));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        diesel::update(world_ability_shares::table.filter(world_ability_shares::id.eq(share_id)))
            .set((
                world_ability_shares::revoked.eq(true),
                world_ability_shares::updated_at.eq(chrono::Utc::now().naive_utc()),
            ))
            .execute(&mut conn)
            .map(|rows| rows > 0)
            .map_err(|e| format!("Failed to revoke share link: {e}"))
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)
}

/// Testable core of `copySharedAbilityToWorld` (FR-035).
///
/// A one-time deep copy producing a fully independent record. Two deliberate
/// divergences from the item version:
///
/// * **`gm_only` is preserved.** Fail closed — a copy arriving un-hidden would
///   silently expose content hidden at the source. The destination DM can clear
///   it themselves.
/// * **Effect formulas are re-validated.** The item version clones effects
///   without re-running `validate_formula`; the source's validity is an
///   assumption rather than a guarantee.
pub async fn copy_shared_ability_to_world_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    input: CopySharedAbilityInput,
) -> GraphQLResult<(WorldAbility, Vec<AbilityEffect>)> {
    if !is_dm_of_world(state, user_id, is_admin, input.destination_world_id).await? {
        return Err(Error::new(
            "You must be the DM (Owner or GM) of the destination world to copy into it",
        ));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let destination_world_id = input.destination_world_id;
    let share_code = input.share_code;

    tokio::task::spawn_blocking(move || {
        conn.transaction::<_, CopyError, _>(|conn| {
            // Re-validate inside the transaction: the link may have been
            // revoked between the preview and the confirm.
            let share = load_active_share(conn, &share_code)?;

            let source = world_abilities::table
                .filter(world_abilities::id.eq(share.ability_id))
                .select(WorldAbility::as_select())
                .first::<WorldAbility>(conn)?;

            let source_effects = world_ability_effects::table
                .filter(world_ability_effects::ability_id.eq(source.id))
                .order(world_ability_effects::sort_order.asc())
                .select(AbilityEffect::as_select())
                .load::<AbilityEffect>(conn)?;

            let copy = diesel::insert_into(world_abilities::table)
                .values(&NewWorldAbility {
                    world_id: destination_world_id,
                    name: source.name.clone(),
                    description: source.description.clone(),
                    classification: source.classification.clone(),
                    // Carried across with the type it belongs to. A copy that
                    // kept the type and dropped the grade would be a 3rd-level
                    // spell arriving as an unlevelled one.
                    grade: source.grade,
                    // Preserved, not reset — see this function's doc comment.
                    gm_only: source.gm_only,
                    created_by: user_id,
                    updated_by: user_id,
                })
                .returning(WorldAbility::as_returning())
                .get_result::<WorldAbility>(conn)?;

            let mut cloned = Vec::with_capacity(source_effects.len());
            for effect in source_effects {
                // Re-validate rather than trusting the source.
                if effect.formula.trim().is_empty()
                    || !effect.formula.chars().any(|c| c.is_ascii_alphanumeric())
                {
                    return Err(CopyError(format!(
                        "Source ability has an invalid effect formula: {:?}",
                        effect.formula
                    )));
                }
                let row = diesel::insert_into(world_ability_effects::table)
                    .values(&NewAbilityEffect {
                        ability_id: copy.id,
                        effect_type: effect.effect_type,
                        formula: effect.formula,
                        target: effect.target,
                        trigger_kind: effect.trigger_kind,
                        sort_order: effect.sort_order,
                    })
                    .returning(AbilityEffect::as_returning())
                    .get_result::<AbilityEffect>(conn)?;
                cloned.push(row);
            }

            // ADR-079: so a takedown of the source reaches this copy.
            crate::moderation::reach::record_adoption_sync(
                conn,
                "world_ability",
                source.id,
                copy.id,
                destination_world_id,
                user_id,
            )
            .map_err(|e| CopyError(format!("Failed to record the copy: {e}")))?;

            // The copy's ownership block starts empty — the destination DM has
            // implicit full control.
            Ok((copy, cloned))
        })
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|e| Error::new(e.0))
}

#[derive(Default)]
pub struct AbilityShareQuery;

#[async_graphql::Object]
impl AbilityShareQuery {
    /// ADR-071: the active share link for a ability the caller owns, or null.
    /// **Authenticated**, and scoped to one ability the caller already has
    /// authority over — see `ability_share_link_impl` for why this is not
    /// enumeration.
    async fn ability_share_link(
        &self,
        ctx: &Context<'_>,
        ability_id: Uuid,
    ) -> GraphQLResult<Option<GraphQLAbilityShareLink>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        Ok(
            ability_share_link_impl(state, auth_user.user_id, auth_user.is_admin, ability_id)
                .await?
                .map(Into::into),
        )
    }

    /// **Deliberately unauthenticated** — ADR-071. Do not add
    /// `authenticated_user(ctx)?` here; it was removed on purpose, and all four
    /// share reads now agree. The caller is identified only to rate-limit them.
    async fn shared_ability(
        &self,
        ctx: &Context<'_>,
        share_code: String,
    ) -> GraphQLResult<SharedAbilityPreview> {
        let state = app_state(ctx)?;
        shared_ability_impl(state, &caller_id(ctx), share_code).await
    }
}

#[derive(Default)]
pub struct AbilityShareMutation;

#[async_graphql::Object]
impl AbilityShareMutation {
    /// Spec 039 FR-001/FR-002: `attestation` is **non-null**.
    async fn create_ability_share_link(
        &self,
        ctx: &Context<'_>,
        ability_id: Uuid,
        attestation: crate::publishing::AttestationInput,
    ) -> GraphQLResult<GraphQLAbilityShareLink> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let row = create_ability_share_link_impl(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            ability_id,
            &attestation,
        )
        .await?;
        Ok(GraphQLAbilityShareLink {
            id: row.id,
            ability_id: row.ability_id,
            share_code: row.share_code,
            revoked: row.revoked,
            created_at: row.created_at,
        })
    }

    async fn revoke_ability_share_link(
        &self,
        ctx: &Context<'_>,
        share_id: Uuid,
    ) -> GraphQLResult<bool> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        revoke_ability_share_link_impl(state, auth_user.user_id, auth_user.is_admin, share_id).await
    }

    async fn copy_shared_ability_to_world(
        &self,
        ctx: &Context<'_>,
        input: CopySharedAbilityInput,
    ) -> GraphQLResult<GraphQLAbility> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let (copy, effects) =
            copy_shared_ability_to_world_impl(state, auth_user.user_id, auth_user.is_admin, input)
                .await?;
        Ok(GraphQLAbility::from_row(
            copy,
            effects
                .into_iter()
                .map(GraphQLAbilityEffect::from)
                .collect(),
            // The copier is the destination DM, so Owner by definition.
            ActorPermissionLevel::Owner,
        ))
    }
}

#[cfg(test)]
#[path = "mutations_ability_shares_tests.rs"]
mod tests;
