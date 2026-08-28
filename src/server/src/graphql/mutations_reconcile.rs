//! Replaying changes made while disconnected (spec 028 US7, T075–T078).
//!
//! `contracts/graphql-delta-sync.md` specifies this mutation; the rules it
//! adjudicates by live in [`thunderforge_cache_core::conflict`], which until
//! now had no caller anywhere in the tree.
//!
//! # What makes this safe to replay
//!
//! The client stores the emitted world-store command **verbatim** and sends
//! it back untouched. So a queued change is not a special kind of edit with
//! its own privileges — it is the same edit, arriving late, and it is
//! authorized here against **current** permissions rather than against
//! whatever the user had when they made it (FR-042). Someone removed from a
//! world while offline reconnects to find their queued edits refused, which
//! is the entire point: the alternative is a window in which revoked access
//! still writes.
//!
//! # Exactly one outcome per submitted change
//!
//! FR-041 prohibits silent loss, and the shape of the code is what enforces
//! it: outcomes are produced by mapping over the inputs, so an input without
//! an outcome is not something this function can express. Every early return
//! inside the loop produces a rejection rather than a `continue`.
//!
//! # Conflict bookkeeping, and its honest limit
//!
//! `conflict::resolve` needs to know what already landed on the same item and
//! who put it there. That memory lives in [`ReconcileMarks`], an in-process
//! map, and the limitation is stated rather than hidden: **a server restart
//! forgets it**, after which two players reconnecting either side of the
//! restart both apply and the later one silently wins on last-write. The
//! window this matters in is the minutes between one client reconnecting and
//! the next, and the cost of being wrong is a token position — recoverable,
//! visible, and re-doable.
//!
//! Making it durable means a table keyed by (world, item) with the winning
//! role and reconnect sequence, written in the same transaction as the edit.
//! That is a schema decision, and it is the right one if offline authoring
//! ever widens past FR-035a's token position/rotation/scale — at which point
//! the thing being lost stops being a position and starts being work.
//!
//! # Peer-adjudicated submissions (Phase 10a)
//!
//! The same mutation carries changes the Game Master's client adjudicated
//! while the server was unreachable (ADR-052). Two things are different about
//! those, and both are deliberately small:
//!
//! - A change may be **attributed** to a user other than the submitter. The
//!   only check is whether the submitter holds the GM role in this world
//!   (FR-061) — the check that already governs every GM-only operation. No
//!   session keypairs, no signature format, no new trust root. The GM is the
//!   trusted party: a GM acting on a player's behalf produces no flag and no
//!   notification to anyone (FR-061b). A non-GM may never attribute a change
//!   to anyone else (FR-061a).
//! - A change may carry a **reported outcome** the server can determine for
//!   itself — a dice result, server-authoritative since ADR-044. Where the
//!   two differ the change is applied anyway and the difference is disclosed
//!   to the GM with both numbers (FR-064 to FR-066). Nothing is rejected,
//!   interrupted, altered, sanctioned, or told to the other players, and
//!   there is no dispute workflow to reach.
//!
//! ## Why detection is silent by default, structurally
//!
//! A missed discrepancy costs nothing — the GM runs their table either way. A
//! false one puts an innocent player under suspicion in front of the only
//! person who can act on it (FR-067a). So reporting is arranged as the
//! exception that must be *reached*, not the default that must be escaped:
//! [`DeterminedValue`] is the only thing [`GraphQLDiscrepancy`] can be built
//! from, and it is constructible only from a stored server-authoritative row.
//! Every ambiguity — a timeout, a row that is not there, an outcome that does
//! not parse, an unrecognised format version, an outcome the server has no
//! independent basis for at all (FR-068) — is therefore not a branch someone
//! could later get wrong but simply the absence of that value. "No
//! determination" and "determined and equal" leave [`discrepancy_between`] by
//! the same `None`.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use async_graphql::{Context, Enum, InputObject, Object, SimpleObject};
use diesel::prelude::*;
use serde::Deserialize;
use thunderforge_cache_core::conflict::{self, Contender, ReconnectSeq, Role, Winner};
use uuid::Uuid;

use crate::auth::world_membership::require_world_member;
use crate::graphql::{GraphQLResult, app_state, authenticated_user};
use crate::world_events::{EVENT_CODE_TOKEN_CHANGED, record_world_event};

/// Why a queued change did not stand. Mirrors
/// `thunderforge_cache_core::queue::RejectionReason`.
#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum GraphQLRejectionReason {
    /// The user may no longer make this change (FR-042).
    PermissionDenied,
    /// A conflicting change took precedence. Deliberately distinct from a
    /// generic failure: it is a working rule, not an error, and the UI has to
    /// be able to say which.
    Superseded,
    /// The thing being changed no longer exists.
    GoneAway,
    /// Malformed, or outside what may be edited offline (FR-035a).
    Invalid,
}

/// One change made while disconnected.
#[derive(InputObject)]
pub struct QueuedChangeInput {
    /// Client-generated correlation id. Echoed back so a client can match an
    /// outcome to the edit it made, without the server needing to understand
    /// the edit.
    pub local_id: String,
    /// The emitted world-store command, verbatim.
    pub command: async_graphql::Json<serde_json::Value>,
    /// Who originated this change, when that is not the submitter — a
    /// peer-adjudicated change relayed by the GM's client (FR-061). Absent, or
    /// naming the submitter, is an ordinary self-submitted replay.
    pub attributed_to_user_id: Option<Uuid>,
    /// What the client says the outcome of this change was, where the server
    /// can determine that for itself (FR-064). Absent for everything the
    /// server has no independent basis for — an ordinary token move, for
    /// instance, which is most of what this mutation carries (FR-068).
    pub reported_outcome: Option<ReportedOutcomeInput>,
}

/// The format version of [`ReportedOutcomeInput`] this server understands.
///
/// A client reporting any other version is telling us something we cannot
/// read, which is an ambiguity and never a mismatch.
const REPORTED_OUTCOME_VERSION: i32 = 1;

