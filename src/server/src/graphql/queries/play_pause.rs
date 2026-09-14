//! Spec 051: what the record of paused play says, and to whom.
//!
//! Two audiences, kept apart by what each query *selects*, not by what each
//! response leaves out (research R8):
//!
//! - **Operators** (`admin_user`): `playPauseCandidates`, `playPauses` and
//!   `playPauseRequests`, which carry grounds, names and triggers.
//! - **Members** (`require_world_member`): `worldPlayState`, which reads
//!   `paused_at` and `lifted_at` and nothing else. Its types have no field a
//!   reason could be put in, so a client cannot ask for one (FR-011).
//!
//! The GraphQL types here are shared with `mutations_play_pause`, which
//! answers with the same `PlayPause` an operator lists.

use std::collections::{HashMap, HashSet};

use async_graphql::{Context, Enum, Object, SimpleObject};
use base64::Engine as _;
use chrono::{DateTime, NaiveDateTime, Utc};
use diesel::prelude::*;
use uuid::Uuid;

use crate::auth::world_membership::require_world_member;
use crate::graphql::{Error, GraphQLResult, admin_user, app_state, authenticated_user};
use crate::play_pause::live_play::live_among;
use crate::play_pause::models::{
    PauseRequest, PauseRequestState, PauseTrigger, PauseTriggerKind, PlayPause,
};
use crate::schema::{
    content_moderation_actions, users, world_play_pause_requests, world_play_pause_triggers,
    world_play_pauses, worlds,
};

/// A page of `playPauses` when the caller names none.
const PAUSES_PER_PAGE: i32 = 50;
const MAX_PAUSES_PER_PAGE: i32 = 200;
const MAX_CANDIDATES: i32 = 100;

fn utc(at: NaiveDateTime) -> DateTime<Utc> {
    at.and_utc()
}

// ----- Operator-facing types ----------------------------------------------

/// An operator, by the name recorded when they acted. The record keeps it
/// when the account is renamed or gone.
#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "OperatorName")]
pub struct GraphQLOperatorName {
    pub id: Uuid,
    pub name: String,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "PauseTriggerKind")]
pub enum GraphQLPauseTriggerKind {
    Takedown,
    Operator,
    AbuseReport,
}

impl From<PauseTriggerKind> for GraphQLPauseTriggerKind {
    fn from(kind: PauseTriggerKind) -> Self {
        match kind {
            PauseTriggerKind::Takedown => Self::Takedown,
            PauseTriggerKind::Operator => Self::Operator,
            PauseTriggerKind::AbuseReport => Self::AbuseReport,
        }
    }
}

/// What led to a pause (FR-037).
#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "PauseTrigger")]
pub struct GraphQLPauseTrigger {
    pub kind: GraphQLPauseTriggerKind,
    pub moderation_action_id: Option<Uuid>,
    /// The moderation case `moderation_action_id` belongs to, so an operator
    /// can open it: cases are opened by case, not by action.
    pub case_id: Option<Uuid>,
    pub entity_type: Option<String>,
    pub entity_id: Option<Uuid>,
    pub note: Option<String>,
    pub recorded_at: DateTime<Utc>,
}

impl From<PauseTrigger> for GraphQLPauseTrigger {
    fn from(trigger: PauseTrigger) -> Self {
        Self {
            kind: trigger.kind.into(),
            moderation_action_id: trigger.moderation_action_id,
            case_id: None,
            entity_type: trigger.entity_type,
            entity_id: trigger.entity_id,
            note: trigger.note,
            recorded_at: utc(trigger.recorded_at),
        }
    }
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "PauseRequestState")]
pub enum GraphQLPauseRequestState {
    Pending,
    Approved,
    Declined,
}

impl From<PauseRequestState> for GraphQLPauseRequestState {
    fn from(state: PauseRequestState) -> Self {
        match state {
            PauseRequestState::Pending => Self::Pending,
            PauseRequestState::Approved => Self::Approved,
            PauseRequestState::Declined => Self::Declined,
        }
    }
}

impl From<GraphQLPauseRequestState> for PauseRequestState {
    fn from(state: GraphQLPauseRequestState) -> Self {
        match state {
            GraphQLPauseRequestState::Pending => Self::Pending,
            GraphQLPauseRequestState::Approved => Self::Approved,
            GraphQLPauseRequestState::Declined => Self::Declined,
        }
    }
}

