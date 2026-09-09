//! The play-field claim, as the client sees it (spec 036 US3a).
//!
//! # Subscribing is claiming
//!
//! `contracts/play-field-claim.md` specified a `claimPlayField` mutation
//! beside a `playFieldClaimChanged` subscription, and building it that way
//! does not work. FR-030 requires a claim never to outlive the client holding
//! it, which the registry achieves by handing back a guard — and a mutation
//! has nowhere to put one. It would have to be stored, which means a
//! heartbeat and a reaper to decide when the storing client is gone, which is
//! the exact failure mode the registry exists to avoid.
//!
//! So the subscription *is* the claim, the way `peerSignals` registration is
//! itself the grant of reachability. Opening the stream takes the table;
//! dropping it — navigating away, closing the window, losing the socket —
//! releases it, with no timeout and nothing to reap. The stream then carries
//! every subsequent change, so a client that is displaced learns it has become
//! a companion on the same channel that made it the holder.
//!
//! There is no `releasePlayField`: closing the stream is the release, and a
//! mutation that could release somebody else's claim would be a way to push a
//! person off their own table.

use async_graphql::{Context, Error, Object, Result as GraphQLResult};
use futures_util::{Stream, StreamExt};

use crate::graphql::helpers::{app_state, authenticated_user};
use crate::play_field::{Claim, registry};

/// Who holds an account's play field.
#[derive(async_graphql::SimpleObject, Clone)]
pub struct GraphQLPlayFieldClaim {
    /// The page-load id currently at the table for this account.
    pub client_id: String,
    pub world_id: uuid::Uuid,
    pub claimed_at: String,
    /// True when the client asking is the one holding it.
    pub is_mine: bool,
}

impl GraphQLPlayFieldClaim {
    fn of(claim: &Claim, asking_client_id: &str) -> Self {
        Self {
            client_id: claim.client_id.clone(),
            world_id: claim.world_id,
            claimed_at: claim.claimed_at.to_string(),
            is_mine: claim.client_id == asking_client_id,
        }
    }
}

#[derive(Default)]
pub struct PlayFieldQuery;

#[Object]
impl PlayFieldQuery {
    /// Who currently holds the calling account's play field, if anybody.
    ///
    /// A companion surface asks this to know whether offering "take the table
    /// back" would take it from another window or simply claim an empty one.
    async fn play_field_claim(
        &self,
        ctx: &Context<'_>,
        client_id: String,
    ) -> GraphQLResult<Option<GraphQLPlayFieldClaim>> {
        let auth_user = authenticated_user(ctx)?;
        Ok(registry()
            .current(auth_user.user_id)
            .map(|claim| GraphQLPlayFieldClaim::of(&claim, &client_id)))
    }
}

/// Take the play field and hold it for as long as this stream is open.
///
/// Yields the claim as it stands after taking it, and again every time it
/// moves — including `null` when nobody holds it. Authorised exactly as any
/// other world subscription is: a claim is a capability over a world, and any
/// failure to confirm membership refuses, because a long-lived grant handed
/// out on an unconfirmed check is the wrong direction to be wrong in.
pub async fn play_field_stream(
    ctx: &Context<'_>,
    world_id: uuid::Uuid,
    client_id: String,
) -> std::pin::Pin<Box<dyn Stream<Item = Result<Option<GraphQLPlayFieldClaim>, Error>> + Send>> {
    let failure = |msg: &str| {
        Box::pin(tokio_stream::iter(vec![Err(Error::new(msg.to_string()))]).boxed())
            as std::pin::Pin<
                Box<dyn Stream<Item = Result<Option<GraphQLPlayFieldClaim>, Error>> + Send>,
            >
    };

    let Ok(state) = app_state(ctx) else {
        return failure("Application state unavailable");
    };
    let Ok(auth_user) = authenticated_user(ctx) else {
        return failure("Authentication required");
    };
    if client_id.trim().is_empty() || client_id.len() > 128 {
        return failure("client id must be 1..=128 characters");
    }
    // The same gate every other world subscription uses. Any failure to
    // confirm refuses: a long-lived grant handed out on an unconfirmed check
    // is the wrong direction to be wrong in.
    let user_id = auth_user.user_id;
    let Ok(mut conn) = state.db_pool.get() else {
        return failure("You must be a member of this world");
    };
    let membership = tokio::task::spawn_blocking(move || {
        crate::auth::world_membership::require_world_member(&mut conn, user_id, world_id)
    })
    .await;
    if !matches!(membership, Ok(Ok(_))) {
        return failure("You must be a member of this world");
    }

    // Watch before claiming, so the client cannot miss a takeover that
    // happens between the two.
    let changes = registry().watch(auth_user.user_id);
    let (claim, guard) = registry().claim(auth_user.user_id, client_id.clone(), world_id);

    let first = GraphQLPlayFieldClaim::of(&claim, &client_id);
    let asking = client_id;
    // FR-010. Ending the stream drops `guard`, and dropping the guard
    // releases the claim — so the same wrapper that stops a revoked client
    // receiving events also stops it holding the play field, which is the
    // half of T036 that would otherwise need its own mechanism.
    let session_id = auth_user.session_id;
    let state = state.clone();

    Box::pin(crate::graphql::session_lifetime::until_session_ends(
        state,
        session_id,
        tokio_stream::iter(vec![Ok(Some(first))]).chain(futures_util::stream::unfold(
            (changes, guard, asking),
            |(mut changes, guard, asking)| async move {
                loop {
                    match changes.recv().await {
                        Ok(next) => {
                            let mapped = next
                                .as_ref()
                                .map(|claim| GraphQLPlayFieldClaim::of(claim, &asking));
                            return Some((Ok(mapped), (changes, guard, asking)));
                        }
                        // Lagged: this client fell behind its own claim moving,
                        // which only matters in that it must not silently keep
                        // the stale answer. The next message is current, so
                        // waiting for it is the recovery.
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => return None,
                    }
                }
            },
        )),
    ))
}

#[cfg(test)]
mod tests {
    /// The surface exists under the names the client uses, and — the part
    /// worth guarding — there is still no mutation that could release
    /// somebody else's claim.
    #[test]
    fn the_play_field_surface_is_registered_and_has_no_release_mutation() {
        let schema = async_graphql::Schema::build(
            crate::graphql::QueryRoot::default(),
            crate::graphql::MutationRoot::default(),
            crate::graphql::SubscriptionRoot,
        )
        .finish();
        let sdl = schema.sdl();

        assert!(sdl.contains("playFieldClaim(clientId: String!"));
        assert!(sdl.contains("playField(worldId: UUID!, clientId: String!"));

        // Closing the stream is the release. A mutation would be a way to
        // push a person off their own table from another window.
        for forbidden in ["\n\treleasePlayField", "\n\tclaimPlayField"] {
            assert!(
                !sdl.contains(forbidden),
                "`{forbidden}` would make the claim outlive, or be taken from, the client holding it"
            );
        }
    }
}