/// The one outcome kind the server can independently determine today: a dice
/// result, authoritative server-side since ADR-044 and stored in
/// `world_roll_records`.
const REPORTED_OUTCOME_KIND_DICE: &str = "dice";

/// Two determined values closer than this are the same number that survived a
/// round trip through JSON, not a discrepancy. The tolerance leans towards
/// silence on purpose: dice totals are integers, so nothing genuinely
/// different ever lands inside it.
const DISCREPANCY_EPSILON: f64 = 1e-6;

/// An outcome a client reports for a change it made while server-isolated.
#[derive(InputObject, Clone)]
pub struct ReportedOutcomeInput {
    /// Which kind of outcome this is. Only `dice` is independently
    /// determinable today; anything else the server cannot check and does not
    /// guess at.
    pub kind: String,
    /// Format version. Deliberately explicit so a client one release ahead
    /// produces silence rather than a fabricated mismatch.
    pub version: i32,
    /// The server-side record this outcome claims to be reporting on — a
    /// `world_roll_records` id for `dice`.
    pub record_id: Option<Uuid>,
    /// The value the client says the outcome was.
    pub value: Option<f64>,
}

/// A client-reported outcome the server was able to make sense of.
///
/// Produced only by [`parse_reported_outcome`]; anything malformed, partial,
/// or of an unknown kind or version never becomes one of these.
struct ReportedOutcome {
    record_id: Uuid,
    claimed: f64,
}

/// A value the server determined for itself.
///
/// The only route to a [`GraphQLDiscrepancy`], and constructible only from a
/// stored server-authoritative row. Its absence is what every ambiguity
/// collapses to, so "we could not tell" is not a case anyone has to remember
/// to handle separately from "they agreed".
struct DeterminedValue(f64);

/// A client reported one value; the server determined another (FR-065).
///
/// Disclosure and nothing more: it accompanies an outcome that was applied,
/// carries both numbers so the GM can inspect them, and has no field for a
/// verdict because there is no verdict to render (FR-065a, FR-066).
#[derive(SimpleObject, Debug)]
pub struct GraphQLDiscrepancy {
    /// Whose outcome this was — the attributed originator, not the submitter,
    /// since the GM relaying a player's change is the ordinary case.
    pub user_id: Uuid,
    /// The server record the two values are about, so the GM can go and look.
    pub record_id: Uuid,
    /// What the client said.
    pub reported_value: f64,
    /// What the server determined.
    pub determined_value: f64,
}

/// What became of one queued change.
#[derive(SimpleObject)]
pub struct GraphQLReconcileOutcome {
    pub local_id: String,
    pub applied: bool,
    pub reason: Option<GraphQLRejectionReason>,
    /// Set when `reason` is `SUPERSEDED`: who won, so the UI can say so
    /// rather than reporting an anonymous failure.
    pub superseded_by_role: Option<String>,
    /// Set only when the server determined a different value than the client
    /// reported, and only for a Game Master submitter (FR-067). It never
    /// changes what happened to the change — `applied` above says that.
    pub discrepancy: Option<GraphQLDiscrepancy>,
}

impl GraphQLReconcileOutcome {
    /// Named `accepted` rather than `applied`: `SimpleObject` generates a
    /// field accessor called `applied`, and a constructor of the same name
    /// collides with it.
    fn accepted(local_id: String) -> Self {
        Self {
            local_id,
            applied: true,
            reason: None,
            superseded_by_role: None,
            discrepancy: None,
        }
    }

    fn rejected(local_id: String, reason: GraphQLRejectionReason) -> Self {
        Self {
            local_id,
            applied: false,
            reason: Some(reason),
            superseded_by_role: None,
            discrepancy: None,
        }
    }

    fn superseded(local_id: String, by: Role) -> Self {
        Self {
            local_id,
            applied: false,
            reason: Some(GraphQLRejectionReason::Superseded),
            superseded_by_role: Some(role_name(by).to_string()),
            discrepancy: None,
        }
    }

    /// Attach a disclosure to whatever became of the change.
    ///
    /// Separate from the constructors above, and applied to all of them,
    /// because the two answers are independent: a discrepancy does not decide
    /// whether a change applied (FR-066), and whether a change applied does
    /// not decide whether the numbers differed.
    fn disclosing(mut self, discrepancy: Option<GraphQLDiscrepancy>) -> Self {
        self.discrepancy = discrepancy;
        self
    }
}

fn role_name(role: Role) -> &'static str {
    match role {
        Role::GameMaster => "GameMaster",
        Role::Player => "Player",
    }
}

/// Who last won an item, and the reconnect that won it.
#[derive(Clone, Copy)]
struct Mark {
    role: Role,
    reconnect_seq: ReconnectSeq,
}

/// Per-world memory of what reconciliation has already settled.
#[derive(Default)]
struct ReconcileMarks {
    /// Monotonic, server-assigned, one per world. This is `ReconnectSeq`:
    /// deliberately a counter and never a clock, because a client timestamp
    /// is forgeable and a skewed one would silently overwrite other people's
    /// work — the exact failure a conflict rule exists to prevent.
    next_seq: HashMap<Uuid, ReconnectSeq>,
    /// The winner so far, per (world, token).
    marks: HashMap<(Uuid, Uuid), Mark>,
}

fn marks() -> &'static Mutex<ReconcileMarks> {
    static MARKS: OnceLock<Mutex<ReconcileMarks>> = OnceLock::new();
    MARKS.get_or_init(|| Mutex::new(ReconcileMarks::default()))
}

/// Take this reconnection's position in the order, for one world.
fn take_reconnect_seq(world_id: Uuid) -> ReconnectSeq {
    let Ok(mut state) = marks().lock() else {
        // A poisoned lock means another thread panicked mid-update. Handing
        // out 0 makes this reconnection sort first, which is the *generous*
        // reading — it can only lose to a GM, never silently beat an equal
        // peer that reconnected earlier.
        return 0;
    };
    let slot = state.next_seq.entry(world_id).or_insert(0);
    let seq = *slot;
    *slot = slot.saturating_add(1);
    seq
}