/// A request to pause a world, as an operator reads it (FR-031).
#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "PauseRequest")]
pub struct GraphQLPauseRequest {
    pub id: Uuid,
    pub world_id: Uuid,
    /// The name when it was raised, which outlives the world.
    pub world_name: String,
    pub world_exists: bool,
    pub raised_at: DateTime<Utc>,
    pub state: GraphQLPauseRequestState,
    /// Whether the world is being played as the operator reads this, not
    /// when the request was raised (FR-031).
    pub played_now: bool,
    /// Oldest first.
    pub triggers: Vec<GraphQLPauseTrigger>,
    pub decided_by: Option<GraphQLOperatorName>,
    pub decided_at: Option<DateTime<Utc>>,
    pub decision_note: Option<String>,
}

/// One page of `playPauseRequests`, newest first, in the shape
/// `PlayPauseConnection` pages with.
#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "PauseRequestConnection")]
pub struct GraphQLPauseRequestConnection {
    pub nodes: Vec<GraphQLPauseRequest>,
    pub next_cursor: Option<String>,
}

/// A pause, as an operator reads it.
#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "PlayPause")]
pub struct GraphQLPlayPause {
    pub id: Uuid,
    pub world_id: Uuid,
    /// The name when it was paused, which outlives the world (FR-053).
    pub world_name: String,
    pub world_exists: bool,
    pub paused_by: GraphQLOperatorName,
    pub paused_at: DateTime<Utc>,
    pub grounds: String,
    pub request_id: Option<Uuid>,
    /// Oldest first.
    pub triggers: Vec<GraphQLPauseTrigger>,
    pub lifted_by: Option<GraphQLOperatorName>,
    pub lifted_at: Option<DateTime<Utc>>,
    pub lift_grounds: Option<String>,
    pub played_now: bool,
}

/// One page of `playPauses`, newest first.
///
/// The contract names the type and leaves its shape open. It is the shape
/// `compendiumEntries` pages with: the rows, and the cursor for the next page,
/// `null` on the last.
#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "PlayPauseConnection")]
pub struct GraphQLPlayPauseConnection {
    pub nodes: Vec<GraphQLPlayPause>,
    pub next_cursor: Option<String>,
}

/// A world an operator might pause.
#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "PauseCandidateWorld")]
pub struct GraphQLPauseCandidateWorld {
    pub id: Uuid,
    pub name: String,
    pub owner_name: String,
    pub played_now: bool,
    pub paused: bool,
}

/// Every stored pause in `rows`, with its triggers and whether its world is
/// still there — three queries however many rows, not three per row.
pub(crate) fn play_pauses_for_graphql(
    conn: &mut PgConnection,
    rows: Vec<PlayPause>,
) -> QueryResult<Vec<GraphQLPlayPause>> {
    let pause_ids: Vec<Uuid> = rows.iter().map(|row| row.id).collect();
    let world_ids: Vec<Uuid> = rows.iter().map(|row| row.world_id).collect();

    let mut triggers: HashMap<Uuid, Vec<GraphQLPauseTrigger>> = HashMap::new();
    let rows_of_triggers = world_play_pause_triggers::table
        .filter(world_play_pause_triggers::pause_id.eq_any(&pause_ids))
        .order((
            world_play_pause_triggers::recorded_at.asc(),
            world_play_pause_triggers::id.asc(),
        ))
        .select(PauseTrigger::as_select())
        .load(conn)?;
    for (owner, trigger) in with_case_ids(conn, rows_of_triggers, |t| t.pause_id)? {
        triggers.entry(owner).or_default().push(trigger);
    }

    let existing = existing_worlds(conn, &world_ids)?;
    let live = live_among(conn, &world_ids)?;

    Ok(rows
        .into_iter()
        .map(|row| GraphQLPlayPause {
            id: row.id,
            world_id: row.world_id,
            world_name: row.world_name,
            world_exists: existing.contains(&row.world_id),
            paused_by: GraphQLOperatorName {
                id: row.paused_by,
                name: row.paused_by_name,
            },
            paused_at: utc(row.paused_at),
            grounds: row.grounds,
            request_id: row.request_id,
            triggers: triggers.remove(&row.id).unwrap_or_default(),
            lifted_by: row
                .lifted_by
                .zip(row.lifted_by_name)
                .map(|(id, name)| GraphQLOperatorName { id, name }),
            lifted_at: row.lifted_at.map(utc),
            lift_grounds: row.lift_grounds,
            played_now: live.contains(&row.world_id),
        })
        .collect())
}

