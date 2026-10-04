//! What the counting costs, rung by rung — and, at the last rung, the window.
//!
//! Spec 039 US5 and US7, `contracts/standing-and-termination.md`,
//! data-model.md § 5.
//!
//! # Derived, except for the window
//!
//! Strikes, whether an account may publish, and whether it is disabled are
//! computed on every read from `content_moderation_actions` through
//! [`super::strikes_of`] — the same definition `repeatInfringerFlags` uses, so
//! the ladder and the flag cannot disagree about what a strike is (FR-027).
//! Nothing has to notice a strike ageing past the lookback: the calendar moved,
//! the count changed (FR-035). Restoration follows the existing process — a
//! case resolved in the person's favour stops counting — rather than a database
//! edit (FR-020).
//!
//! The one thing that is stored is a termination window in
//! `account_terminations`, because a window cannot be derived: when it opened,
//! when deletion falls due, and what was promised at the start of it.
//!
//! # The ladder is a value, not an ambient read
//!
//! [`Ladder::from_env`] reads the settings once; everything below takes a
//! [`Ladder`]. A test states the ladder it means rather than depending on what
//! the process environment happens to hold, which is the shared-global-state
//! trap this crate has already paid for twice.
//!
//! # The sweep is a backstop, not the mechanism
//!
//! A termination opens the moment the third strike is recorded
//! ([`tell_of_strike_sync`]), not at the next tick. [`run_due_standing_work`]
//! catches what the moment missed — an account that crossed the threshold by
//! some other route, a strike that aged out, a window that ran out — and is
//! called from the periodic task, from the admin moderation query, and from the
//! sign-in of an account with an open window, so somebody signing in on day
//! thirty-one sees the truth rather than a stale window.

use chrono::{DateTime, Utc};
use diesel::prelude::*;
use serde_json::json;
use uuid::Uuid;

use super::Strike;
use crate::models::{
    AccountTermination, ContentModerationAction, NewAccountTermination, NewContentModerationAction,
};
use crate::notices;
use crate::schema::{account_terminations, content_moderation_actions, users};
use crate::state::AppState;