/// The FR-035a shape: a token move or manipulate, and nothing else.
///
/// Parsed structurally rather than by trusting a `type` field alone, because
/// the restriction is what keeps conflict resolution honest — precedence can
/// settle two positions, and cannot settle a create racing a delete without
/// destroying work someone cannot see was destroyed.
#[derive(Debug, Deserialize)]
struct UpsertTokenCommand {
    #[serde(rename = "type")]
    kind: String,
    token: CommandToken,
}

#[derive(Debug, Deserialize)]
struct CommandToken {
    id: Uuid,
    x: Option<f64>,
    y: Option<f64>,
    rotation: Option<f64>,
    scale: Option<f64>,
}

/// What a queued change is asking for, once it has been understood.
struct TokenEdit {
    token_id: Uuid,
    x: Option<f64>,
    y: Option<f64>,
    rotation: Option<f64>,
    scale: Option<f64>,
}

/// Interpret a queued command, refusing anything outside FR-035a.
///
/// Returns `None` for a command this mutation will not replay — which is a
/// rejection, never a silent skip.
fn parse_token_edit(command: &serde_json::Value) -> Option<TokenEdit> {
    let parsed: UpsertTokenCommand = serde_json::from_value(command.clone()).ok()?;
    if parsed.kind != "upsert_token" {
        return None;
    }
    // At least one of the permitted fields must be present, or this is a
    // no-op dressed as an edit.
    if parsed.token.x.is_none()
        && parsed.token.y.is_none()
        && parsed.token.rotation.is_none()
        && parsed.token.scale.is_none()
    {
        return None;
    }
    Some(TokenEdit {
        token_id: parsed.token.id,
        x: parsed.token.x,
        y: parsed.token.y,
        rotation: parsed.token.rotation,
        scale: parsed.token.scale,
    })
}

/// Make sense of a reported outcome, or decline to.
///
/// Returns `None` for every shape this server cannot read with confidence: an
/// unrecognised kind, a format version from a client that is not this one, a
/// missing record id or value, a value that is not a finite number. Each of
/// those is a parse failure, and a parse failure is an ambiguity — the one
/// thing it must never become is a reported value the server then "disagrees"
/// with (FR-067a).
fn parse_reported_outcome(input: &ReportedOutcomeInput) -> Option<ReportedOutcome> {
    if input.version != REPORTED_OUTCOME_VERSION {
        return None;
    }
    if input.kind != REPORTED_OUTCOME_KIND_DICE {
        return None;
    }
    let claimed = input.value?;
    if !claimed.is_finite() {
        return None;
    }
    Some(ReportedOutcome {
        record_id: input.record_id?,
        claimed,
    })
}

/// Turn a database answer into a determination, or into nothing.
///
/// Split out from the query it interprets so the failing cases are testable
/// without contriving a broken database: an `Err` (a statement timeout, a
/// dropped connection, anything else that went wrong) and an absent row both
/// mean the server did not determine a value, which is materially different
/// from determining one that happens to differ.
fn determination_from_lookup(
    lookup: Result<Option<f64>, diesel::result::Error>,
) -> Option<DeterminedValue> {
    match lookup {
        Ok(Some(value)) if value.is_finite() => Some(DeterminedValue(value)),
        _ => None,
    }
}

/// Read what the server itself recorded for a roll in this world.
///
/// Scoped by `world_id` as well as id, so a record id from another world reads
/// as "no such row" rather than as a value to compare against.
fn determined_roll_value(
    conn: &mut PgConnection,
    world_id: Uuid,
    record_id: Uuid,
) -> Option<DeterminedValue> {
    use crate::schema::world_roll_records;
    determination_from_lookup(
        world_roll_records::table
            .filter(world_roll_records::id.eq(record_id))
            .filter(world_roll_records::world_id.eq(world_id))
            .select(world_roll_records::result_value)
            .first::<f64>(conn)
            .optional(),
    )
}

/// The whole of the reporting decision, in one place.
///
/// `determined?` is the load-bearing line: with no determination in hand there
/// is nothing to build a discrepancy from, so the ambiguous cases and the
/// agreeing case leave by the same `None` and no future edit can make one of
/// them loud without making the other loud too.
fn discrepancy_between(
    subject_user: Uuid,
    reported: &ReportedOutcome,
    determined: Option<DeterminedValue>,
) -> Option<GraphQLDiscrepancy> {
    let DeterminedValue(determined) = determined?;
    if (reported.claimed - determined).abs() <= DISCREPANCY_EPSILON {
        return None;
    }
    Some(GraphQLDiscrepancy {
        user_id: subject_user,
        record_id: reported.record_id,
        reported_value: reported.claimed,
        determined_value: determined,
    })
}

/// Compare a client-reported outcome against the server's own (FR-064).
///
/// A change with nothing reported — every ordinary token move — never reaches
/// the comparison at all (FR-068): absence of evidence is not a flag.
fn detect_discrepancy(
    conn: &mut PgConnection,
    world_id: Uuid,
    subject_user: Uuid,
    reported: Option<&ReportedOutcomeInput>,
) -> Option<GraphQLDiscrepancy> {
    let reported = parse_reported_outcome(reported?)?;
    let determined = determined_roll_value(conn, world_id, reported.record_id);
    discrepancy_between(subject_user, &reported, determined)
}

/// A world membership role, as `conflict` sees it.
///
/// Owner and GM both run the table, which is the same convention
/// `roleBadgeLabel` uses in the client and `ability_impl` uses server-side.
pub fn role_from_membership(member_role: &str) -> Role {
    match member_role {
        "Owner" | "GM" => Role::GameMaster,
        _ => Role::Player,
    }
}

#[derive(Default)]
pub struct ReconcileMutation;