/// `rows` as an operator reads them, each paired with the pause or request
/// `owner` says it belongs to, and each takedown's moderation case filled in
/// from its action — one query however many triggers.
fn with_case_ids(
    conn: &mut PgConnection,
    rows: Vec<PauseTrigger>,
    owner: impl Fn(&PauseTrigger) -> Option<Uuid>,
) -> QueryResult<Vec<(Uuid, GraphQLPauseTrigger)>> {
    let action_ids: Vec<Uuid> = rows.iter().filter_map(|t| t.moderation_action_id).collect();
    let cases: HashMap<Uuid, Uuid> = if action_ids.is_empty() {
        HashMap::new()
    } else {
        content_moderation_actions::table
            .filter(content_moderation_actions::id.eq_any(&action_ids))
            .select((
                content_moderation_actions::id,
                content_moderation_actions::case_id,
            ))
            .load::<(Uuid, Uuid)>(conn)?
            .into_iter()
            .collect()
    };
    Ok(rows
        .into_iter()
        .filter_map(|row| {
            let owner = owner(&row)?;
            let case_id = row
                .moderation_action_id
                .and_then(|action| cases.get(&action).copied());
            let mut trigger = GraphQLPauseTrigger::from(row);
            trigger.case_id = case_id;
            Some((owner, trigger))
        })
        .collect())
}

fn existing_worlds(conn: &mut PgConnection, world_ids: &[Uuid]) -> QueryResult<HashSet<Uuid>> {
    Ok(worlds::table
        .filter(worlds::id.eq_any(world_ids))
        .select(worlds::id)
        .load::<Uuid>(conn)?
        .into_iter()
        .collect())
}

/// Every stored request in `rows`, with its triggers, whether its world is
/// still there and whether it is played now — four queries however many rows.
pub(crate) fn pause_requests_for_graphql(
    conn: &mut PgConnection,
    rows: Vec<PauseRequest>,
) -> QueryResult<Vec<GraphQLPauseRequest>> {
    let request_ids: Vec<Uuid> = rows.iter().map(|row| row.id).collect();
    let world_ids: Vec<Uuid> = rows.iter().map(|row| row.world_id).collect();

    let mut triggers: HashMap<Uuid, Vec<GraphQLPauseTrigger>> = HashMap::new();
    let rows_of_triggers = world_play_pause_triggers::table
        .filter(world_play_pause_triggers::request_id.eq_any(&request_ids))
        .order((
            world_play_pause_triggers::recorded_at.asc(),
            world_play_pause_triggers::id.asc(),
        ))
        .select(PauseTrigger::as_select())
        .load(conn)?;
    for (owner, trigger) in with_case_ids(conn, rows_of_triggers, |t| t.request_id)? {
        triggers.entry(owner).or_default().push(trigger);
    }

    let existing = existing_worlds(conn, &world_ids)?;
    let live = live_among(conn, &world_ids)?;

    Ok(rows
        .into_iter()
        .map(|row| GraphQLPauseRequest {
            id: row.id,
            world_id: row.world_id,
            world_name: row.world_name,
            world_exists: existing.contains(&row.world_id),
            raised_at: utc(row.raised_at),
            state: row.state.into(),
            played_now: live.contains(&row.world_id),
            triggers: triggers.remove(&row.id).unwrap_or_default(),
            decided_by: row
                .decided_by
                .zip(row.decided_by_name)
                .map(|(id, name)| GraphQLOperatorName { id, name }),
            decided_at: row.decided_at.map(utc),
            decision_note: row.decision_note,
        })
        .collect())
}

fn encode_cursor(id: Uuid) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(id.as_bytes())
}

fn decode_cursor(cursor: &str) -> GraphQLResult<Uuid> {
    base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(cursor)
        .ok()
        .and_then(|bytes| Uuid::from_slice(&bytes).ok())
        .ok_or_else(|| Error::new("That is not a page of the pause record."))
}

/// `%` and `_` in what an operator typed are letters, not wildcards.
fn like_pattern(search: &str) -> String {
    format!(
        "%{}%",
        search
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_")
    )
}

fn connection(
    ctx: &Context<'_>,
) -> GraphQLResult<diesel::r2d2::PooledConnection<diesel::r2d2::ConnectionManager<PgConnection>>> {
    app_state(ctx)?
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))
}

// ----- Member-facing types --------------------------------------------------

