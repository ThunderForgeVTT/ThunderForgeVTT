//! Spec 035 / ADR-072: the administrator's view of, and control over, instance
//! admission — the policy, its audit trail, and instance invitations.
//!
//! # No lookup by code
//!
//! There is deliberately no query here that resolves an invitation by its
//! code. The only thing that may is redemption, and it consumes a use doing
//! so. This mirrors the no-enumeration invariant ADR-049's determination
//! depends on for share links: a code is unguessable, and nothing exists that
//! would let it be confirmed without spending it.

use async_graphql::{Context, Enum, Error, InputObject, Result as GraphQLResult, SimpleObject};
use diesel::prelude::*;
use uuid::Uuid;

use crate::auth::instance_access::InstanceAccessPolicy;
use crate::graphql::share_codes::generate_link_code;
use crate::graphql::{admin_user, app_state};
use crate::models::{InstanceInvitation, NewInstanceInvitation};
use crate::schema::{instance_invitation_redemptions, instance_invitations, users};

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "InstanceAccessPolicy")]
pub enum GraphQLInstanceAccessPolicy {
    Open,
    InviteOnly,
    Closed,
}

impl From<GraphQLInstanceAccessPolicy> for InstanceAccessPolicy {
    fn from(v: GraphQLInstanceAccessPolicy) -> Self {
        match v {
            GraphQLInstanceAccessPolicy::Open => InstanceAccessPolicy::Open,
            GraphQLInstanceAccessPolicy::InviteOnly => InstanceAccessPolicy::InviteOnly,
            GraphQLInstanceAccessPolicy::Closed => InstanceAccessPolicy::Closed,
        }
    }
}

impl From<InstanceAccessPolicy> for GraphQLInstanceAccessPolicy {
    fn from(v: InstanceAccessPolicy) -> Self {
        match v {
            InstanceAccessPolicy::Open => GraphQLInstanceAccessPolicy::Open,
            InstanceAccessPolicy::InviteOnly => GraphQLInstanceAccessPolicy::InviteOnly,
            InstanceAccessPolicy::Closed => GraphQLInstanceAccessPolicy::Closed,
        }
    }
}

#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "InstanceAccessSettings")]
pub struct GraphQLInstanceAccessSettings {
    pub policy: GraphQLInstanceAccessPolicy,
    pub updated_at: String,
    pub updated_by: Option<Uuid>,
}

#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "InstanceAccessEvent")]
pub struct GraphQLInstanceAccessEvent {
    pub id: Uuid,
    pub event_type: String,
    pub occurred_at: String,
    pub actor_user_id: Option<Uuid>,
    pub previous_policy: Option<String>,
    pub new_policy: Option<String>,
    pub attempted_route: Option<String>,
    pub policy_at_attempt: Option<String>,
}

#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "InstanceInvitationRedemption")]
pub struct GraphQLInstanceInvitationRedemption {
    pub user_id: Uuid,
    pub username: String,
    pub redeemed_at: String,
    pub route: String,
}

/// An invitation as the operator sees it.
///
/// `invite_code` appears **only here**, on an admin-only query. It is not in
/// the public status response, not in an access event, and not in a log line
/// (FR-019).
#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "InstanceInvitation")]
pub struct GraphQLInstanceInvitation {
    pub id: Uuid,
    pub invite_code: String,
    pub max_uses: i32,
    pub used_count: i32,
    pub remaining_uses: i32,
    pub state: String,
    pub expires_at: Option<String>,
    pub note: Option<String>,
    pub created_by: Uuid,
    pub created_at: String,
    pub redemptions: Vec<GraphQLInstanceInvitationRedemption>,
}

#[derive(InputObject, Debug, Clone)]
pub struct CreateInstanceInvitationInput {
    pub max_uses: Option<i32>,
    pub expires_in_hours: Option<i32>,
    pub note: Option<String>,
}

/// Derived, never stored — the same shape `derive_link_state` gives a world
/// invite. **Only the operator sees this.** To a redeemer, every non-active
/// state is one refusal (FR-011).
fn derive_state(row: &InstanceInvitation) -> &'static str {
    if row.revoked {
        "REVOKED"
    } else if row
        .expires_at
        .is_some_and(|e| e <= chrono::Utc::now().naive_utc())
    {
        "EXPIRED"
    } else if row.used_count >= row.max_uses {
        "EXHAUSTED"
    } else {
        "ACTIVE"
    }
}