/// Config: the strike at which the person is warned. A notice; publishing is
/// unaffected.
pub fn strike_warn_at() -> i64 {
    std::env::var("MODERATION_STRIKE_WARN_AT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1)
}

/// Config: the strike at which publishing is suspended (FR-018). Play, edit and
/// read are untouched (FR-019).
pub fn strike_suspend_publishing_at() -> i64 {
    std::env::var("MODERATION_STRIKE_SUSPEND_PUBLISHING_AT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2)
}

/// Config: days between disablement and deletion (FR-030). Snapshotted into a
/// window when it opens.
pub fn termination_window_days() -> i64 {
    std::env::var("MODERATION_TERMINATION_WINDOW_DAYS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(30)
}

/// Config: whether the end of the window waits for a person rather than a
/// timer. **Defaults to true** — a self-hosted table of six friends should not
/// have a timer that deletes one of them.
pub fn termination_requires_human() -> bool {
    std::env::var("MODERATION_TERMINATION_REQUIRES_HUMAN")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(true)
}

/// The rungs in force, read once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ladder {
    pub warn_at: i64,
    pub suspend_publishing_at: i64,
    /// The existing `MODERATION_REPEAT_INFRINGER_THRESHOLD`: disablement.
    pub threshold: i64,
    pub lookback_days: i64,
    pub window_days: i64,
    pub requires_human: bool,
}

impl Ladder {
    pub fn from_env() -> Self {
        Self {
            warn_at: strike_warn_at(),
            suspend_publishing_at: strike_suspend_publishing_at(),
            threshold: super::repeat_infringer_threshold(),
            lookback_days: super::repeat_infringer_lookback_days(),
            window_days: termination_window_days(),
            requires_human: termination_requires_human(),
        }
    }

    /// When a strike recorded at `recorded_at` stops counting — exactly when
    /// the existing lookback stops including it.
    pub fn ages_out_at(&self, recorded_at: DateTime<Utc>) -> DateTime<Utc> {
        recorded_at + chrono::Duration::days(self.lookback_days)
    }
}

/// `account_terminations.appeal_state`, mirrored by the migration's CHECK.
pub mod appeal_state {
    pub const NONE: &str = "none";
    pub const OPEN: &str = "open";
    pub const UPHELD: &str = "upheld";
    pub const REJECTED: &str = "rejected";
}

/// `account_terminations.closed_reason`, mirrored by the migration's CHECK.
pub mod closed_reason {
    pub const APPEAL_UPHELD: &str = "appeal_upheld";
    pub const STRIKES_AGED_OUT: &str = "strikes_aged_out";
    /// The account filed a counter-notice, and with that case no longer
    /// counting fell below the threshold — the statutory route back, which a
    /// disabled account keeps (decided 2026-09-10).
    pub const COUNTER_NOTICE: &str = "counter_notice";
    pub const DELETED: &str = "deleted";
    pub const ADMIN_REVERSED: &str = "admin_reversed";
}

/// Where one account stands.
#[derive(Debug, Clone)]
pub struct Standing {
    /// Oldest first.
    pub strikes: Vec<Strike>,
    pub ladder: Ladder,
    pub may_publish: bool,
    /// An open termination disables this account. Not the same as "an open
    /// termination exists": the last administrator's is written and does not
    /// disable (FR-039).
    pub disabled: bool,
    pub termination: Option<AccountTermination>,
}

impl Standing {
    /// The ladder with no window open — the whole of US5's logic, with no
    /// database in it.
    pub fn from_strikes(ladder: Ladder, strikes: Vec<Strike>) -> Self {
        Self::with_termination(ladder, strikes, None)
    }

    /// Publishing stops at the suspension rung **or** the threshold, whichever
    /// comes first — an operator who sets the suspension rung above the
    /// threshold has not thereby let an account at the threshold publish — and
    /// while any window is open.
    pub fn with_termination(
        ladder: Ladder,
        strikes: Vec<Strike>,
        termination: Option<AccountTermination>,
    ) -> Self {
        let count = strikes.len() as i64;
        Self {
            may_publish: count < ladder.suspend_publishing_at
                && count < ladder.threshold
                && termination.is_none(),
            disabled: termination.as_ref().is_some_and(|t| t.disables_account),
            strikes,
            ladder,
            termination,
        }
    }

    pub fn strike_count(&self) -> i64 {
        self.strikes.len() as i64
    }

    /// At or past the warning rung.
    pub fn warned(&self) -> bool {
        self.strike_count() >= self.ladder.warn_at
    }
}

fn open_termination_of(
    conn: &mut PgConnection,
    account_id: Uuid,
) -> QueryResult<Option<AccountTermination>> {
    account_terminations::table
        .filter(account_terminations::account_id.eq(account_id))
        .filter(account_terminations::closed_at.is_null())
        .select(AccountTermination::as_select())
        .first(conn)
        .optional()
}

pub fn standing_of_sync(
    conn: &mut PgConnection,
    account_id: Uuid,
    ladder: Ladder,
) -> QueryResult<Standing> {
    let strikes = super::strikes_of(conn, account_id, ladder.lookback_days)?;
    let termination = open_termination_of(conn, account_id)?;
    Ok(Standing::with_termination(ladder, strikes, termination))
}

/// One account's standing under the ladder in force.
pub async fn standing_of(state: &AppState, account_id: Uuid) -> Result<Standing, String> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection".to_string())?;
    let ladder = Ladder::from_env();

    tokio::task::spawn_blocking(move || standing_of_sync(&mut conn, account_id, ladder))
        .await
        .map_err(|_| "Failed to spawn blocking task".to_string())?
        .map_err(|_| "Failed to read the account's standing".to_string())
}

/// Is this account disabled right now? The one question every authenticated
/// request asks, so it is one indexed `EXISTS` and nothing more.
pub fn is_disabled_sync(conn: &mut PgConnection, account_id: Uuid) -> QueryResult<bool> {
    diesel::select(diesel::dsl::exists(
        account_terminations::table
            .filter(account_terminations::account_id.eq(account_id))
            .filter(account_terminations::closed_at.is_null())
            .filter(account_terminations::disables_account.eq(true)),
    ))
    .get_result(conn)
}

/// The terms a window opens on: whether it disables the account, and whether a
/// person must decide its end.
///
/// FR-039, as a pure function so it can be tested without a database that has
/// exactly one administrator in it: the last administrator's window is written
/// with `requires_human = true` whatever the setting says, and does **not**
/// disable them — a product that locks itself out to enforce a rule has
/// enforced nothing and can no longer undo it.
pub fn termination_terms(ladder: Ladder, is_admin: bool, other_admins: i64) -> (bool, bool) {
    if is_admin && other_admins == 0 {
        (false, true)
    } else {
        (true, ladder.requires_human)
    }
}

/// Open a window for an account at or over the threshold, if none is open.
///
/// `None` when nothing was opened — below the threshold, or already open —
/// so calling it from both the strike path and the sweep is harmless.
pub fn open_termination_sync(
    conn: &mut PgConnection,
    account_id: Uuid,
    ladder: Ladder,
    now: DateTime<Utc>,
) -> QueryResult<Option<AccountTermination>> {
    if open_termination_of(conn, account_id)?.is_some() {
        return Ok(None);
    }
    let count = super::strikes_of(conn, account_id, ladder.lookback_days)?.len() as i64;
    if count < ladder.threshold {
        return Ok(None);
    }

    let is_admin = users::table
        .filter(users::id.eq(account_id))
        .select(users::is_admin)
        .first::<bool>(conn)
        .optional()?
        .unwrap_or(false);
    let other_admins: i64 = users::table
        .filter(users::is_admin.eq(true))
        .filter(users::id.ne(account_id))
        .count()
        .get_result(conn)?;
    let (disables_account, requires_human) = termination_terms(ladder, is_admin, other_admins);

    let opened = diesel::insert_into(account_terminations::table)
        .values(&NewAccountTermination {
            id: Uuid::now_v7(),
            account_id,
            opened_at: now,
            deletion_due_at: now + chrono::Duration::days(ladder.window_days),
            strike_count_at_open: count as i32,
            requires_human,
            disables_account,
        })
        .returning(AccountTermination::as_returning())
        .get_result(conn)?;

    // FR-030/FR-036: told at the **start** of the window what happens at the
    // end of it. The last administrator is not disabled, so is not told they
    // are; the administrators' view shows the window instead.
    if disables_account {
        notices::record_sync(
            conn,
            account_id,
            notices::kind::ACCOUNT_DISABLED,
            None,
            Some(json!({
                "strikeCount": count,
                "openedAt": opened.opened_at.to_rfc3339(),
                "deletionDueAt": opened.deletion_due_at.to_rfc3339(),
                "requiresHuman": opened.requires_human,
            })),
        )?;
    }
    Ok(Some(opened))
}

/// Close a window, and tell the person why when there is still a person.
fn close_termination_sync(
    conn: &mut PgConnection,
    termination: &AccountTermination,
    reason: &str,
    now: DateTime<Utc>,
) -> QueryResult<()> {
    diesel::update(account_terminations::table.filter(account_terminations::id.eq(termination.id)))
        .set((
            account_terminations::closed_at.eq(Some(now)),
            account_terminations::closed_reason.eq(Some(reason)),
        ))
        .execute(conn)?;

    let notice = match reason {
        closed_reason::STRIKES_AGED_OUT
        | closed_reason::COUNTER_NOTICE
        | closed_reason::ADMIN_REVERSED => {
            Some((notices::kind::ACCOUNT_RESTORED, json!({ "reason": reason })))
        }
        closed_reason::APPEAL_UPHELD => {
            Some((notices::kind::APPEAL_RESOLVED, json!({ "upheld": true })))
        }
        // Deleted: there is nobody left to tell.
        _ => None,
    };
    if let Some((kind, payload)) = notice
        && termination.disables_account
    {
        notices::record_sync(conn, termination.account_id, kind, None, Some(payload))?;
    }
    Ok(())
}

/// Whether a window's end has come and nothing stands in its way: past the
/// date, **no appeal open** (FR-034 — an appeal pauses the deletion, it does
/// not extend the window), and a window that actually disabled somebody.
pub fn is_due_for_deletion(termination: &AccountTermination, now: DateTime<Utc>) -> bool {
    termination.closed_at.is_none()
        && termination.disables_account
        && termination.appeal_state != appeal_state::OPEN
        && termination.deletion_due_at <= now
}

/// Carry out a window's end: real deletion (FR-036), then the row closed as
/// the record that it happened. One transaction.
///
/// What survives is stated in `contracts/standing-and-termination.md`: the
/// agreements with the name removed, the moderation cases, and this row —
/// nothing that names the person.
pub fn execute_termination_sync(
    conn: &mut PgConnection,
    termination: &AccountTermination,
    now: DateTime<Utc>,
) -> QueryResult<()> {
    conn.transaction(|conn| {
        crate::users::delete_user_data_on(conn, termination.account_id)?;
        close_termination_sync(conn, termination, closed_reason::DELETED, now)
    })
}

/// Whether an event of this kind happened on the account's cases since a
/// window opened.
fn happened_since(
    conn: &mut PgConnection,
    account_id: Uuid,
    action: &str,
    since: DateTime<Utc>,
) -> QueryResult<bool> {
    diesel::select(diesel::dsl::exists(
        content_moderation_actions::table
            .filter(content_moderation_actions::account_id.eq(account_id))
            .filter(content_moderation_actions::action_type.eq(action))
            .filter(content_moderation_actions::created_at.ge(since)),
    ))
    .get_result(conn)
}

/// Why an account fell below the threshold, so the closed window says so:
/// the person's own counter-notice, an administrator restoring a case, or the
/// calendar.
fn why_below_threshold(
    conn: &mut PgConnection,
    termination: &AccountTermination,
) -> QueryResult<&'static str> {
    let (account, since) = (termination.account_id, termination.opened_at);
    if happened_since(
        conn,
        account,
        super::action_type::COUNTER_NOTICE_RECEIVED,
        since,
    )? {
        Ok(closed_reason::COUNTER_NOTICE)
    } else if happened_since(conn, account, super::action_type::CONTENT_RESTORED, since)? {
        Ok(closed_reason::ADMIN_REVERSED)
    } else {
        Ok(closed_reason::STRIKES_AGED_OUT)
    }
}