#[Object]
impl ReconcileMutation {
    /// Replay changes made while disconnected, and report what became of
    /// each (US7).
    ///
    /// Applied in submitted order, so a client's own sequential edits do not
    /// reorder against each other — replaying "move right, then move up"
    /// backwards lands the token somewhere the user never put it.
    async fn reconcile_queued_changes(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
        changes: Vec<QueuedChangeInput>,
    ) -> GraphQLResult<Vec<GraphQLReconcileOutcome>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let user_id = auth_user.user_id;

        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| async_graphql::Error::new("Failed to get DB connection"))?;

        // FR-042, and the first half of why this is safe: membership is read
        // now, not when the edits were made. A user removed from the world
        // while offline gets every queued change refused.
        let member_role = match require_world_member(&mut conn, user_id, world_id) {
            Ok(role) => role,
            Err(_) => {
                // Every change rejected, one outcome each. Deliberately not
                // an error: the client needs the per-change verdicts to know
                // what to tell the user and what to stop replaying.
                return Ok(changes
                    .into_iter()
                    .map(|change| {
                        GraphQLReconcileOutcome::rejected(
                            change.local_id,
                            GraphQLRejectionReason::PermissionDenied,
                        )
                    })
                    .collect());
            }
        };
        let role = role_from_membership(&member_role);
        let reconnect_seq = take_reconnect_seq(world_id);

        let mut outcomes = Vec::with_capacity(changes.len());
        for change in changes {
            outcomes.push(apply_one(
                &mut conn,
                world_id,
                user_id,
                role,
                reconnect_seq,
                change,
            ));
        }
        Ok(outcomes)
    }
}

/// Adjudicate and apply one queued change, always answering with an outcome.
fn apply_one(
    conn: &mut PgConnection,
    world_id: Uuid,
    user_id: Uuid,
    role: Role,
    reconnect_seq: ReconnectSeq,
    change: QueuedChangeInput,
) -> GraphQLReconcileOutcome {
    let local_id = change.local_id;

    // FR-061/FR-061a. Who this change is *for*: the submitter unless they
    // named someone else, and naming someone else is a GM's prerogative.
    let subject_user = change.attributed_to_user_id.unwrap_or(user_id);
    if subject_user != user_id && !matches!(role, Role::GameMaster) {
        // The whole of the trust model's negative case. A GM relaying a
        // player's peer-adjudicated change is expected and unremarkable; a
        // player claiming to relay someone else's is the one thing this
        // mutation refuses on attribution grounds.
        return GraphQLReconcileOutcome::rejected(
            local_id,
            GraphQLRejectionReason::PermissionDenied,
        );
    }

    // Disclosure is computed for the GM and only the GM (FR-067): the response
    // goes to the submitter, and a peer-adjudicated batch is submitted over
    // the GM's own connection by design, so this is the one place it can be
    // shown without also showing it to the players it is about.
    //
    // Computed before anything is applied, and carried onto whatever outcome
    // results, so that it can never be mistaken for a reason the change did
    // or did not stand (FR-066).
    let discrepancy = match role {
        Role::GameMaster => detect_discrepancy(
            conn,
            world_id,
            subject_user,
            change.reported_outcome.as_ref(),
        ),
        Role::Player => None,
    };
    if let Some(found) = &discrepancy {
        // Recorded where the deployment can see it and nowhere else — no
        // telemetry leaves the device or the deployment (FR-067/FR-052), and
        // deliberately not a `world_events` row, which every member of the
        // world would receive.
        tracing::info!(
            world_id = %world_id,
            user_id = %found.user_id,
            record_id = %found.record_id,
            reported_value = found.reported_value,
            determined_value = found.determined_value,
            "reconciled outcome differs from the value the server determined"
        );
    }

    let Some(edit) = parse_token_edit(&change.command.0) else {
        return GraphQLReconcileOutcome::rejected(local_id, GraphQLRejectionReason::Invalid)
            .disclosing(discrepancy);
    };

    use crate::schema::tokens;
    let existing = tokens::table
        .filter(tokens::token_id.eq(edit.token_id))
        .select(crate::models::Token::as_select())
        .first::<crate::models::Token>(conn)
        .optional();

    let existing = match existing {
        Ok(Some(token)) => token,
        // Deleted server-side while the client was away. Discarded with a
        // reason rather than resurrected — recreating something someone
        // deliberately removed is the failure FR-035a's create/delete
        // restriction exists to avoid, and it would be perverse to do it here.
        Ok(None) => {
            return GraphQLReconcileOutcome::rejected(local_id, GraphQLRejectionReason::GoneAway)
                .disclosing(discrepancy);
        }
        Err(_) => {
            return GraphQLReconcileOutcome::rejected(local_id, GraphQLRejectionReason::Invalid)
                .disclosing(discrepancy);
        }
    };

    // The second half of FR-042. A Game Master may move anything in their
    // world; a player may move only what they own. Same rule `move_own_token`
    // applies to a live edit, asked again at reconnect time.
    let may_edit = match role {
        Role::GameMaster => true,
        Role::Player => existing.owner_user_id == Some(user_id),
    };
    if !may_edit {
        return GraphQLReconcileOutcome::rejected(
            local_id,
            GraphQLRejectionReason::PermissionDenied,
        )
        .disclosing(discrepancy);
    }

    // FR-040: who wins. `conflict::resolve` is shared with the client, which
    // predicts with it so the UI can say what will happen — the two answers
    // must never differ, which is why nothing here reimplements the rule.
    let incoming = Contender {
        role,
        reconnect_seq,
    };
    if let Some(previous) = existing_mark(world_id, edit.token_id) {
        let held = Contender {
            role: previous.role,
            reconnect_seq: previous.reconnect_seq,
        };
        if matches!(conflict::resolve(held, incoming), Winner::A) {
            return GraphQLReconcileOutcome::superseded(local_id, previous.role)
                .disclosing(discrepancy);
        }
    }

    let update = crate::models::TokenUpdate {
        actor_id: None,
        x: edit.x,
        y: edit.y,
        rotation: edit.rotation,
        scale: edit.scale,
        metadata: None,
        owner_user_id: None,
        is_primary: None,
        photo_url: None,
        // Not offline-editable (FR-035a), and explicitly `None` rather than
        // carried from the command: health is adjudicated by the ruleset, and
        // a client that queued a stale value while disconnected must not be
        // able to write it back hours later.
        health: None,
        max_health: None,
    };

    let updated = diesel::update(tokens::table.filter(tokens::token_id.eq(edit.token_id)))
        .set(&update)
        .execute(conn);

    if updated.is_err() {
        return GraphQLReconcileOutcome::rejected(local_id, GraphQLRejectionReason::Invalid)
            .disclosing(discrepancy);
    }

    remember_mark(world_id, edit.token_id, role, reconnect_seq);

    // An ordinary world event, deliberately: other clients learn about a
    // reconciled change through the subscription they already have, with no
    // special path to keep in step with the normal one.
    //
    // `reconciled`, `by_user` and `by_role` are what make the
    // `Applied → Superseded` case detectable on the client (FR-041). A player
    // whose change applied at their own reconnect is long gone from that call
    // by the time a GM reconnects and overrides it, so the only way they learn
    // is this event — and to recognise it as *their* work being overridden
    // rather than ordinary table activity, they need to know the change was a
    // replay and that somebody else made it.
    let _ = record_world_event(
        conn,
        world_id,
        EVENT_CODE_TOKEN_CHANGED,
        Some(serde_json::json!({
            "token_id": edit.token_id,
            "reconciled": true,
            "by_user": subject_user,
            "by_role": role_name(role),
            // Present only when the change was relayed on someone else's
            // behalf, so the ordinary case emits exactly the payload it
            // always did. It says who relayed it, never that anything was
            // wrong with it: a GM acting for a player is unremarkable and is
            // flagged to nobody (FR-061b).
            "submitted_by": (subject_user != user_id).then_some(user_id),
        })),
        user_id,
    );

    GraphQLReconcileOutcome::accepted(local_id).disclosing(discrepancy)
}

