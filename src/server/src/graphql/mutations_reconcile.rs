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
        // Same reasoning as health: what a token *is* is not something an
        // offline client reclassifies. Only position and pose replay.
        token_type: None,
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
#[path = "mutations_reconcile_tests.rs"]
mod tests;