/// What one sweep did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SweepReport {
    pub opened: usize,
    pub closed: usize,
    pub deleted: usize,
}

/// The sweep, given the ladder and the time. Idempotent.
///
/// 1. Opens a window for every account at the threshold without one.
/// 2. Closes every window whose account has fallen below the threshold —
///    restored without anybody asking (FR-035).
/// 3. Carries out every window that is due, unblocked and set to end without
///    a person (the snapshot, not today's setting). The rest wait in the
///    administrators' queue.
pub fn sweep_sync(
    conn: &mut PgConnection,
    ladder: Ladder,
    now: DateTime<Utc>,
) -> QueryResult<SweepReport> {
    sweep_scoped_sync(conn, ladder, now, None)
}

/// The sweep, narrowed to one account when `only` is given.
///
/// The narrowing exists for tests, and it is not a convenience: the database
/// is shared, and an unscoped sweep under a test's ladder would open windows
/// for — and disable — every account at the threshold in it.
pub fn sweep_scoped_sync(
    conn: &mut PgConnection,
    ladder: Ladder,
    now: DateTime<Utc>,
    only: Option<Uuid>,
) -> QueryResult<SweepReport> {
    let mut report = SweepReport::default();

    for (account_id, strikes) in super::strikes_by_account(conn, only, ladder.lookback_days)? {
        if strikes.len() as i64 >= ladder.threshold
            && open_termination_sync(conn, account_id, ladder, now)?.is_some()
        {
            report.opened += 1;
        }
    }

    let mut query = account_terminations::table
        .filter(account_terminations::closed_at.is_null())
        .into_boxed();
    if let Some(account_id) = only {
        query = query.filter(account_terminations::account_id.eq(account_id));
    }
    let open: Vec<AccountTermination> = query.select(AccountTermination::as_select()).load(conn)?;
    for termination in open {
        let count =
            super::strikes_of(conn, termination.account_id, ladder.lookback_days)?.len() as i64;
        if count < ladder.threshold {
            let reason = why_below_threshold(conn, &termination)?;
            close_termination_sync(conn, &termination, reason, now)?;
            report.closed += 1;
            continue;
        }
        if is_due_for_deletion(&termination, now) && !termination.requires_human {
            execute_termination_sync(conn, &termination, now)?;
            report.deleted += 1;
        }
    }
    Ok(report)
}