/// One stretch of paused play: *when*, and nothing else.
#[derive(SimpleObject, Debug, Clone, PartialEq, Eq)]
#[graphql(name = "PlayPauseSpan")]
pub struct GraphQLPlayPauseSpan {
    pub paused_at: DateTime<Utc>,
    pub lifted_at: Option<DateTime<Utc>>,
}

/// Whether a world's play is paused, as its members may know it (FR-050).
#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "WorldPlayState")]
pub struct GraphQLWorldPlayState {
    pub paused: bool,
    /// `null` when not paused.
    pub paused_at: Option<DateTime<Utc>>,
    /// Newest first.
    pub history: Vec<GraphQLPlayPauseSpan>,
}

/// The member-facing read, as a function of a connection.
///
/// Selects `paused_at` and `lifted_at` and nothing more. Grounds, names,
/// triggers and requests are not withheld from the answer; they are never
/// read (research R8).
pub fn world_play_state_sync(
    conn: &mut PgConnection,
    world_id: Uuid,
) -> QueryResult<GraphQLWorldPlayState> {
    let spans: Vec<(NaiveDateTime, Option<NaiveDateTime>)> = world_play_pauses::table
        .filter(world_play_pauses::world_id.eq(world_id))
        .order((
            world_play_pauses::paused_at.desc(),
            world_play_pauses::id.desc(),
        ))
        .select((world_play_pauses::paused_at, world_play_pauses::lifted_at))
        .load(conn)?;

    let paused_at = spans
        .iter()
        .find(|(_, lifted_at)| lifted_at.is_none())
        .map(|(paused_at, _)| utc(*paused_at));

    Ok(GraphQLWorldPlayState {
        paused: paused_at.is_some(),
        paused_at,
        history: spans
            .into_iter()
            .map(|(paused_at, lifted_at)| GraphQLPlayPauseSpan {
                paused_at: utc(paused_at),
                lifted_at: lifted_at.map(utc),
            })
            .collect(),
    })
}

#[derive(Default)]
pub struct PlayPauseQuery;

