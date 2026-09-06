//! Starting a session, and the three clocks that run inside it: the Wish
//! Pool, the Doom Clock, and Puzzle Clocks with their rewards.

use super::*;

// ============================================================================
// startGenieSession (see module doc comment — an addition beyond the
// contract, the missing prerequisite step for everything else here)
// ============================================================================

#[derive(InputObject, Debug, Clone)]
pub struct StartGenieSessionInput {
    pub world_id: Uuid,
    pub doom_clock_max: i32,
}

pub async fn start_genie_session_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    input: StartGenieSessionInput,
) -> GraphQLResult<GraphQLGenieSession> {
    if !is_dm_of_world(state, user_id, is_admin, input.world_id).await? {
        return Err(Error::new("Only the GM may start a Genie session"));
    }
    if input.doom_clock_max <= 0 {
        return Err(Error::new("doomClockMax must be greater than zero"));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let world_id = input.world_id;
    let doom_clock_max = input.doom_clock_max;

    let session = tokio::task::spawn_blocking(move || -> Result<GenieSession, String> {
        // At most one active session per world, enforced here because
        // nothing else enforces it and the rest of this module assumes it.
        //
        // `genieSession(worldId)` returns the *newest* active session
        // (`queries/genie_session.rs` — `order(created_at.desc()).first()`),
        // and creating a Genie world already starts one (`graphql.rs`,
        // `is_genie_world`). So an unguarded insert left a world holding two
        // live sessions, and concluding the newer one silently resurfaced the
        // older: the Doom Clock a GM had just filled was replaced on screen by
        // an untouched clock from a session nobody knew existed, with the
        // "Session lost" banner never appearing. The carryover query below
        // already assumes this invariant — it looks for the most recent
        // *non-active* session to copy holdings from, which only means
        // anything if a new session begins after the last one ended.
        let active_exists = diesel::select(diesel::dsl::exists(
            world_genie_sessions::table
                .filter(world_genie_sessions::world_id.eq(world_id))
                .filter(world_genie_sessions::status.eq("active")),
        ))
        .get_result::<bool>(&mut conn)
        .map_err(|e| format!("Failed to check for an active session: {e}"))?;
        if active_exists {
            return Err(
                "This world already has an active Genie session. Conclude it before \
                 starting another."
                    .to_string(),
            );
        }

        // FR-003 (research.md R1): per-world GM setting. When enabled,
        // copy every character's ending holdings from the most recently
        // concluded session (by created_at) into the new session before
        // returning — "the rope doesn't disappear." When disabled
        // (default), holdings simply start empty, same as before this
        // spec.
        let carryover_enabled = worlds::table
            .filter(worlds::id.eq(world_id))
            .select(worlds::genie_resource_carryover_enabled)
            .first::<bool>(&mut conn)
            .map_err(|e| format!("Failed to load world: {e}"))?;

        let prior_holdings: Vec<GenieResourceHolding> = if carryover_enabled {
            let prior_session_id = world_genie_sessions::table
                .filter(world_genie_sessions::world_id.eq(world_id))
                .filter(world_genie_sessions::status.ne("active"))
                .order(world_genie_sessions::created_at.desc())
                .select(world_genie_sessions::id)
                .first::<Uuid>(&mut conn)
                .optional()
                .map_err(|e| format!("Failed to load prior session: {e}"))?;

            match prior_session_id {
                Some(prior_id) => world_genie_resource_holdings::table
                    .filter(world_genie_resource_holdings::session_id.eq(prior_id))
                    .select(GenieResourceHolding::as_select())
                    .load::<GenieResourceHolding>(&mut conn)
                    .map_err(|e| format!("Failed to load prior holdings: {e}"))?,
                None => Vec::new(),
            }
        } else {
            Vec::new()
        };

        let new_session = NewGenieSession {
            world_id,
            doom_clock_max,
            created_by: user_id,
        };
        let session = diesel::insert_into(world_genie_sessions::table)
            .values(&new_session)
            .returning(GenieSession::as_returning())
            .get_result::<GenieSession>(&mut conn)
            .map_err(|e| format!("Failed to start session: {e}"))?;

        for holding in &prior_holdings {
            set_holding_quantity(
                &mut conn,
                session.id,
                holding.actor_id,
                &holding.resource_type,
                holding.quantity,
            )
            .map_err(|e| format!("Failed to carry over holding: {e}"))?;
        }

        let _ = record_world_event(
            &mut conn,
            world_id,
            EVENT_CODE_GENIE_SESSION_STATE,
            Some(serde_json::json!({
                "kind": "wish_pool",
                "session_id": session.id,
                "action": "session_started",
                "wishes_remaining": session.wishes_remaining,
                "carried_over_holdings": prior_holdings.len(),
            })),
            user_id,
        );

        Ok(session)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)?;

    Ok(build_graphql_session(session, Vec::new()))
}

// ============================================================================
// grantSessionResource (spec 020, FR-001) — GM-only direct grant, the
// bootstrapping fix: the only way a Session Resource holding could
// previously come to exist was a trade or a Puzzle Clock spend, both of
// which require holdings to already exist.
// ============================================================================

pub async fn grant_session_resource_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    session_id: Uuid,
    actor_id: Uuid,
    resource_type: String,
    amount: i32,
) -> GraphQLResult<GraphQLGenieResourceHolding> {
    if amount <= 0 {
        return Err(Error::new("amount must be greater than zero"));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let session = tokio::task::spawn_blocking(move || load_session_row(&mut conn, session_id))
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(Error::new)?;

    if !is_dm_of_world(state, user_id, is_admin, session.world_id).await? {
        return Err(Error::new("Only the GM may grant Session Resources"));
    }
    if session.status != "active" {
        return Err(Error::new(
            "Start a session first — there is no active Genie session for this world",
        ));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let world_id = session.world_id;

    let holding = tokio::task::spawn_blocking(move || -> Result<GenieResourceHolding, String> {
        conn.transaction(|conn| -> Result<GenieResourceHolding, TxError> {
            let current = load_holding_quantity(conn, session_id, actor_id, &resource_type)
                .map_err(TxError::Msg)?;
            let updated =
                set_holding_quantity(conn, session_id, actor_id, &resource_type, current + amount)
                    .map_err(TxError::Msg)?;

            let _ = record_world_event(
                conn,
                world_id,
                EVENT_CODE_GENIE_SESSION_STATE,
                Some(serde_json::json!({
                    "kind": "resource_grant",
                    "session_id": session_id,
                    "actor_id": actor_id,
                    "resource_type": updated.resource_type,
                    "quantity": updated.quantity,
                    "amount_granted": amount,
                })),
                user_id,
            );

            Ok(updated)
        })
        .map_err(String::from)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)?;

    Ok(GraphQLGenieResourceHolding::from(holding))
}

// ============================================================================
// spendWish (T032) — GM-only
// ============================================================================

pub async fn spend_wish_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    session_id: Uuid,
    narrative_effect: String,
) -> GraphQLResult<GraphQLGenieSession> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let session = tokio::task::spawn_blocking(move || load_session_row(&mut conn, session_id))
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(Error::new)?;

    if !is_dm_of_world(state, user_id, is_admin, session.world_id).await? {
        return Err(Error::new("Only the GM may spend a wish"));
    }
    if session.wishes_remaining <= 0 {
        return Err(Error::new("No wishes remaining in the Session Wish Pool"));
    }
    if narrative_effect.trim().is_empty() {
        return Err(Error::new("narrativeEffect must not be empty (FR-014)"));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let world_id = session.world_id;
    let new_wishes = session.wishes_remaining - 1;

    let (updated, clocks) = tokio::task::spawn_blocking(
        move || -> Result<(GenieSession, Vec<GeniePuzzleClock>), String> {
            let now = Utc::now().naive_utc();
            let updated = diesel::update(
                world_genie_sessions::table.filter(world_genie_sessions::id.eq(session_id)),
            )
            .set((
                world_genie_sessions::wishes_remaining.eq(new_wishes),
                world_genie_sessions::updated_at.eq(now),
            ))
            .returning(GenieSession::as_returning())
            .get_result::<GenieSession>(&mut conn)
            .map_err(|e| format!("Failed to spend wish: {e}"))?;

            let _ = record_world_event(
                &mut conn,
                world_id,
                EVENT_CODE_GENIE_SESSION_STATE,
                Some(serde_json::json!({
                    "kind": "wish_pool",
                    "session_id": session_id,
                    "action": "wish_spent",
                    "wishes_remaining": updated.wishes_remaining,
                    "narrative_effect": narrative_effect,
                })),
                user_id,
            );

            let clocks = load_puzzle_clocks_for_session(&mut conn, session_id)?;
            Ok((updated, clocks))
        },
    )
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)?;

    Ok(build_graphql_session(updated, clocks))
}

// ============================================================================
// advanceDoomClock (T032/T033) — GM-only; loss check per FR-016
// ============================================================================

pub async fn advance_doom_clock_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    session_id: Uuid,
    delta: i32,
) -> GraphQLResult<GraphQLGenieSession> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let session = tokio::task::spawn_blocking(move || load_session_row(&mut conn, session_id))
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(Error::new)?;

    if !is_dm_of_world(state, user_id, is_admin, session.world_id).await? {
        return Err(Error::new("Only the GM may advance the Doom Clock"));
    }
    if session.status != "active" {
        return Err(Error::new("This Genie session has already concluded"));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let world_id = session.world_id;
    let new_current = (session.doom_clock_current + delta).clamp(0, session.doom_clock_max);
    // FR-016: filling the Doom Clock completely, while the session is
    // still active (checked above — a same-action Puzzle Clock win takes
    // precedence per spec.md's Edge Cases, since that would have already
    // flipped status away from 'active' in its own prior mutation),
    // triggers the loss condition.
    let just_lost = new_current >= session.doom_clock_max;

    let (updated, clocks) = tokio::task::spawn_blocking(
        move || -> Result<(GenieSession, Vec<GeniePuzzleClock>), String> {
            let now = Utc::now().naive_utc();
            let new_status = if just_lost { "lost" } else { "active" };
            let updated = diesel::update(
                world_genie_sessions::table.filter(world_genie_sessions::id.eq(session_id)),
            )
            .set((
                world_genie_sessions::doom_clock_current.eq(new_current),
                world_genie_sessions::status.eq(new_status),
                world_genie_sessions::updated_at.eq(now),
            ))
            .returning(GenieSession::as_returning())
            .get_result::<GenieSession>(&mut conn)
            .map_err(|e| format!("Failed to advance Doom Clock: {e}"))?;

            let _ = record_world_event(
                &mut conn,
                world_id,
                EVENT_CODE_GENIE_SESSION_STATE,
                Some(serde_json::json!({
                    "kind": "doom_clock",
                    "session_id": session_id,
                    "doom_clock_current": updated.doom_clock_current,
                    "doom_clock_max": updated.doom_clock_max,
                    "session_status": updated.status,
                })),
                user_id,
            );

            let clocks = load_puzzle_clocks_for_session(&mut conn, session_id)?;
            Ok((updated, clocks))
        },
    )
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)?;

    Ok(build_graphql_session(updated, clocks))
}