/// The sweep under the ladder in force, now.
pub async fn run_due_standing_work(state: &AppState) -> Result<SweepReport, String> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection".to_string())?;
    let ladder = Ladder::from_env();

    tokio::task::spawn_blocking(move || sweep_sync(&mut conn, ladder, Utc::now()))
        .await
        .map_err(|_| "Failed to spawn blocking task".to_string())?
        .map_err(|e| format!("Failed to sweep account standing: {e}"))
}

/// The sweep for one account, now — for the moment something about that
/// account's standing has just changed, so the person need not wait for the
/// tick to be restored.
pub async fn run_due_standing_work_for(
    state: &AppState,
    account_id: Uuid,
) -> Result<SweepReport, String> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection".to_string())?;
    let ladder = Ladder::from_env();

    tokio::task::spawn_blocking(move || {
        sweep_scoped_sync(&mut conn, ladder, Utc::now(), Some(account_id))
    })
    .await
    .map_err(|_| "Failed to spawn blocking task".to_string())?
    .map_err(|e| format!("Failed to sweep account standing: {e}"))
}

/// How often the backstop runs.
const TICK_SECONDS: u64 = 300;

/// Start the backstop. The shape `lore_sync/schedule.rs` documents as house
/// style — a `spawn_*_task` in the library, called from the binary, owning its
/// own schedule and staying off every hot path.
pub fn spawn_standing_task(state: AppState) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(TICK_SECONDS));
        loop {
            interval.tick().await;
            match run_due_standing_work(&state).await {
                Ok(report) if report != SweepReport::default() => {
                    eprintln!("[Standing] sweep: {report:?}");
                }
                Ok(_) => {}
                Err(e) => eprintln!("[Standing] ⚠️  {e}"),
            }
        }
    });
}