#[Object]
impl PlayPauseQuery {
    /// Worlds whose name contains `search`, or whose id it is, for an
    /// operator deciding what to pause.
    async fn play_pause_candidates(
        &self,
        ctx: &Context<'_>,
        search: String,
        #[graphql(default = 20)] first: Option<i32>,
    ) -> GraphQLResult<Vec<GraphQLPauseCandidateWorld>> {
        admin_user(ctx)?;
        let mut conn = connection(ctx)?;
        let limit = i64::from(first.unwrap_or(20).clamp(1, MAX_CANDIDATES));
        let search = search.trim().to_string();

        let rows = tokio::task::spawn_blocking(move || {
            let mut query = worlds::table
                .inner_join(users::table.on(users::id.eq(worlds::created_by)))
                .select((worlds::id, worlds::name, users::username))
                .order((worlds::name.asc(), worlds::id.asc()))
                .limit(limit)
                .into_boxed();
            // An operator handed a world's id by a report pastes the id.
            query = match Uuid::parse_str(&search) {
                Ok(id) => query.filter(worlds::id.eq(id)),
                Err(_) => query.filter(worlds::name.ilike(like_pattern(&search))),
            };
            let found: Vec<(Uuid, String, String)> = query.load(&mut conn)?;

            let ids: Vec<Uuid> = found.iter().map(|(id, _, _)| *id).collect();
            let paused: HashSet<Uuid> = world_play_pauses::table
                .filter(world_play_pauses::world_id.eq_any(&ids))
                .filter(world_play_pauses::lifted_at.is_null())
                .select(world_play_pauses::world_id)
                .load::<Uuid>(&mut conn)?
                .into_iter()
                .collect();
            let live = live_among(&mut conn, &ids)?;

            QueryResult::Ok(
                found
                    .into_iter()
                    .map(|(id, name, owner_name)| GraphQLPauseCandidateWorld {
                        paused: paused.contains(&id),
                        id,
                        name,
                        played_now: live.contains(&id),
                        owner_name,
                    })
                    .collect(),
            )
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to search worlds"))?;

        Ok(rows)
    }

    /// The pause record, newest first. `active` narrows it to pauses in force
    /// (`true`) or lifted (`false`); `worldId` to one world.
    async fn play_pauses(
        &self,
        ctx: &Context<'_>,
        active: Option<bool>,
        world_id: Option<Uuid>,
        first: Option<i32>,
        after: Option<String>,
    ) -> GraphQLResult<GraphQLPlayPauseConnection> {
        admin_user(ctx)?;
        let after = after.as_deref().map(decode_cursor).transpose()?;
        let limit = first
            .unwrap_or(PAUSES_PER_PAGE)
            .clamp(1, MAX_PAUSES_PER_PAGE) as usize;
        let mut conn = connection(ctx)?;

        tokio::task::spawn_blocking(move || {
            let mut query = world_play_pauses::table
                .select(PlayPause::as_select())
                // Ids are v7, minted as the pause is written, so id order is
                // time order and a total one: the cursor is the last id.
                .order(world_play_pauses::id.desc())
                .limit(limit as i64 + 1)
                .into_boxed();
            if let Some(active) = active {
                query = if active {
                    query.filter(world_play_pauses::lifted_at.is_null())
                } else {
                    query.filter(world_play_pauses::lifted_at.is_not_null())
                };
            }
            if let Some(world_id) = world_id {
                query = query.filter(world_play_pauses::world_id.eq(world_id));
            }
            if let Some(after) = after {
                query = query.filter(world_play_pauses::id.lt(after));
            }

            let mut rows: Vec<PlayPause> = query.load(&mut conn)?;
            let next_cursor = if rows.len() > limit {
                rows.truncate(limit);
                rows.last().map(|row| encode_cursor(row.id))
            } else {
                None
            };

            Ok(GraphQLPlayPauseConnection {
                nodes: play_pauses_for_graphql(&mut conn, rows)?,
                next_cursor,
            })
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_: diesel::result::Error| Error::new("Failed to read the pause record"))
    }

    /// Requests to pause a world, newest first: the pending ones an operator
    /// has to decide, unless `state` asks for decided ones.
    async fn play_pause_requests(
        &self,
        ctx: &Context<'_>,
        #[graphql(default_with = "Some(GraphQLPauseRequestState::Pending)")] state: Option<
            GraphQLPauseRequestState,
        >,
        first: Option<i32>,
        after: Option<String>,
    ) -> GraphQLResult<GraphQLPauseRequestConnection> {
        admin_user(ctx)?;
        let after = after.as_deref().map(decode_cursor).transpose()?;
        let limit = first
            .unwrap_or(PAUSES_PER_PAGE)
            .clamp(1, MAX_PAUSES_PER_PAGE) as usize;
        let mut conn = connection(ctx)?;

        tokio::task::spawn_blocking(move || {
            let mut query = world_play_pause_requests::table
                .select(PauseRequest::as_select())
                // v7 ids, minted as the request is raised: id order is time
                // order, and the cursor is the last id.
                .order(world_play_pause_requests::id.desc())
                .limit(limit as i64 + 1)
                .into_boxed();
            if let Some(state) = state {
                query = query
                    .filter(world_play_pause_requests::state.eq(PauseRequestState::from(state)));
            }
            if let Some(after) = after {
                query = query.filter(world_play_pause_requests::id.lt(after));
            }

            let mut rows: Vec<PauseRequest> = query.load(&mut conn)?;
            let next_cursor = if rows.len() > limit {
                rows.truncate(limit);
                rows.last().map(|row| encode_cursor(row.id))
            } else {
                None
            };

            Ok(GraphQLPauseRequestConnection {
                nodes: pause_requests_for_graphql(&mut conn, rows)?,
                next_cursor,
            })
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_: diesel::result::Error| Error::new("Failed to read the pause requests"))
    }

    /// Whether this world's play is paused, since when, and when it was
    /// before (FR-024, FR-050).
    ///
    /// Guarded by membership and **not** by the pause gate, because the
    /// notice a paused table is sent to is what asks it. A site admin who is
    /// not a member is refused like anyone else who is not.
    async fn world_play_state(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
    ) -> GraphQLResult<GraphQLWorldPlayState> {
        let caller = authenticated_user(ctx)?.user_id;
        let mut conn = connection(ctx)?;

        tokio::task::spawn_blocking(move || {
            require_world_member(&mut conn, caller, world_id)
                .map_err(|_| Error::new("You must be a member of this world"))?;
            world_play_state_sync(&mut conn, world_id)
                .map_err(|_| Error::new("Failed to read whether this world is paused"))
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
    }
}

#[cfg(test)]
#[path = "play_pause_tests.rs"]
mod play_pause_tests;