// ============================================================================
// createPuzzleClock (T032) — GM-only
// ============================================================================

pub async fn create_puzzle_clock_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    session_id: Uuid,
    label: String,
    segments_max: i32,
) -> GraphQLResult<GraphQLGeniePuzzleClock> {
    if label.trim().is_empty() {
        return Err(Error::new("label must not be empty"));
    }
    if segments_max <= 0 {
        return Err(Error::new("segmentsMax must be greater than zero"));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let session = tokio::task::spawn_blocking(move || load_session_row(&mut conn, session_id))
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(Error::new)?;

    if !is_dm_of_world(state, user_id, is_admin, session.world_id).await? {
        return Err(Error::new("Only the GM may create a Puzzle Clock"));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let world_id = session.world_id;

    let clock = tokio::task::spawn_blocking(move || -> Result<GeniePuzzleClock, String> {
        let new_clock = NewGeniePuzzleClock {
            session_id,
            label,
            segments_max,
        };
        let clock = diesel::insert_into(world_genie_puzzle_clocks::table)
            .values(&new_clock)
            .returning(GeniePuzzleClock::as_returning())
            .get_result::<GeniePuzzleClock>(&mut conn)
            .map_err(|e| format!("Failed to create Puzzle Clock: {e}"))?;

        let _ = record_world_event(
            &mut conn,
            world_id,
            EVENT_CODE_GENIE_SESSION_STATE,
            Some(serde_json::json!({
                "kind": "puzzle_clock",
                "session_id": session_id,
                "action": "created",
                "clock_id": clock.id,
                "label": clock.label,
                "segments_current": clock.segments_current,
                "segments_max": clock.segments_max,
            })),
            user_id,
        );

        Ok(clock)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)?;

    Ok(GraphQLGeniePuzzleClock::from(clock))
}

// ============================================================================
// advancePuzzleClock (T032/T033) — GM-only; win check per FR-016
// ============================================================================

/// Spec 020 (research.md R4, FR-006/FR-006a): grants every configured
/// reward row for `clock_id` whose `trigger_segment` falls in
/// `(old_segments, new_segments]` and whose `granted_at` is still NULL
/// (the "exactly once" guard), inside the caller's already-open
/// transaction. `actor_id` is the optional actor to credit for
/// `"triggering_actor"`-mode rows (FR-006a); when `None`, those rows
/// fall back to the same whole-party split every `"whole_party"` row
/// uses. Returns the number of reward rows granted (for the `world_events`
/// payload's `rewards_granted` field). `acting_user_id` is the GM whose
/// clock advance triggered the grant — the person accountable for the
/// inventory rows it writes.
pub(crate) fn grant_puzzle_clock_rewards(
    conn: &mut PgConnection,
    clock_id: Uuid,
    world_id: Uuid,
    session_id: Uuid,
    old_segments: i32,
    new_segments: i32,
    actor_id: Option<Uuid>,
    acting_user_id: Uuid,
) -> Result<i32, diesel::result::Error> {
    let due_rewards = world_genie_puzzle_clock_rewards::table
        .filter(world_genie_puzzle_clock_rewards::clock_id.eq(clock_id))
        .filter(world_genie_puzzle_clock_rewards::granted_at.is_null())
        .filter(world_genie_puzzle_clock_rewards::trigger_segment.gt(old_segments))
        .filter(world_genie_puzzle_clock_rewards::trigger_segment.le(new_segments))
        .select(GeniePuzzleClockReward::as_select())
        .load::<GeniePuzzleClockReward>(conn)?;

    if due_rewards.is_empty() {
        return Ok(0);
    }

    // "Whole party" = every non-NPC actor in this clock's world — the
    // same definition the client's useGenieSession.ts already applies
    // locally when it computes partyMembers.
    let party_member_ids: Vec<Uuid> = world_actors::table
        .filter(world_actors::world_id.eq(world_id))
        .filter(world_actors::is_npc.eq(false))
        .order(world_actors::id.asc())
        .select(world_actors::id)
        .load::<Uuid>(conn)?;

    let now = Utc::now().naive_utc();

    for reward in &due_rewards {
        let recipients: Vec<Uuid> = if reward.recipient_mode == "triggering_actor" {
            match actor_id {
                Some(id) => vec![id],
                // FR-006a fallback: no actor attributed on this advance —
                // treat exactly like a whole_party row rather than
                // failing or crediting no one.
                None => party_member_ids.clone(),
            }
        } else {
            party_member_ids.clone()
        };

        if !recipients.is_empty() {
            if let (Some(resource_type), Some(amount)) =
                (&reward.reward_resource_type, reward.reward_resource_amount)
            {
                // Split as evenly as possible; any remainder goes to the
                // first recipients (by id order) so the full configured
                // amount is always granted, never lost to rounding.
                let base_share = amount / recipients.len() as i32;
                let remainder = amount % recipients.len() as i32;
                for (idx, recipient_id) in recipients.iter().enumerate() {
                    let share = base_share + if (idx as i32) < remainder { 1 } else { 0 };
                    if share <= 0 {
                        continue;
                    }
                    let current =
                        load_holding_quantity(conn, session_id, *recipient_id, resource_type)
                            .map_err(|_| diesel::result::Error::RollbackTransaction)?;
                    set_holding_quantity(
                        conn,
                        session_id,
                        *recipient_id,
                        resource_type,
                        current + share,
                    )
                    .map_err(|_| diesel::result::Error::RollbackTransaction)?;
                }
            } else if let (Some(item_id), Some(quantity)) =
                (reward.reward_item_id, reward.reward_item_quantity)
            {
                // Item rewards aren't divisible — every recipient gets
                // the full configured quantity (a "whole_party" item
                // reward means "everyone gets one," not "split one item
                // between everyone").
                for recipient_id in &recipients {
                    grant_item_to_actor_in_tx(
                        conn,
                        *recipient_id,
                        item_id,
                        quantity,
                        acting_user_id,
                    )?;
                }
            }
        }

        diesel::update(
            world_genie_puzzle_clock_rewards::table
                .filter(world_genie_puzzle_clock_rewards::id.eq(reward.id)),
        )
        .set(world_genie_puzzle_clock_rewards::granted_at.eq(now))
        .execute(conn)?;
    }

    Ok(due_rewards.len() as i32)
}

/// Sync upsert of one item into an actor's inventory, for use inside an
/// already-open transaction (mirrors `add_item_to_inventory_impl`'s
/// upsert query — that function owns its own connection/spawn_blocking
/// and can't be called from inside this transaction closure, so the
/// core upsert is duplicated here rather than reused). `acting_user_id` is
/// attributed as `created_by` on a new row and `updated_by` on every write,
/// matching `upsert_inventory_entry`.
pub(crate) fn grant_item_to_actor_in_tx(
    conn: &mut PgConnection,
    actor_id: Uuid,
    item_id: Uuid,
    quantity: i32,
    acting_user_id: Uuid,
) -> Result<(), diesel::result::Error> {
    let item_name = world_items::table
        .filter(world_items::id.eq(item_id))
        .select(world_items::name)
        .first::<String>(conn)?;

    diesel::insert_into(world_actor_inventory::table)
        .values((
            world_actor_inventory::actor_id.eq(actor_id),
            world_actor_inventory::item_id.eq(item_id),
            world_actor_inventory::item_name_snapshot.eq(item_name.clone()),
            world_actor_inventory::quantity.eq(quantity),
            world_actor_inventory::created_by.eq(acting_user_id),
            world_actor_inventory::updated_by.eq(acting_user_id),
        ))
        .on_conflict((
            world_actor_inventory::actor_id,
            world_actor_inventory::item_id,
        ))
        .do_update()
        .set((
            world_actor_inventory::quantity.eq(world_actor_inventory::quantity + quantity),
            world_actor_inventory::item_name_snapshot.eq(item_name),
            world_actor_inventory::updated_at.eq(Utc::now().naive_utc()),
            // created_by stays with whoever first stocked the row.
            world_actor_inventory::updated_by.eq(acting_user_id),
        ))
        .execute(conn)?;
    Ok(())
}

pub async fn advance_puzzle_clock_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    clock_id: Uuid,
    delta: i32,
    actor_id: Option<Uuid>,
) -> GraphQLResult<GraphQLGeniePuzzleClock> {
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

    if !is_dm_of_world(state, user_id, is_admin, session.world_id).await? {
        return Err(Error::new("Only the GM may advance a Puzzle Clock"));
    }
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
    let new_segments = (clock.segments_current + delta).clamp(0, clock.segments_max);
    let resolves_now = new_segments >= clock.segments_max;

    let old_segments = clock.segments_current;
    let updated_clock = tokio::task::spawn_blocking(move || -> Result<GeniePuzzleClock, String> {
        conn.transaction(|conn| -> Result<GeniePuzzleClock, diesel::result::Error> {
            let now = Utc::now().naive_utc();
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

            // FR-016 win check: only relevant when this advance just
            // resolved the clock — an unresolved clock can't complete
            // "every active Puzzle Clock resolved".
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

            // FR-006/FR-006a: grant any reward rows newly crossed by this
            // advance, exactly once each, in this same transaction.
            let rewards_granted = grant_puzzle_clock_rewards(
                conn,
                clock_id,
                world_id,
                session_id,
                old_segments,
                new_segments,
                actor_id,
                user_id,
            )?;

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
                })),
                user_id,
            );

            if rewards_granted > 0 {
                let _ = record_world_event(
                    conn,
                    world_id,
                    EVENT_CODE_GENIE_SESSION_STATE,
                    Some(serde_json::json!({
                        "kind": "clock_reward",
                        "session_id": session_id,
                        "clock_id": updated_clock.id,
                        "actor_id": actor_id,
                        "rewards_granted": rewards_granted,
                    })),
                    user_id,
                );
            }

            Ok(updated_clock)
        })
        .map_err(|e| format!("Failed to advance Puzzle Clock: {e}"))
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)?;

    Ok(GraphQLGeniePuzzleClock::from(updated_clock))
}