/// FR-031: the person appeals. One appeal per window, in their own words.
pub fn file_appeal_sync(
    conn: &mut PgConnection,
    account_id: Uuid,
    statement: &str,
    now: DateTime<Utc>,
) -> Result<AccountTermination, String> {
    let termination = open_termination_of(conn, account_id)
        .map_err(|_| "Failed to read the account's standing".to_string())?
        .ok_or_else(|| "There is no open decision on this account to appeal".to_string())?;
    if termination.appeal_state != appeal_state::NONE {
        return Err("An appeal has already been filed against this decision".to_string());
    }

    diesel::update(account_terminations::table.filter(account_terminations::id.eq(termination.id)))
        .set((
            account_terminations::appeal_state.eq(appeal_state::OPEN),
            account_terminations::appeal_statement.eq(Some(statement)),
            account_terminations::appeal_filed_at.eq(Some(now)),
        ))
        .returning(AccountTermination::as_returning())
        .get_result(conn)
        .map_err(|_| "Failed to file the appeal".to_string())
}

/// Put a case back the way the moderation programme always has: a
/// `content_restored` event, which is what stops a case counting (FR-033).
fn restore_case_sync(conn: &mut PgConnection, case_id: Uuid) -> QueryResult<()> {
    let last: ContentModerationAction = content_moderation_actions::table
        .filter(content_moderation_actions::case_id.eq(case_id))
        .order(content_moderation_actions::created_at.desc())
        .select(ContentModerationAction::as_select())
        .first(conn)?;
    diesel::insert_into(content_moderation_actions::table)
        .values(NewContentModerationAction {
            case_id,
            action_type: super::action_type::CONTENT_RESTORED.to_string(),
            entity_type: last.entity_type,
            entity_id: last.entity_id,
            world_id: last.world_id,
            account_id: last.account_id,
            claimant_name: String::new(),
            claimant_contact: String::new(),
            copyrighted_work_description: String::new(),
            infringing_material_location: String::new(),
            good_faith_statement: false,
            accuracy_statement: false,
            signature: String::new(),
            validity_result: None,
            missing_elements: None,
            counter_notice_id: None,
            restoration_due_at: None,
            created_by: None,
        })
        .execute(conn)?;
    Ok(())
}

