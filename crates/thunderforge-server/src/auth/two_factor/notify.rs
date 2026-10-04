//! Telling somebody their second factor changed (FR-015, FR-001b).
//!
//! # After the commit, and unable to fail the request
//!
//! Every caller here has already committed the act — the factor is removed,
//! the codes are reissued, the operator's reset is done — and the record of it
//! is in `two_factor_events` inside that same transaction. This is the
//! *notification*, which is a different thing with a different failure mode:
//! it goes out to a mail server that may be misconfigured, unreachable, or
//! entirely absent, and none of those may turn a completed act into an error
//! the person sees.
//!
//! So it takes no result the caller checks, and it swallows what it cannot do.
//! A caller that could fail on this would be a caller that fails a *removal*
//! because a mail server was down, leaving the account changed and the request
//! refused — the worst of both.
//!
//! # Why the outbox rather than a send
//!
//! `mail::outbox::enqueue` is the only way anything here sends mail, and it
//! **succeeds on an instance with no mail configured** — the row lands
//! `blocked`, with the reason naming the settings that are unset, and goes out
//! if somebody configures them later. That is exactly the posture FR-001b
//! asks for: nothing in this feature consults mail before acting, and an
//! instance that cannot send is a normal instance rather than a broken one.
//!
//! # What a message may say
//!
//! That something changed, and what to do if it was not them. Never a code,
//! never a secret, never how many recovery codes are left, and never who the
//! operator was — the subject of a reset is entitled to know it happened, and
//! naming the administrator in an email turns a security notice into something
//! to forward. The record in `two_factor_events` holds the actor; the message
//! holds the fact.

use diesel::prelude::*;

use crate::mail::outbox::{self, NewOutboxMessage};
use crate::schema::users;
use crate::state::AppState;

/// What happened, in the words the person reads.
///
/// A closed set rather than a caller-supplied string: these are security
/// notices, and "whatever the call site felt like writing" is how one of them
/// ends up quoting a code back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Change {
    /// FR-014. The account holder turned it off themselves.
    Removed,
    /// FR-025. An operator cleared it, so the account signs in on a password
    /// until it enrols again. The most important of these to send.
    ResetByOperator,
    /// FR-010. A fresh set was issued and every earlier code stopped working.
    RecoveryCodesReissued,
}

impl Change {
    fn subject(self) -> &'static str {
        match self {
            Self::Removed => "Two-factor authentication was turned off",
            Self::ResetByOperator => "Your second factor was reset",
            Self::RecoveryCodesReissued => "Your recovery codes were replaced",
        }
    }

    fn body(self) -> &'static str {
        match self {
            Self::Removed => {
                "Two-factor authentication was turned off on your ThunderForge \
                 account. Signing in now needs your password alone.\n\n\
                 If this was not you, sign in and turn it back on, and change \
                 your password."
            }
            Self::ResetByOperator => {
                "An administrator of this ThunderForge instance reset the second \
                 factor on your account. Signing in now needs your password \
                 alone, so set a new one up as soon as you can.\n\n\
                 If you did not ask for this, change your password and speak to \
                 whoever runs this instance."
            }
            Self::RecoveryCodesReissued => {
                "A new set of recovery codes was issued for your ThunderForge \
                 account. Every code from the previous set has stopped working.\n\n\
                 If this was not you, sign in and turn two-factor \
                 authentication off and on again, and change your password."
            }
        }
    }
}

/// Tell `user_id` that `change` happened. Best-effort, and silent about it.
///
/// Call **after** the transaction that performed the act has committed. There
/// is no return value on purpose: there is nothing a caller could correctly do
/// with a failure here, and offering one invites somebody to `?` it.
pub(crate) async fn tell(state: &AppState, user_id: uuid::Uuid, change: Change) {
    let Ok(mut conn) = state.db_pool.get() else {
        return;
    };
    let address = tokio::task::spawn_blocking(move || {
        users::table
            .filter(users::id.eq(user_id))
            .select(users::email)
            .first::<String>(&mut conn)
            .optional()
    })
    .await;

    let Ok(Ok(Some(address))) = address else {
        return;
    };
    if address.trim().is_empty() {
        return;
    }

    // The one place a failure could be interesting is a database that is down,
    // and the caller has just committed against it, so it is not. Dropped
    // rather than logged at error: an instance with no mail configured would
    // otherwise log one of these every time anybody used the feature.
    let _ = outbox::enqueue(
        state,
        NewOutboxMessage {
            purpose: "two_factor_change".to_string(),
            to_address: address,
            subject: change.subject().to_string(),
            body_text: change.body().to_string(),
            // The instance's own notice. Naming the operator here would put
            // them in the message metadata of a mail the subject can forward.
            created_by: None,
        },
    )
    .await;
}

#[cfg(test)]
#[path = "notify_tests.rs"]
mod notify_tests;
