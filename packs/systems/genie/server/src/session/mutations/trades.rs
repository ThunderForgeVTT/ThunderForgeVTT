//! Two-party resource trades, and the one-party spend that needs no
//! counterpart.

use super::*;

// ============================================================================
// proposeResourceTrade / acceptResourceTrade (T034) — two-party consent
// ============================================================================

#[allow(clippy::too_many_arguments)]
pub async fn propose_resource_trade_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    session_id: Uuid,
    from_actor_id: Uuid,
    from_resource_type: String,
    from_quantity: i32,
    to_actor_id: Uuid,
    to_resource_type: String,
    to_quantity: i32,
) -> GraphQLResult<GraphQLGenieTradeProposal> {
    if from_actor_id == to_actor_id {
        return Err(Error::new("Cannot propose a trade with yourself"));
    }
    if from_quantity <= 0 || to_quantity <= 0 {
        return Err(Error::new("Trade quantities must be greater than zero"));
    }

    require_member_of_session_world(state, user_id, session_id).await?;

    // Callable by either named party (research.md R8).
    let controls_from = caller_controls_actor(state, user_id, is_admin, from_actor_id).await?;
    let controls_to = caller_controls_actor(state, user_id, is_admin, to_actor_id).await?;
    if !controls_from && !controls_to {
        return Err(Error::new(
            "You must control one of the two parties to propose this trade",
        ));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let world_id = require_member_of_session_world(state, user_id, session_id).await?;

    let proposal = tokio::task::spawn_blocking(move || -> Result<GenieTradeProposal, String> {
        let new_proposal = NewGenieTradeProposal {
            session_id,
            from_actor_id,
            from_resource_type: from_resource_type.clone(),
            from_quantity,
            to_actor_id,
            to_resource_type: to_resource_type.clone(),
            to_quantity,
            created_by: user_id,
        };
        let proposal = diesel::insert_into(world_genie_trade_proposals::table)
            .values(&new_proposal)
            .returning(GenieTradeProposal::as_returning())
            .get_result::<GenieTradeProposal>(&mut conn)
            .map_err(|e| format!("Failed to propose trade: {e}"))?;

        let _ = record_world_event(
            &mut conn,
            world_id,
            EVENT_CODE_GENIE_SESSION_STATE,
            Some(serde_json::json!({
                "kind": "resource_trade",
                "session_id": session_id,
                "action": "proposed",
                "proposal_id": proposal.id,
                "from_actor_id": from_actor_id,
                "to_actor_id": to_actor_id,
            })),
            user_id,
        );

        Ok(proposal)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)?;

    Ok(GraphQLGenieTradeProposal::from(proposal))
}

pub async fn accept_resource_trade_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    proposal_id: Uuid,
) -> GraphQLResult<Vec<GraphQLGenieResourceHolding>> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let proposal = tokio::task::spawn_blocking(move || -> Result<GenieTradeProposal, String> {
        world_genie_trade_proposals::table
            .filter(world_genie_trade_proposals::id.eq(proposal_id))
            .select(GenieTradeProposal::as_select())
            .first::<GenieTradeProposal>(&mut conn)
            .map_err(|_| "Trade proposal not found".to_string())
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)?;

    if proposal.status != "pending" {
        return Err(Error::new("This trade proposal is no longer pending"));
    }
    // The proposer can never accept their own proposal (research.md R8).
    if proposal.created_by == user_id {
        return Err(Error::new("You cannot accept your own trade proposal"));
    }
    // Caller must control the counterpart actor — either side, since
    // `proposeResourceTrade` doesn't record which side literally clicked
    // "propose", only who authored it (`created_by`); requiring the
    // accepter to control one of the two named actors (and not be the
    // proposer, checked above) enforces "the OTHER party" from
    // contracts/genie-session-loop.md.
    let controls_from =
        caller_controls_actor(state, user_id, is_admin, proposal.from_actor_id).await?;
    let controls_to = caller_controls_actor(state, user_id, is_admin, proposal.to_actor_id).await?;
    if !controls_from && !controls_to {
        return Err(Error::new("You are not a party to this trade"));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let holdings =
        tokio::task::spawn_blocking(move || -> Result<Vec<GenieResourceHolding>, String> {
            conn.transaction(|conn| -> Result<Vec<GenieResourceHolding>, TxError> {
                let from_current = load_holding_quantity(
                    conn,
                    proposal.session_id,
                    proposal.from_actor_id,
                    &proposal.from_resource_type,
                )
                .map_err(TxError::Msg)?;
                if from_current < proposal.from_quantity {
                    return Err(TxError::Msg(
                        "The proposing party no longer holds enough to complete this trade"
                            .to_string(),
                    ));
                }
                let to_current = load_holding_quantity(
                    conn,
                    proposal.session_id,
                    proposal.to_actor_id,
                    &proposal.to_resource_type,
                )
                .map_err(TxError::Msg)?;
                if to_current < proposal.to_quantity {
                    return Err(TxError::Msg(
                        "The counterpart no longer holds enough to complete this trade".to_string(),
                    ));
                }

                let from_after = set_holding_quantity(
                    conn,
                    proposal.session_id,
                    proposal.from_actor_id,
                    &proposal.from_resource_type,
                    from_current - proposal.from_quantity,
                )
                .map_err(TxError::Msg)?;
                let from_gains_base = load_holding_quantity(
                    conn,
                    proposal.session_id,
                    proposal.from_actor_id,
                    &proposal.to_resource_type,
                )
                .map_err(TxError::Msg)?;
                let from_gains = set_holding_quantity(
                    conn,
                    proposal.session_id,
                    proposal.from_actor_id,
                    &proposal.to_resource_type,
                    from_gains_base + proposal.to_quantity,
                )
                .map_err(TxError::Msg)?;

                let to_after = set_holding_quantity(
                    conn,
                    proposal.session_id,
                    proposal.to_actor_id,
                    &proposal.to_resource_type,
                    to_current - proposal.to_quantity,
                )
                .map_err(TxError::Msg)?;
                let to_gains_base = load_holding_quantity(
                    conn,
                    proposal.session_id,
                    proposal.to_actor_id,
                    &proposal.from_resource_type,
                )
                .map_err(TxError::Msg)?;
                let to_gains = set_holding_quantity(
                    conn,
                    proposal.session_id,
                    proposal.to_actor_id,
                    &proposal.from_resource_type,
                    to_gains_base + proposal.from_quantity,
                )
                .map_err(TxError::Msg)?;

                diesel::update(
                    world_genie_trade_proposals::table
                        .filter(world_genie_trade_proposals::id.eq(proposal_id)),
                )
                .set((
                    world_genie_trade_proposals::status.eq("accepted"),
                    world_genie_trade_proposals::updated_at.eq(Utc::now().naive_utc()),
                ))
                .execute(conn)?;

                let trade_world_id = world_genie_sessions::table
                    .filter(world_genie_sessions::id.eq(proposal.session_id))
                    .select(world_genie_sessions::world_id)
                    .first::<Uuid>(conn)?;

                let _ = record_world_event(
                    conn,
                    trade_world_id,
                    EVENT_CODE_GENIE_SESSION_STATE,
                    Some(serde_json::json!({
                        "kind": "resource_trade",
                        "session_id": proposal.session_id,
                        "action": "accepted",
                        "proposal_id": proposal.id,
                        "from_actor_id": proposal.from_actor_id,
                        "to_actor_id": proposal.to_actor_id,
                    })),
                    user_id,
                );

                Ok(vec![from_after, from_gains, to_after, to_gains])
            })
            .map_err(String::from)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(Error::new)?;

    // Return just the final holdings for the two parties (one row per
    // resource type touched — a party's own resource_type row and the
    // counterpart's resource_type row it now also holds).
    Ok(holdings
        .into_iter()
        .map(GraphQLGenieResourceHolding::from)
        .collect())
}

/// Spec 019: the counterpart declines a still-pending proposal — no
/// holdings change, just a status flip to `"rejected"` (the table's
/// CHECK constraint already allowed this value; it was simply never set
/// by any mutation) — mirrors accept_resource_trade_impl's authorization
/// exactly: not the proposer, must control one of the two named actors —
/// so the proposer isn't left waiting on a trade nobody will ever accept.
pub async fn decline_resource_trade_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    proposal_id: Uuid,
) -> GraphQLResult<GraphQLGenieTradeProposal> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let proposal = tokio::task::spawn_blocking(move || -> Result<GenieTradeProposal, String> {
        world_genie_trade_proposals::table
            .filter(world_genie_trade_proposals::id.eq(proposal_id))
            .select(GenieTradeProposal::as_select())
            .first::<GenieTradeProposal>(&mut conn)
            .map_err(|_| "Trade proposal not found".to_string())
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)?;

    if proposal.status != "pending" {
        return Err(Error::new("This trade proposal is no longer pending"));
    }
    if proposal.created_by == user_id {
        return Err(Error::new("You cannot decline your own trade proposal"));
    }
    let controls_from =
        caller_controls_actor(state, user_id, is_admin, proposal.from_actor_id).await?;
    let controls_to = caller_controls_actor(state, user_id, is_admin, proposal.to_actor_id).await?;
    if !controls_from && !controls_to {
        return Err(Error::new("You are not a party to this trade"));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let updated = tokio::task::spawn_blocking(move || -> Result<GenieTradeProposal, String> {
        conn.transaction(
            |conn| -> Result<GenieTradeProposal, diesel::result::Error> {
                let now = Utc::now().naive_utc();
                let updated = diesel::update(
                    world_genie_trade_proposals::table
                        .filter(world_genie_trade_proposals::id.eq(proposal_id)),
                )
                .set((
                    world_genie_trade_proposals::status.eq("rejected"),
                    world_genie_trade_proposals::updated_at.eq(now),
                ))
                .returning(GenieTradeProposal::as_returning())
                .get_result::<GenieTradeProposal>(conn)?;

                let trade_world_id = world_genie_sessions::table
                    .filter(world_genie_sessions::id.eq(updated.session_id))
                    .select(world_genie_sessions::world_id)
                    .first::<Uuid>(conn)?;

                let _ = record_world_event(
                    conn,
                    trade_world_id,
                    EVENT_CODE_GENIE_SESSION_STATE,
                    Some(serde_json::json!({
                        "kind": "resource_trade",
                        "session_id": updated.session_id,
                        "action": "rejected",
                        "proposal_id": updated.id,
                        "from_actor_id": updated.from_actor_id,
                        "to_actor_id": updated.to_actor_id,
                    })),
                    user_id,
                );

                Ok(updated)
            },
        )
        .map_err(|e| format!("Failed to decline trade: {e}"))
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)?;

    Ok(GraphQLGenieTradeProposal::from(updated))
}

// ============================================================================
// spendResourceOnPuzzleClock (T035) — self-spend, no counterpart consent
// ============================================================================

pub async fn spend_resource_on_puzzle_clock_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    clock_id: Uuid,
    actor_id: Uuid,
    resource_type: String,
    quantity: i32,
) -> GraphQLResult<GraphQLGeniePuzzleClock> {
    if quantity <= 0 {
        return Err(Error::new("quantity must be greater than zero"));
    }
    require_caller_controls_actor(state, user_id, is_admin, actor_id).await?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let (clock, session) = tokio::task::spawn_blocking(
        move || -> Result<(GeniePuzzleClock, GenieSession), String> {
            let clock = load_puzzle_clock_row(&mut conn, clock_id)?;
            let session = load_session_row(&mut conn, clock.session_id)?;
            Ok((clock, session))
        },
    )
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)?;

    if session.status != "active" {
        return Err(Error::new("This Genie session has already concluded"));
    }
    if clock.resolved_at.is_some() {
        return Err(Error::new("This Puzzle Clock has already resolved"));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let world_id = session.world_id;
    let session_id = session.id;

    let updated_clock = tokio::task::spawn_blocking(move || -> Result<GeniePuzzleClock, String> {
        conn.transaction(|conn| -> Result<GeniePuzzleClock, TxError> {
            let current_qty = load_holding_quantity(conn, session_id, actor_id, &resource_type)
                .map_err(TxError::Msg)?;
            if current_qty < quantity {
                return Err(TxError::Msg(
                    "You do not hold enough of this resource".to_string(),
                ));
            }
            set_holding_quantity(
                conn,
                session_id,
                actor_id,
                &resource_type,
                current_qty - quantity,
            )
            .map_err(TxError::Msg)?;

            let now = Utc::now().naive_utc();
            // One resource spent advances the clock by one segment
            // (data-model.md/contracts/genie-session-loop.md leave the
            // exact resource-to-segment ratio to game-balance content
            // decisions; 1:1 is the simplest structurally-valid choice).
            let new_segments = (clock.segments_current + quantity).clamp(0, clock.segments_max);
            let resolves_now = new_segments >= clock.segments_max;
            let resolved_at = if resolves_now { Some(now) } else { None };

            let updated_clock = diesel::update(
                world_genie_puzzle_clocks::table.filter(world_genie_puzzle_clocks::id.eq(clock_id)),
            )
            .set((
                world_genie_puzzle_clocks::segments_current.eq(new_segments),
                world_genie_puzzle_clocks::resolved_at.eq(resolved_at),
                world_genie_puzzle_clocks::updated_at.eq(now),
            ))
            .returning(GeniePuzzleClock::as_returning())
            .get_result::<GeniePuzzleClock>(conn)?;

            let mut session_status_for_event = session.status.clone();
            if resolves_now {
                let all_clocks = world_genie_puzzle_clocks::table
                    .filter(world_genie_puzzle_clocks::session_id.eq(session_id))
                    .select(GeniePuzzleClock::as_select())
                    .load::<GeniePuzzleClock>(conn)?;
                if all_puzzle_clocks_resolved(&all_clocks) {
                    diesel::update(
                        world_genie_sessions::table.filter(world_genie_sessions::id.eq(session_id)),
                    )
                    .set((
                        world_genie_sessions::status.eq("won"),
                        world_genie_sessions::updated_at.eq(now),
                    ))
                    .execute(conn)?;
                    session_status_for_event = "won".to_string();
                }
            }

            let _ = record_world_event(
                conn,
                world_id,
                EVENT_CODE_GENIE_SESSION_STATE,
                Some(serde_json::json!({
                    "kind": "puzzle_clock",
                    "session_id": session_id,
                    "clock_id": updated_clock.id,
                    "segments_current": updated_clock.segments_current,
                    "segments_max": updated_clock.segments_max,
                    "resolved": updated_clock.resolved_at.is_some(),
                    "session_status": session_status_for_event,
                    "spent_by_actor_id": actor_id,
                    "spent_quantity": quantity,
                })),
                user_id,
            );

            Ok(updated_clock)
        })
        .map_err(String::from)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)?;

    Ok(GraphQLGeniePuzzleClock::from(updated_clock))
}