/// A refusal or a database failure, inside a transaction that must roll back
/// on either. The refusal's sentence is what the administrator is shown.
struct Refused(String);

impl From<String> for Refused {
    fn from(message: String) -> Self {
        Self(message)
    }
}

impl From<diesel::result::Error> for Refused {
    fn from(error: diesel::result::Error) -> Self {
        Self(format!("Failed to record the decision: {error}"))
    }
}

/// What an administrator decided about an appeal.
#[derive(Debug, Clone)]
pub struct AppealDecision {
    pub upheld: bool,
    pub note: Option<String>,
    /// The case the appeal overturned. `None` means the most recent strike —
    /// the one that crossed the threshold.
    pub overturned_case: Option<Uuid>,
}

/// An administrator decides an appeal.
///
/// **Upheld** (FR-033): the case it overturned is restored, the window closes
/// and the account is restored. **Rejected**: the window resumes from the date
/// already fixed, so filing on day 29 bought no second window (FR-034).
pub fn resolve_appeal_sync(
    conn: &mut PgConnection,
    account_id: Uuid,
    resolved_by: Uuid,
    decision: &AppealDecision,
    ladder: Ladder,
    now: DateTime<Utc>,
) -> Result<AccountTermination, String> {
    let upheld = decision.upheld;
    let note = decision.note.as_deref();
    let overturned_case = decision.overturned_case;
    conn.transaction::<_, Refused, _>(|conn| {
        let termination = open_termination_of(conn, account_id)
            .map_err(|_| "Failed to read the account's standing".to_string())?
            .ok_or_else(|| "There is no open decision on this account".to_string())?;
        if termination.appeal_state != appeal_state::OPEN {
            return Err("There is no open appeal on this account".to_string().into());
        }

        let decided = diesel::update(
            account_terminations::table.filter(account_terminations::id.eq(termination.id)),
        )
        .set((
            account_terminations::appeal_state.eq(if upheld {
                appeal_state::UPHELD
            } else {
                appeal_state::REJECTED
            }),
            account_terminations::appeal_resolved_at.eq(Some(now)),
            account_terminations::appeal_resolved_by.eq(Some(resolved_by)),
            account_terminations::appeal_note.eq(note),
        ))
        .returning(AccountTermination::as_returning())
        .get_result(conn)
        .map_err(|_| "Failed to record the decision".to_string())?;

        if !upheld {
            notices::record_sync(
                conn,
                account_id,
                notices::kind::APPEAL_RESOLVED,
                None,
                Some(json!({
                    "upheld": false,
                    "deletionDueAt": decided.deletion_due_at.to_rfc3339(),
                    "requiresHuman": decided.requires_human,
                })),
            )
            .map_err(|_| "Failed to tell the person".to_string())?;
            return Ok(decided);
        }

        let strikes = super::strikes_of(conn, account_id, ladder.lookback_days)
            .map_err(|_| "Failed to read the account's strikes".to_string())?;
        let case = match overturned_case {
            Some(case) if strikes.iter().any(|s| s.case_id == case) => case,
            Some(_) => {
                return Err("That case is not one of this account's strikes"
                    .to_string()
                    .into());
            }
            None => {
                strikes
                    .last()
                    .ok_or_else(|| "This account holds no strike to overturn".to_string())?
                    .case_id
            }
        };
        restore_case_sync(conn, case).map_err(|_| "Failed to restore the case".to_string())?;
        close_termination_sync(conn, &decided, closed_reason::APPEAL_UPHELD, now)
            .map_err(|_| "Failed to close the decision".to_string())?;

        // The row as it now stands — closed — rather than as it was a moment
        // ago, so the caller shows the administrator what actually happened.
        Ok(account_terminations::table
            .filter(account_terminations::id.eq(decided.id))
            .select(AccountTermination::as_select())
            .first(conn)?)
    })
    .map_err(|Refused(message)| message)
}