// ============================================================================
// configurePuzzleClockReward (spec 020, FR-006) — GM-only
// ============================================================================

#[allow(clippy::too_many_arguments)]
pub async fn configure_puzzle_clock_reward_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    clock_id: Uuid,
    trigger_segment: i32,
    reward_resource_type: Option<String>,
    reward_resource_amount: Option<i32>,
    reward_item_id: Option<Uuid>,
    reward_item_quantity: Option<i32>,
    recipient_mode: GenieRewardRecipientMode,
) -> GraphQLResult<GraphQLGeniePuzzleClockReward> {
    if trigger_segment <= 0 {
        return Err(Error::new("triggerSegment must be greater than zero"));
    }
    let is_resource_reward = reward_resource_type.is_some() && reward_resource_amount.is_some();
    let is_item_reward = reward_item_id.is_some() && reward_item_quantity.is_some();
    if is_resource_reward == is_item_reward {
        return Err(Error::new(
            "Exactly one of a resource reward or an item reward must be configured per entry",
        ));
    }

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

    if !is_dm_of_world(state, user_id, is_admin, session.world_id).await? {
        return Err(Error::new("Only the GM may configure Puzzle Clock rewards"));
    }
    let _ = clock;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let reward = tokio::task::spawn_blocking(move || -> Result<GeniePuzzleClockReward, String> {
        let new_reward = NewGeniePuzzleClockReward {
            clock_id,
            trigger_segment,
            reward_resource_type,
            reward_resource_amount,
            reward_item_id,
            reward_item_quantity,
            recipient_mode: recipient_mode.as_db_str().to_string(),
            created_by: user_id,
        };
        diesel::insert_into(world_genie_puzzle_clock_rewards::table)
            .values(&new_reward)
            .returning(GeniePuzzleClockReward::as_returning())
            .get_result::<GeniePuzzleClockReward>(&mut conn)
            .map_err(|e| format!("Failed to configure Puzzle Clock reward: {e}"))
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)?;

    Ok(GraphQLGeniePuzzleClockReward::from(reward))
}
