//! The way back in for an instance whose only administrator has lost both
//! their authenticator and their recovery codes.
//!
//! # The lockout this closes
//!
//! `operator_reset` is the defined path for somebody who has lost everything,
//! and it needs an administrator to perform it. On the small self-hosted
//! instance this product is mostly run as, there is one administrator, and
//! when they are the person who lost everything nobody is left to press the
//! button. The only remaining way in was the database edit spec 041 FR-024
//! forbids: unaudited, unnotified, done from memory.
//!
//! # Why an environment variable
//!
//! For the reason `THUNDERFORGE_REGENERATE_SETUP_CODE` is one: the person
//! this is for has no session and cannot get one, so the only handle they
//! have on the instance is its environment. Being able to set a variable and
//! restart the process is proof of control of the machine, which is a
//! stronger position than any administrator's session — whoever holds it
//! could already read the database and the secret that encrypts it.
//!
//! # What it does, and does not
//!
//! Exactly what an operator's reset does and nothing more: the factor, its
//! pending enrolment and its recovery codes are cleared in one transaction,
//! a `reset_by_operator` event is written with **no** actor (nobody signed in
//! did this, and the record must not pretend somebody did), and the account
//! holder is told. It does not enrol anything and does not touch the
//! password. The next sign-in then takes the administrator through enrolment,
//! because an administrator is required to hold a factor by role.
//!
//! It is fenced to administrators. An ordinary account that has lost
//! everything has an administrator to ask, and a variable that could strip
//! any account's factor is more than this needs to be.
//!
//! # The variable must be removed afterwards
//!
//! It is read on **every** start, and unlike the setup-code variable there is
//! no state to tell a deliberate request from one left behind: an
//! administrator who re-enrols and restarts with the variable still set has
//! their new factor cleared again. Every log line this writes says so. It is
//! loud rather than clever on purpose — a guard that refused a second reset
//! would also refuse the second time somebody genuinely needed one.

use super::disable::clear_second_factor_sync;
use super::events;
use super::*;

/// Names the administrator, by username, whose second factor is cleared at
/// startup.
pub(crate) const RESET_VAR: &str = "THUNDERFORGE_RESET_ADMIN_SECOND_FACTOR";

/// What a start did about the variable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartupReset {
    /// The variable is unset. The ordinary case; nothing is logged.
    NotRequested,
    /// Nobody has that username.
    NoSuchUser,
    /// The account exists and is not an administrator.
    NotAnAdministrator,
    /// The administrator holds no factor, so there was nothing to clear —
    /// what a second start with the variable left in place looks like.
    NothingToClear,
    /// The factor was cleared.
    Cleared,
}

/// The decision, from what was found about the named account.
///
/// Pure for the reason `bootstrap_action` is: the rule is what is worth
/// reading in one place, and the statements that carry it out are not.
pub(crate) fn startup_reset_decision(account: Option<(bool, bool, bool)>) -> StartupReset {
    match account {
        None => StartupReset::NoSuchUser,
        Some((false, _, _)) => StartupReset::NotAnAdministrator,
        // A pending enrolment with no confirmed factor is still something to
        // clear: it is a secret on an authenticator that may also be lost.
        Some((true, false, false)) => StartupReset::NothingToClear,
        Some((true, _, _)) => StartupReset::Cleared,
    }
}

/// Clear `username`'s second factor if they are an administrator.
pub(crate) async fn reset_admin_second_factor(
    state: &AppState,
    username: &str,
) -> Result<StartupReset, String> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection".to_string())?;
    let username = username.to_string();

    let (outcome, user_id) = tokio::task::spawn_blocking(
        move || -> Result<(StartupReset, Option<uuid::Uuid>), String> {
            let row = users::table
                .filter(users::username.eq(&username))
                .select((
                    users::id,
                    users::is_admin,
                    users::two_factor_enabled,
                    users::two_factor_pending_secret_encrypted.is_not_null(),
                ))
                .first::<(uuid::Uuid, bool, bool, bool)>(&mut conn)
                .optional()
                .map_err(|_| "Failed to look up the named administrator".to_string())?;

            let decision = startup_reset_decision(
                row.map(|(_, is_admin, enabled, pending)| (is_admin, enabled, pending)),
            );
            let user_id = row.map(|(id, ..)| id);

            if let (StartupReset::Cleared, Some(user_id)) = (&decision, user_id) {
                clear_second_factor_sync(
                    &mut conn,
                    user_id,
                    None,
                    events::event_type::RESET_BY_OPERATOR,
                )
                .map_err(|_| "Failed to clear the second factor".to_string())?;
            }

            Ok((decision, user_id))
        },
    )
    .await
    .map_err(|_| "Failed to spawn blocking task".to_string())??;

    if let (StartupReset::Cleared, Some(user_id)) = (&outcome, user_id) {
        // After the commit, and unable to fail the reset — the same order and
        // the same reason as the operator's route.
        super::notify::tell(state, user_id, super::notify::Change::ResetByOperator).await;
    }

    Ok(outcome)
}

/// Read the variable and act on it. Called once, at startup.
///
/// Never an error the process should die of: an instance that will not start
/// because a recovery variable named nobody is a worse outage than the one the
/// variable was set to fix. A failure is logged and the server carries on.
pub async fn apply_startup_second_factor_reset(state: &AppState) -> StartupReset {
    let Some(username) = crate::settings::registry::read_env(RESET_VAR) else {
        return StartupReset::NotRequested;
    };

    let outcome = match reset_admin_second_factor(state, &username).await {
        Ok(outcome) => outcome,
        Err(message) => {
            tracing::error!(
                "{RESET_VAR} is set but the reset could not be performed: {message}. \
                 Nothing was changed."
            );
            return StartupReset::NotRequested;
        }
    };

    match outcome {
        StartupReset::Cleared => tracing::warn!(
            "{RESET_VAR} is set: the second factor of administrator '{username}' has been \
             CLEARED, along with their recovery codes. Their next sign-in will take them \
             through enrolling a new one. REMOVE {RESET_VAR} from the environment now — \
             while it is set, every restart clears that administrator's factor again."
        ),
        StartupReset::NothingToClear => tracing::warn!(
            "{RESET_VAR} is set, and administrator '{username}' holds no second factor, so \
             nothing was changed. Remove {RESET_VAR} from the environment: left in place \
             it will clear the factor they enrol next, on the next restart."
        ),
        StartupReset::NoSuchUser => tracing::warn!(
            "{RESET_VAR} is set to '{username}', and no account has that username. Nothing \
             was changed. It takes the administrator's username, not their email address."
        ),
        StartupReset::NotAnAdministrator => tracing::warn!(
            "{RESET_VAR} is set to '{username}', which is not an administrator. Nothing was \
             changed: this variable resets administrators only, and an administrator can \
             reset anybody else from the administration screen."
        ),
        StartupReset::NotRequested => {}
    }

    outcome
}

#[cfg(test)]
#[path = "startup_reset_tests.rs"]
mod startup_reset_tests;