/// An administrator carries out a window that waits for a person.
pub fn execute_by_administrator_sync(
    conn: &mut PgConnection,
    account_id: Uuid,
    now: DateTime<Utc>,
) -> Result<(), String> {
    let termination = open_termination_of(conn, account_id)
        .map_err(|_| "Failed to read the account's standing".to_string())?
        .ok_or_else(|| "There is no open decision on this account".to_string())?;
    if !termination.disables_account {
        return Err(
            "This is the instance's last administrator. Deleting it would lock the instance out \
             of its own administration; appoint another administrator first."
                .to_string(),
        );
    }
    if termination.appeal_state == appeal_state::OPEN {
        return Err("An appeal is open; decide it first".to_string());
    }
    if !is_due_for_deletion(&termination, now) {
        return Err("The window has not ended".to_string());
    }
    execute_termination_sync(conn, &termination, now)
        .map_err(|e| format!("Failed to delete the account: {e}"))
}

/// FR-028: a case against `account_id` has just become upheld — tell them.
///
/// Called on the connection that wrote the moderation event, straight after
/// it, and only when that event made the case count — whether it is news is
/// the caller's to know, because only the caller saw the case before. Writes
/// nothing if the case turns out not to count after all.
///
/// The notice says what the strike was for, that it counts, how many remain
/// and when it stops counting. The strike that reaches the suspension rung
/// also gets a second notice saying so, and the one that reaches the threshold
/// opens the window there and then (FR-030) rather than at the next sweep.
pub fn tell_of_strike_sync(
    conn: &mut PgConnection,
    account_id: Uuid,
    case_id: Uuid,
    ladder: Ladder,
) -> QueryResult<()> {
    let standing = standing_of_sync(conn, account_id, ladder)?;
    let Some(strike) = standing.strikes.iter().find(|s| s.case_id == case_id) else {
        return Ok(());
    };
    let count = standing.strike_count();
    let subject = json!({
        "caseId": strike.case_id,
        "entityType": strike.entity_type,
        "entityId": strike.entity_id,
        "worldId": strike.world_id,
    });

    notices::record_sync(
        conn,
        account_id,
        notices::kind::STRIKE_RECORDED,
        Some(subject.clone()),
        Some(json!({
            "strikeCount": count,
            "suspendPublishingAt": ladder.suspend_publishing_at,
            "threshold": ladder.threshold,
            "agesOutAt": ladder.ages_out_at(strike.recorded_at).to_rfc3339(),
        })),
    )?;

    if count == ladder.suspend_publishing_at && count < ladder.threshold {
        notices::record_sync(
            conn,
            account_id,
            notices::kind::PUBLISHING_SUSPENDED,
            Some(subject),
            Some(json!({
                "strikeCount": count,
                "threshold": ladder.threshold,
            })),
        )?;
    }

    if count >= ladder.threshold {
        open_termination_sync(conn, account_id, ladder, Utc::now())?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "standing_tests.rs"]
mod standing_tests;