fn existing_mark(world_id: Uuid, token_id: Uuid) -> Option<Mark> {
    marks()
        .lock()
        .ok()?
        .marks
        .get(&(world_id, token_id))
        .copied()
}

fn remember_mark(world_id: Uuid, token_id: Uuid, role: Role, reconnect_seq: ReconnectSeq) {
    if let Ok(mut state) = marks().lock() {
        state.marks.insert(
            (world_id, token_id),
            Mark {
                role,
                reconnect_seq,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn command(kind: &str, id: Uuid, x: Option<f64>) -> serde_json::Value {
        serde_json::json!({
            "type": kind,
            "token": { "id": id, "x": x, "y": 2.0 },
        })
    }

    #[test]
    fn a_token_move_is_understood() {
        let id = Uuid::now_v7();
        let edit = parse_token_edit(&command("upsert_token", id, Some(1.0)))
            .expect("a token move is exactly what this replays");

        assert_eq!(edit.token_id, id);
        assert_eq!(edit.x, Some(1.0));
        assert_eq!(edit.y, Some(2.0));
    }

    /// FR-035a. Anything that is not a token move/manipulate is refused, and
    /// refusal has to be a *rejection* rather than a skip — a skipped input
    /// would produce no outcome, which is the silent loss FR-041 prohibits.
    #[test]
    fn a_command_that_is_not_a_token_edit_is_refused() {
        let id = Uuid::now_v7();
        assert!(parse_token_edit(&command("remove_token", id, Some(1.0))).is_none());
        assert!(parse_token_edit(&serde_json::json!({"type": "create_wall"})).is_none());
        assert!(parse_token_edit(&serde_json::json!("not even an object")).is_none());
    }

    /// A command naming a token and changing nothing is not an edit. Applying
    /// it would take the conflict mark for an item nobody actually moved,
    /// which would then supersede someone's real change.
    #[test]
    fn an_edit_that_changes_nothing_is_refused() {
        let id = Uuid::now_v7();
        assert!(
            parse_token_edit(&serde_json::json!({
                "type": "upsert_token",
                "token": { "id": id },
            }))
            .is_none()
        );
    }

    /// Rotation and scale are permitted alongside position (FR-035a), so a
    /// manipulate replays as readily as a move.
    #[test]
    fn rotation_and_scale_are_permitted() {
        let id = Uuid::now_v7();
        let edit = parse_token_edit(&serde_json::json!({
            "type": "upsert_token",
            "token": { "id": id, "rotation": 90.0, "scale": 2.0 },
        }))
        .expect("rotation and scale are offline-editable");

        assert_eq!(edit.rotation, Some(90.0));
        assert_eq!(edit.scale, Some(2.0));
        assert_eq!(edit.x, None);
    }

    /// Owner and GM both run the table. A player is anything else, including
    /// a role string this server does not recognise — the safe reading of an
    /// unknown role is the least privileged one.
    #[test]
    fn owner_and_gm_are_both_game_masters() {
        assert_eq!(role_from_membership("Owner"), Role::GameMaster);
        assert_eq!(role_from_membership("GM"), Role::GameMaster);
        assert_eq!(role_from_membership("Player"), Role::Player);
        assert_eq!(role_from_membership("something-new"), Role::Player);
    }

    fn insert_token(conn: &mut PgConnection, scene_id: Uuid, owner: Option<Uuid>) -> Uuid {
        use crate::schema::tokens;
        let token_id = Uuid::now_v7();
        let now = chrono::Utc::now().naive_utc();
        diesel::insert_into(tokens::table)
            .values((
                tokens::token_id.eq(token_id),
                tokens::scene_id.eq(scene_id),
                tokens::x.eq(0.0_f64),
                tokens::y.eq(0.0_f64),
                tokens::owner_user_id.eq(owner),
                tokens::created_at.eq(now),
                tokens::updated_at.eq(now),
            ))
            .execute(conn)
            .expect("failed to insert test token");
        token_id
    }

    fn move_command(token_id: Uuid, x: f64) -> QueuedChangeInput {
        QueuedChangeInput {
            local_id: Uuid::now_v7().to_string(),
            command: async_graphql::Json(serde_json::json!({
                "type": "upsert_token",
                "token": { "id": token_id, "x": x, "y": 0.0 },
            })),
            attributed_to_user_id: None,
            reported_outcome: None,
        }
    }

    /// A roll the server itself resolved and recorded (ADR-044) — the only
    /// thing discrepancy detection is ever allowed to compare against.
    fn insert_roll_record(conn: &mut PgConnection, world_id: Uuid, by: Uuid, value: f64) -> Uuid {
        use crate::schema::world_roll_records;
        diesel::insert_into(world_roll_records::table)
            .values(&crate::models::NewRollRecord {
                world_id,
                triggered_by: by,
                formula: "1d20".to_string(),
                bindings: None,
                detail: serde_json::json!({}),
                result_kind: "total".to_string(),
                result_value: value,
            })
            .returning(world_roll_records::id)
            .get_result::<Uuid>(conn)
            .expect("failed to insert test roll record")
    }

    fn dice_report(record_id: Uuid, value: f64) -> ReportedOutcomeInput {
        ReportedOutcomeInput {
            kind: REPORTED_OUTCOME_KIND_DICE.to_string(),
            version: REPORTED_OUTCOME_VERSION,
            record_id: Some(record_id),
            value: Some(value),
        }
    }

    fn token_x(conn: &mut PgConnection, token_id: Uuid) -> f64 {
        use crate::schema::tokens;
        tokens::table
            .filter(tokens::token_id.eq(token_id))
            .select(tokens::x)
            .first::<f64>(conn)
            .expect("token should exist")
    }

    /// T078/FR-041: every submitted change gets exactly one outcome, whatever
    /// happened to it. Silent loss is the prohibited failure, and the way it
    /// would appear is an input with no matching outcome.
    #[test]
    fn every_change_gets_exactly_one_outcome_including_the_bad_ones() {
        let state = crate::test_support::test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner = crate::test_support::insert_test_user(&mut conn);
        let world = crate::test_support::insert_test_world(&mut conn, owner);
        let scene = crate::test_support::insert_test_scene(&mut conn, world, owner);
        let token = insert_token(&mut conn, scene, Some(owner));
        let gone = Uuid::now_v7();

        let changes = vec![
            move_command(token, 5.0),
            move_command(gone, 7.0),
            QueuedChangeInput {
                local_id: Uuid::now_v7().to_string(),
                command: async_graphql::Json(serde_json::json!({"type": "delete_token"})),
                attributed_to_user_id: None,
                reported_outcome: None,
            },
        ];
        let submitted: Vec<String> = changes.iter().map(|c| c.local_id.clone()).collect();

        let outcomes: Vec<GraphQLReconcileOutcome> = changes
            .into_iter()
            .map(|change| {
                apply_one(
                    &mut conn,
                    world,
                    owner,
                    Role::GameMaster,
                    take_reconnect_seq(world),
                    change,
                )
            })
            .collect();

        assert_eq!(outcomes.len(), submitted.len());
        let answered: Vec<String> = outcomes.iter().map(|o| o.local_id.clone()).collect();
        assert_eq!(
            answered, submitted,
            "outcomes must match inputs one for one"
        );

        assert!(outcomes[0].applied, "a valid move by the owner applies");
        assert_eq!(
            outcomes[1].reason,
            Some(GraphQLRejectionReason::GoneAway),
            "a token deleted server-side is discarded with a reason, not resurrected"
        );
        assert_eq!(
            outcomes[2].reason,
            Some(GraphQLRejectionReason::Invalid),
            "a command outside FR-035a is rejected, never skipped"
        );
    }

    /// FR-042: a player may replay a move of their own token, and not of
    /// someone else's — asked at reconnect time, not at edit time.
    #[test]
    fn a_player_may_only_replay_moves_of_their_own_token() {
        let state = crate::test_support::test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner = crate::test_support::insert_test_user(&mut conn);
        let player = crate::test_support::insert_test_user(&mut conn);
        let world = crate::test_support::insert_test_world(&mut conn, owner);
        let scene = crate::test_support::insert_test_scene(&mut conn, world, owner);
        let theirs = insert_token(&mut conn, scene, Some(player));
        let someone_elses = insert_token(&mut conn, scene, Some(owner));

        let mine = apply_one(
            &mut conn,
            world,
            player,
            Role::Player,
            take_reconnect_seq(world),
            move_command(theirs, 3.0),
        );
        let not_mine = apply_one(
            &mut conn,
            world,
            player,
            Role::Player,
            take_reconnect_seq(world),
            move_command(someone_elses, 3.0),
        );

        assert!(mine.applied);
        assert_eq!(
            not_mine.reason,
            Some(GraphQLRejectionReason::PermissionDenied)
        );
        assert_eq!(
            token_x(&mut conn, someone_elses),
            0.0,
            "a refused replay must not have written anything"
        );
    }

    /// FR-040, the sharp edge: a player reconnects first and their change
    /// applies; the GM reconnects later with a conflicting edit and takes
    /// precedence. The GM's change is what stands.
    #[test]
    fn a_game_master_reconnecting_later_still_wins() {
        let state = crate::test_support::test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let gm = crate::test_support::insert_test_user(&mut conn);
        let player = crate::test_support::insert_test_user(&mut conn);
        let world = crate::test_support::insert_test_world(&mut conn, gm);
        let scene = crate::test_support::insert_test_scene(&mut conn, world, gm);
        let token = insert_token(&mut conn, scene, Some(player));

        let player_first = apply_one(
            &mut conn,
            world,
            player,
            Role::Player,
            take_reconnect_seq(world),
            move_command(token, 10.0),
        );
        assert!(
            player_first.applied,
            "the player got back first and applied"
        );
        assert_eq!(token_x(&mut conn, token), 10.0);

        let gm_later = apply_one(
            &mut conn,
            world,
            gm,
            Role::GameMaster,
            take_reconnect_seq(world),
            move_command(token, 20.0),
        );

        assert!(gm_later.applied, "a GM beats a player regardless of order");
        assert_eq!(token_x(&mut conn, token), 20.0);
    }

    /// And the other direction, which is the case the UI has to explain: the
    /// GM reconnected first, so the player's queued edit is superseded rather
    /// than failing. `supersededByRole` is what lets the client say who won
    /// instead of reporting an anonymous error.
    #[test]
    fn a_player_losing_to_a_game_master_is_told_who_won() {
        let state = crate::test_support::test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let gm = crate::test_support::insert_test_user(&mut conn);
        let player = crate::test_support::insert_test_user(&mut conn);
        let world = crate::test_support::insert_test_world(&mut conn, gm);
        let scene = crate::test_support::insert_test_scene(&mut conn, world, gm);
        let token = insert_token(&mut conn, scene, Some(player));

        apply_one(
            &mut conn,
            world,
            gm,
            Role::GameMaster,
            take_reconnect_seq(world),
            move_command(token, 42.0),
        );

        let player_late = apply_one(
            &mut conn,
            world,
            player,
            Role::Player,
            take_reconnect_seq(world),
            move_command(token, 99.0),
        );

        assert!(!player_late.applied);
        assert_eq!(
            player_late.reason,
            Some(GraphQLRejectionReason::Superseded),
            "losing to a GM is a working rule, not an error"
        );
        assert_eq!(
            player_late.superseded_by_role.as_deref(),
            Some("GameMaster"),
            "the UI has to be able to say who won"
        );
        assert_eq!(token_x(&mut conn, token), 42.0, "the GM's value stands");
    }

    /// The sequence is per world and monotonic. Two worlds reconnecting do
    /// not share an ordering, and one world's sequence never repeats — a
    /// repeat would make two different reconnections tie, and
    /// `conflict::resolve` breaks a tie by favouring the *held* side, so the
    /// later client would lose without a rule ever having said so.
    #[test]
    fn reconnect_sequences_are_per_world_and_increasing() {
        let world_a = Uuid::now_v7();
        let world_b = Uuid::now_v7();

        let first = take_reconnect_seq(world_a);
        let second = take_reconnect_seq(world_a);
        let other = take_reconnect_seq(world_b);

        assert!(second > first, "a world's sequence must advance");
        assert_eq!(other, 0, "a different world starts its own ordering");
    }

    /// FR-061, the whole of the positive case: the GM's client relays a change
    /// a player originated while the server was unreachable, and the only
    /// question asked is whether the submitter runs this table. If this ever
    /// starts demanding an attestation, peer-adjudicated play has stopped
    /// working for the party the design trusts.
    #[test]
    fn a_game_master_may_submit_a_change_a_player_originated() {
        let state = crate::test_support::test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let gm = crate::test_support::insert_test_user(&mut conn);
        let player = crate::test_support::insert_test_user(&mut conn);
        let world = crate::test_support::insert_test_world(&mut conn, gm);
        let scene = crate::test_support::insert_test_scene(&mut conn, world, gm);
        let token = insert_token(&mut conn, scene, Some(player));

        let mut change = move_command(token, 12.0);
        change.attributed_to_user_id = Some(player);

        let outcome = apply_one(
            &mut conn,
            world,
            gm,
            Role::GameMaster,
            take_reconnect_seq(world),
            change,
        );

        assert!(outcome.applied, "the GM is the trusted party by design");
        assert_eq!(token_x(&mut conn, token), 12.0);
    }

    /// SC-021/FR-061a, and the reason the previous test is not simply a hole:
    /// the *identical* submission from someone who does not run the table is
    /// refused. This is the one thing attribution is checked for.
    #[test]
    fn a_player_may_not_submit_a_change_attributed_to_someone_else() {
        let state = crate::test_support::test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let gm = crate::test_support::insert_test_user(&mut conn);
        let player = crate::test_support::insert_test_user(&mut conn);
        let other = crate::test_support::insert_test_user(&mut conn);
        let world = crate::test_support::insert_test_world(&mut conn, gm);
        let scene = crate::test_support::insert_test_scene(&mut conn, world, gm);
        // Owned by the submitter, so ownership cannot be what refuses this —
        // only the attribution can.
        let token = insert_token(&mut conn, scene, Some(player));

        let mut change = move_command(token, 12.0);
        change.attributed_to_user_id = Some(other);

        let outcome = apply_one(
            &mut conn,
            world,
            player,
            Role::Player,
            take_reconnect_seq(world),
            change,
        );

        assert!(!outcome.applied);
        assert_eq!(
            outcome.reason,
            Some(GraphQLRejectionReason::PermissionDenied)
        );
        assert_eq!(
            token_x(&mut conn, token),
            0.0,
            "a refused attribution must not have written anything"
        );
    }

    /// FR-064/FR-065/FR-066 together, and the ordering matters: the change is
    /// **applied**, and the difference is disclosed alongside it with both
    /// numbers. A test that only checked the flag would pass on an
    /// implementation that rejected the change, which is the outcome the ADR
    /// specifically rules out.
    #[test]
    fn a_reported_value_the_server_determined_differently_is_applied_and_disclosed() {
        let state = crate::test_support::test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let gm = crate::test_support::insert_test_user(&mut conn);
        let player = crate::test_support::insert_test_user(&mut conn);
        let world = crate::test_support::insert_test_world(&mut conn, gm);
        let scene = crate::test_support::insert_test_scene(&mut conn, world, gm);
        let token = insert_token(&mut conn, scene, Some(player));
        let record = insert_roll_record(&mut conn, world, player, 7.0);

        let mut change = move_command(token, 3.0);
        change.attributed_to_user_id = Some(player);
        change.reported_outcome = Some(dice_report(record, 20.0));

        let outcome = apply_one(
            &mut conn,
            world,
            gm,
            Role::GameMaster,
            take_reconnect_seq(world),
            change,
        );

        assert!(
            outcome.applied,
            "a discrepancy never rejects, interrupts, or alters the outcome"
        );
        assert_eq!(token_x(&mut conn, token), 3.0);

        let found = outcome
            .discrepancy
            .expect("a genuine determined-value mismatch is what disclosure exists for");
        assert_eq!(
            found.user_id, player,
            "whose result it was, not who relayed it"
        );
        assert_eq!(found.record_id, record);
        assert_eq!(found.reported_value, 20.0);
        assert_eq!(
            found.determined_value, 7.0,
            "both numbers, so the GM can look"
        );
    }

    /// FR-061b. A GM acting on a player's behalf is table authority being
    /// exercised, not an attack: it produces no flag and notifies nobody. The
    /// failure this catches is treating attribution *itself* as suspicious,
    /// which would put every relayed change under a cloud.
    #[test]
    fn a_game_master_acting_on_a_players_behalf_produces_no_flag() {
        let state = crate::test_support::test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let gm = crate::test_support::insert_test_user(&mut conn);
        let player = crate::test_support::insert_test_user(&mut conn);
        let world = crate::test_support::insert_test_world(&mut conn, gm);
        let scene = crate::test_support::insert_test_scene(&mut conn, world, gm);
        let token = insert_token(&mut conn, scene, Some(player));
        let record = insert_roll_record(&mut conn, world, player, 15.0);

        let mut change = move_command(token, 4.0);
        change.attributed_to_user_id = Some(player);
        change.reported_outcome = Some(dice_report(record, 15.0));

        let outcome = apply_one(
            &mut conn,
            world,
            gm,
            Role::GameMaster,
            take_reconnect_seq(world),
            change,
        );

        assert!(outcome.applied);
        assert!(
            outcome.discrepancy.is_none(),
            "agreeing values are the silent case, exactly like no determination"
        );
    }

    /// FR-068. Ordinary token movement — the overwhelming majority of what
    /// this mutation carries — has no independent basis, so there is nothing
    /// to detect and nothing to say. Absence of evidence is not a flag.
    #[test]
    fn an_ordinary_token_move_reports_no_discrepancy() {
        let state = crate::test_support::test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let gm = crate::test_support::insert_test_user(&mut conn);
        let world = crate::test_support::insert_test_world(&mut conn, gm);
        let scene = crate::test_support::insert_test_scene(&mut conn, world, gm);
        let token = insert_token(&mut conn, scene, Some(gm));

        let outcome = apply_one(
            &mut conn,
            world,
            gm,
            Role::GameMaster,
            take_reconnect_seq(world),
            move_command(token, 6.0),
        );

        assert!(outcome.applied);
        assert!(outcome.discrepancy.is_none());
    }

    /// FR-067. Disclosure is the GM's and nobody else's. A player replaying
    /// their own queued work is told what became of it and is never handed a
    /// flag about themselves — and, since a world event reaches every member,
    /// the discrepancy deliberately never travels that way either.
    #[test]
    fn a_player_submitter_is_shown_no_discrepancy() {
        let state = crate::test_support::test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let gm = crate::test_support::insert_test_user(&mut conn);
        let player = crate::test_support::insert_test_user(&mut conn);
        let world = crate::test_support::insert_test_world(&mut conn, gm);
        let scene = crate::test_support::insert_test_scene(&mut conn, world, gm);
        let token = insert_token(&mut conn, scene, Some(player));
        let record = insert_roll_record(&mut conn, world, player, 2.0);

        let mut change = move_command(token, 8.0);
        change.reported_outcome = Some(dice_report(record, 19.0));

        let outcome = apply_one(
            &mut conn,
            world,
            player,
            Role::Player,
            take_reconnect_seq(world),
            change,
        );

        assert!(outcome.applied);
        assert!(outcome.discrepancy.is_none());
    }

    /// FR-067a, the expensive failure: a database error — a statement timeout
    /// above all — means the server did not determine anything. Reading that
    /// as "the server says otherwise" would accuse someone because a query
    /// was slow.
    #[test]
    fn a_lookup_that_failed_determined_nothing() {
        assert!(
            determination_from_lookup(Err(diesel::result::Error::BrokenTransactionManager))
                .is_none()
        );
    }

    /// The same for a row that is not there — a roll the server has no record
    /// of, or one belonging to another world. Nothing to compare is nothing to
    /// report.
    #[test]
    fn a_missing_record_determined_nothing() {
        assert!(determination_from_lookup(Ok(None)).is_none());
        assert!(determination_from_lookup(Ok(Some(f64::NAN))).is_none());
    }

    /// A reported outcome the server cannot read is a parse failure, and a
    /// parse failure is an ambiguity. None of these may ever become a claimed
    /// value the server then "disagrees" with.
    #[test]
    fn an_unreadable_report_is_never_a_mismatch() {
        let record = Uuid::now_v7();

        let mut unknown_kind = dice_report(record, 20.0);
        unknown_kind.kind = "vibes".to_string();
        assert!(parse_reported_outcome(&unknown_kind).is_none());

        let mut no_record = dice_report(record, 20.0);
        no_record.record_id = None;
        assert!(parse_reported_outcome(&no_record).is_none());

        let mut no_value = dice_report(record, 20.0);
        no_value.value = None;
        assert!(parse_reported_outcome(&no_value).is_none());

        assert!(
            parse_reported_outcome(&dice_report(record, f64::NAN)).is_none(),
            "a value that is not a number cannot be compared to one"
        );
    }

    /// A client one release ahead of this server is reporting in a format we
    /// cannot read. That is a version mismatch, which is an ambiguity — the
    /// worst possible response is to compare the fields we recognise and
    /// accuse whoever upgraded first.
    #[test]
    fn a_report_in_a_format_this_server_does_not_know_is_never_a_mismatch() {
        let mut ahead = dice_report(Uuid::now_v7(), 20.0);
        ahead.version = REPORTED_OUTCOME_VERSION + 1;
        assert!(parse_reported_outcome(&ahead).is_none());
    }

    /// The structural claim the module doc makes, asserted directly: with no
    /// determination in hand there is no discrepancy, whatever was claimed —
    /// the same answer agreement gives. If these two ever diverge, every
    /// ambiguity above becomes an accusation.
    #[test]
    fn no_determination_and_agreement_are_the_same_silence() {
        let reported = ReportedOutcome {
            record_id: Uuid::now_v7(),
            claimed: 20.0,
        };
        let user = Uuid::now_v7();

        assert!(discrepancy_between(user, &reported, None).is_none());
        assert!(discrepancy_between(user, &reported, Some(DeterminedValue(20.0))).is_none());
        assert!(
            discrepancy_between(user, &reported, Some(DeterminedValue(20.0 + 1e-12))).is_none(),
            "a float round trip is not a discrepancy"
        );
        assert!(
            discrepancy_between(user, &reported, Some(DeterminedValue(7.0))).is_some(),
            "and a genuine mismatch still gets reported, or none of this is worth having"
        );
    }
}
