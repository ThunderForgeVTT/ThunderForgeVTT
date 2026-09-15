//! `attack`, `sceneAttacks`, `pendingOffers` and `previewAttack` (spec 046
//! contracts/fight.md §1).
//!
//! Every answer is built for the caller through `types_attacks`, which asks
//! `combat::redaction` what they may know. A world member reads a world's
//! attacks; nobody else reads anything.

use async_graphql::{Context, Error, Object, Result as GraphQLResult};
use diesel::prelude::*;
use uuid::Uuid;

use crate::combat::records::AttackRecord;
use crate::graphql::types::types_attacks::{
    AttackFlag, AttackInput, GraphQLAttack, GraphQLAttackPreview, GraphQLOffer, GraphQLTurnCheck,
    Sights, build_attacks, build_offers,
};
use crate::graphql::{app_state, authenticated_user};
use crate::schema::{scenes, world_attacks};

/// A page of `sceneAttacks`.
pub const SCENE_ATTACKS_PAGE: i64 = 50;

fn member_or_refuse(
    conn: &mut diesel::PgConnection,
    user_id: Uuid,
    is_admin: bool,
    world_id: Uuid,
) -> GraphQLResult<()> {
    let actor = crate::auth::world_membership::actor_in_world(conn, user_id, is_admin, world_id);
    if actor.role.is_none() && !actor.is_site_admin {
        return Err(Error::new("Not a member of this world"));
    }
    Ok(())
}

#[derive(Default)]
pub struct AttackQuery;

#[Object]
impl AttackQuery {
    /// One attack, as the caller may see it: the attacker and target read
    /// "Unknown" to a player who cannot see them (FR-002a).
    async fn attack(&self, ctx: &Context<'_>, id: Uuid) -> GraphQLResult<Option<GraphQLAttack>> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        let (user_id, is_admin) = (user.user_id, user.is_admin);
        let systems_dir = state.directories.systems_dir.clone();
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;
        tokio::task::spawn_blocking(move || {
            let Some(record) = world_attacks::table
                .filter(world_attacks::id.eq(id))
                .select(AttackRecord::as_select())
                .first::<AttackRecord>(&mut conn)
                .optional()
                .map_err(|_| Error::new("Failed to load attack"))?
            else {
                return Ok(None);
            };
            member_or_refuse(&mut conn, user_id, is_admin, record.world_id)?;
            let mut sights = Sights::new(&systems_dir, user_id, is_admin);
            Ok(build_attacks(&mut conn, &mut sights, vec![record])
                .map_err(|e| Error::new(format!("Failed to read attack: {e}")))?
                .pop())
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
    }

    /// A scene's attacks, newest first, fifty at a time. `before` is the last
    /// attack of the previous page.
    async fn scene_attacks(
        &self,
        ctx: &Context<'_>,
        scene_id: Uuid,
        before: Option<Uuid>,
    ) -> GraphQLResult<Vec<GraphQLAttack>> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        let (user_id, is_admin) = (user.user_id, user.is_admin);
        let systems_dir = state.directories.systems_dir.clone();
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;
        tokio::task::spawn_blocking(move || {
            let world_id = scenes::table
                .filter(scenes::scene_id.eq(scene_id))
                .select(scenes::world_id)
                .first::<Uuid>(&mut conn)
                .map_err(|_| Error::new("Scene not found"))?;
            member_or_refuse(&mut conn, user_id, is_admin, world_id)?;

            let mut query = world_attacks::table
                .filter(world_attacks::scene_id.eq(scene_id))
                .into_boxed();
            if let Some(before) = before {
                let cursor = world_attacks::table
                    .filter(world_attacks::id.eq(before))
                    .select((world_attacks::created_at, world_attacks::id))
                    .first::<(chrono::NaiveDateTime, Uuid)>(&mut conn)
                    .optional()
                    .map_err(|_| Error::new("Failed to load attacks"))?;
                if let Some((at, id)) = cursor {
                    query = query.filter(
                        world_attacks::created_at
                            .lt(at)
                            .or(world_attacks::created_at
                                .eq(at)
                                .and(world_attacks::id.lt(id))),
                    );
                }
            }
            let records = query
                .order((world_attacks::created_at.desc(), world_attacks::id.desc()))
                .limit(SCENE_ATTACKS_PAGE)
                .select(AttackRecord::as_select())
                .load::<AttackRecord>(&mut conn)
                .map_err(|_| Error::new("Failed to load attacks"))?;
            let mut sights = Sights::new(&systems_dir, user_id, is_admin);
            build_attacks(&mut conn, &mut sights, records)
                .map_err(|e| Error::new(format!("Failed to read attacks: {e}")))
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
    }

    /// The caller's pending offers, for a reconnect (FR-008). A Game Master
    /// receives every pending offer in the world.
    async fn pending_offers(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
    ) -> GraphQLResult<Vec<GraphQLOffer>> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        let (user_id, is_admin) = (user.user_id, user.is_admin);
        let systems_dir = state.directories.systems_dir.clone();
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;
        tokio::task::spawn_blocking(move || {
            member_or_refuse(&mut conn, user_id, is_admin, world_id)?;
            let offers =
                crate::combat::offers::pending_offers(&mut conn, user_id, is_admin, world_id)
                    .map_err(|_| Error::new("Failed to load offers"))?;
            let mut sights = Sights::new(&systems_dir, user_id, is_admin);
            build_offers(&mut conn, &mut sights, offers)
                .map_err(|e| Error::new(format!("Failed to read offers: {e}")))
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
    }

    /// What `makeAttack` would record, and whether the turn would refuse it.
    /// A warning before rolling (FR-033): it refuses nothing and writes
    /// nothing.
    async fn preview_attack(
        &self,
        ctx: &Context<'_>,
        input: AttackInput,
    ) -> GraphQLResult<GraphQLAttackPreview> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        let (user_id, is_admin) = (user.user_id, user.is_admin);
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;
        let preview = tokio::task::spawn_blocking(move || {
            crate::combat::attack::preview_attack(
                &mut conn,
                user_id,
                is_admin,
                &input.into_request(),
            )
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))??;
        Ok(GraphQLAttackPreview {
            distance: preview.distance,
            flags: preview
                .flags
                .iter()
                .filter_map(|f| AttackFlag::from_db_str(f))
                .collect(),
            turn: GraphQLTurnCheck {
                allowed: preview.turn.allowed,
                active_label: preview.turn.active_label,
            },
        })
    }
}