pub async fn create_instance_invitation_impl(
    state: &crate::state::AppState,
    user_id: Uuid,
    input: CreateInstanceInvitationInput,
) -> GraphQLResult<InstanceInvitation> {
    let max_uses = input.max_uses.unwrap_or(1);
    if max_uses < 1 {
        return Err(Error::new("An invitation must allow at least one use"));
    }
    let expires_at = input
        .expires_in_hours
        .filter(|h| *h > 0)
        .map(|h| chrono::Utc::now().naive_utc() + chrono::Duration::hours(h as i64));

    let row = NewInstanceInvitation {
        id: Uuid::now_v7(),
        // v4-derived, never v7: a v7 code leaks its creation time and narrows
        // a guess (ADR-049, FR-019).
        invite_code: generate_link_code(),
        max_uses,
        expires_at,
        note: input.note,
        created_by: user_id,
    };

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        diesel::insert_into(instance_invitations::table)
            .values(&row)
            .returning(InstanceInvitation::as_returning())
            .get_result::<InstanceInvitation>(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|e| Error::new(format!("Failed to create the invitation: {e}")))
}

pub async fn revoke_instance_invitation_impl(
    state: &crate::state::AppState,
    invitation_id: Uuid,
) -> GraphQLResult<bool> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let affected = tokio::task::spawn_blocking(move || {
        diesel::update(
            instance_invitations::table.filter(instance_invitations::id.eq(invitation_id)),
        )
        .set((
            instance_invitations::revoked.eq(true),
            instance_invitations::updated_at.eq(chrono::Utc::now().naive_utc()),
        ))
        .execute(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to revoke the invitation"))?;

    Ok(affected > 0)
}

pub async fn list_instance_invitations_impl(
    state: &crate::state::AppState,
) -> GraphQLResult<Vec<GraphQLInstanceInvitation>> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let rows = tokio::task::spawn_blocking(move || {
        let invitations = instance_invitations::table
            .order(instance_invitations::created_at.desc())
            .select(InstanceInvitation::as_select())
            .load::<InstanceInvitation>(&mut conn)?;

        let redemptions = instance_invitation_redemptions::table
            .inner_join(users::table.on(users::id.eq(instance_invitation_redemptions::user_id)))
            .select((
                instance_invitation_redemptions::invitation_id,
                instance_invitation_redemptions::user_id,
                users::username,
                instance_invitation_redemptions::redeemed_at,
                instance_invitation_redemptions::route,
            ))
            .load::<(Uuid, Uuid, String, chrono::NaiveDateTime, String)>(&mut conn)?;

        Ok::<_, diesel::result::Error>((invitations, redemptions))
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to load invitations"))?;

    let (invitations, redemptions) = rows;
    Ok(invitations
        .into_iter()
        .map(|row| {
            let state_label = derive_state(&row).to_string();
            let mine = redemptions
                .iter()
                .filter(|(invitation_id, ..)| *invitation_id == row.id)
                .map(|(_, user_id, username, redeemed_at, route)| {
                    GraphQLInstanceInvitationRedemption {
                        user_id: *user_id,
                        username: username.clone(),
                        redeemed_at: redeemed_at.to_string(),
                        route: route.clone(),
                    }
                })
                .collect();
            GraphQLInstanceInvitation {
                id: row.id,
                invite_code: row.invite_code,
                max_uses: row.max_uses,
                used_count: row.used_count,
                remaining_uses: (row.max_uses - row.used_count).max(0),
                state: state_label,
                expires_at: row.expires_at.map(|e| e.to_string()),
                note: row.note,
                created_by: row.created_by,
                created_at: row.created_at.to_string(),
                redemptions: mine,
            }
        })
        .collect())
}

#[derive(Default)]
pub struct InstanceAccessQuery;

#[async_graphql::Object]
impl InstanceAccessQuery {
    /// Admin-only. The unauthenticated surface reads the policy from
    /// `/authentication/setup/status` instead (FR-003).
    async fn instance_access_settings(
        &self,
        ctx: &Context<'_>,
    ) -> GraphQLResult<GraphQLInstanceAccessSettings> {
        let state = app_state(ctx)?;
        let _ = admin_user(ctx)?;
        let row = crate::admin::load_instance_access_settings(state)
            .await
            .map_err(Error::new)?;
        Ok(GraphQLInstanceAccessSettings {
            policy: InstanceAccessPolicy::from_db_str(&row.access_policy).into(),
            updated_at: row.updated_at.to_string(),
            updated_by: row.updated_by,
        })
    }

    /// Admin-only. FR-004 and FR-012's record, newest first.
    async fn instance_access_events(
        &self,
        ctx: &Context<'_>,
        limit: Option<i32>,
    ) -> GraphQLResult<Vec<GraphQLInstanceAccessEvent>> {
        let state = app_state(ctx)?;
        let _ = admin_user(ctx)?;
        let limit = limit.unwrap_or(50).clamp(1, 500) as i64;

        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        let rows = tokio::task::spawn_blocking(move || {
            crate::schema::instance_access_events::table
                .order(crate::schema::instance_access_events::occurred_at.desc())
                .limit(limit)
                .select(crate::models::InstanceAccessEvent::as_select())
                .load::<crate::models::InstanceAccessEvent>(&mut conn)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to load access events"))?;

        Ok(rows
            .into_iter()
            .map(|r| GraphQLInstanceAccessEvent {
                id: r.id,
                event_type: r.event_type,
                occurred_at: r.occurred_at.to_string(),
                actor_user_id: r.actor_user_id,
                previous_policy: r.previous_policy,
                new_policy: r.new_policy,
                attempted_route: r.attempted_route,
                policy_at_attempt: r.policy_at_attempt,
            })
            .collect())
    }

    /// Admin-only. Note there is no `instanceInvitation(code:)` counterpart,
    /// and none may be added — see this module's header.
    async fn instance_invitations(
        &self,
        ctx: &Context<'_>,
    ) -> GraphQLResult<Vec<GraphQLInstanceInvitation>> {
        let state = app_state(ctx)?;
        let _ = admin_user(ctx)?;
        list_instance_invitations_impl(state).await
    }
}

#[derive(Default)]
pub struct InstanceAccessMutation;

#[async_graphql::Object]
impl InstanceAccessMutation {
    /// FR-002: takes effect for every subsequent admission attempt, with no
    /// restart — the gate reads the row per attempt.
    async fn set_instance_access_policy(
        &self,
        ctx: &Context<'_>,
        policy: GraphQLInstanceAccessPolicy,
    ) -> GraphQLResult<GraphQLInstanceAccessSettings> {
        let state = app_state(ctx)?;
        let admin = admin_user(ctx)?;
        let row = crate::admin::update_instance_access_policy(state, admin.user_id, policy.into())
            .await
            .map_err(Error::new)?;
        Ok(GraphQLInstanceAccessSettings {
            policy: InstanceAccessPolicy::from_db_str(&row.access_policy).into(),
            updated_at: row.updated_at.to_string(),
            updated_by: row.updated_by,
        })
    }

    async fn create_instance_invitation(
        &self,
        ctx: &Context<'_>,
        input: CreateInstanceInvitationInput,
    ) -> GraphQLResult<GraphQLInstanceInvitation> {
        let state = app_state(ctx)?;
        let admin = admin_user(ctx)?;
        let row = create_instance_invitation_impl(state, admin.user_id, input).await?;
        Ok(GraphQLInstanceInvitation {
            id: row.id,
            invite_code: row.invite_code,
            max_uses: row.max_uses,
            used_count: row.used_count,
            remaining_uses: row.max_uses - row.used_count,
            state: "ACTIVE".to_string(),
            expires_at: row.expires_at.map(|e| e.to_string()),
            note: row.note,
            created_by: row.created_by,
            created_at: row.created_at.to_string(),
            redemptions: Vec::new(),
        })
    }

    async fn revoke_instance_invitation(
        &self,
        ctx: &Context<'_>,
        invitation_id: Uuid,
    ) -> GraphQLResult<bool> {
        let state = app_state(ctx)?;
        let _ = admin_user(ctx)?;
        revoke_instance_invitation_impl(state, invitation_id).await
    }
}

#[cfg(test)]
mod tests {
    /// A mutation that compiles but was never merged into the root fails for
    /// the first operator who tries to close their instance, not for the
    /// suite. `mutations_lore_tree.rs` and `mutations_party.rs` keep the same
    /// guard for the same reason.
    #[test]
    fn the_access_surface_is_registered_under_the_names_the_client_uses() {
        let schema = async_graphql::Schema::build(
            crate::graphql::QueryRoot::default(),
            crate::graphql::MutationRoot::default(),
            crate::graphql::SubscriptionRoot,
        )
        .finish();
        let sdl = schema.sdl();

        for field in [
            "instanceAccessSettings",
            "instanceAccessEvents(",
            "instanceInvitations",
            "setInstanceAccessPolicy(policy: InstanceAccessPolicy!",
            "createInstanceInvitation(input: CreateInstanceInvitationInput!",
            "revokeInstanceInvitation(invitationId: UUID!",
        ] {
            assert!(
                sdl.contains(field),
                "`{field}` must be reachable from the root"
            );
        }

        // FR-019: there is no lookup by code, and adding one would re-open the
        // no-enumeration invariant ADR-049's determination rests on.
        // Matched against a field *declaration* — SDL indents fields with a
        // tab — rather than anywhere in the text. A first draft searched the
        // whole SDL and caught the doc comment two fields up, which says in
        // prose that this query does not exist: async_graphql emits doc
        // comments as descriptions, so the schema contains the sentence
        // denying the thing as well as (not) the thing.
        assert!(
            !sdl.contains("\n\tinstanceInvitation(code"),
            "an invitation must never be resolvable by code outside redemption"
        );
    }
}
