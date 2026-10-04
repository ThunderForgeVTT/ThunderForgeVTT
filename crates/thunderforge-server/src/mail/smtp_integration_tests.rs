//! The level it is tempting to skip: a real [`SmtpTransport`] against a real
//! SMTP server.
//!
//! The capturing transport proves the code *around* `lettre` — that a message
//! is enqueued, that a refusal is recorded, that a body is never returned. It
//! proves nothing about `lettre` itself, about STARTTLS negotiation, or about
//! whether the From address is the one the operator set, and those are where
//! mail actually fails (research.md § R8).
//!
//! # Why these are `#[ignore]`d
//!
//! They need Mailpit, which `compose.yml` provides but which a plain
//! `cargo test` has no way to start. An ignored test that says how to run it
//! is honest; a test that silently passes when the server it was written
//! against is absent is worse than no test, because it reports a green result
//! for a thing it never did.
//!
//! ```sh
//! docker compose up -d mailpit
//! cargo test -p thunderforge-server --lib mail::smtp_integration -- --ignored
//! ```

use diesel::prelude::*;

use super::outbox::{self, NewOutboxMessage, OutboxState, PURPOSE_TEST};
use super::smtp::{MailConfig, Security, SmtpTransport};
use super::{MailTransport, OutgoingMessage, schedule};
use crate::schema::{instance_settings, mail_outbox};
use crate::test_support::test_app_state;

const MAILPIT_SMTP_PORT: u16 = 1025;
const MAILPIT_API: &str = "http://localhost:8025/api/v1";

fn mailpit_config() -> MailConfig {
    MailConfig {
        host: "localhost".to_string(),
        port: MAILPIT_SMTP_PORT,
        // Mailpit's SMTP listener is plaintext in the dev stack, which is what
        // `none` is for and why the declaration offers it.
        security: Security::None,
        username: None,
        password: None,
        from_address: "no-reply@thunderforge.test".to_string(),
        from_name: Some("ThunderForge".to_string()),
    }
}

async fn clear_inbox() {
    let _ = reqwest::Client::new()
        .delete(format!("{MAILPIT_API}/messages"))
        .send()
        .await;
}

/// The whole point of this file: a message built by this code, carried by
/// `lettre`, accepted by a real SMTP server, and read back with the From
/// address and subject the operator configured.
#[tokio::test]
#[ignore = "needs Mailpit: docker compose up -d mailpit"]
async fn a_message_reaches_a_real_smtp_server_with_the_configured_from_address() {
    clear_inbox().await;

    let transport = SmtpTransport::new(mailpit_config());
    transport
        .send(&OutgoingMessage {
            to: "operator@thunderforge.test".to_string(),
            subject: "ThunderForge: mail is configured".to_string(),
            body_text: "If you are reading this, it does.".to_string(),
        })
        .await
        .expect("Mailpit accepts the message");

    let delivered: serde_json::Value = reqwest::get(format!("{MAILPIT_API}/messages"))
        .await
        .expect("Mailpit's API answers")
        .json()
        .await
        .expect("it answers JSON");
    let first = &delivered["messages"][0];

    assert_eq!(first["From"]["Address"], "no-reply@thunderforge.test");
    assert_eq!(first["From"]["Name"], "ThunderForge");
    assert_eq!(first["To"][0]["Address"], "operator@thunderforge.test");
    assert_eq!(first["Subject"], "ThunderForge: mail is configured");
}

/// A host that is not listening is the commonest misconfiguration there is,
/// and the reason it produces must name the settings rather than repeat a
/// `lettre` debug string.
#[tokio::test]
#[ignore = "needs Mailpit: docker compose up -d mailpit"]
async fn an_unreachable_host_is_retryable_and_names_the_settings() {
    let mut config = mailpit_config();
    config.port = 1;

    let failure = SmtpTransport::new(config)
        .send(&OutgoingMessage {
            to: "operator@thunderforge.test".to_string(),
            subject: "s".to_string(),
            body_text: "b".to_string(),
        })
        .await
        .expect_err("nothing is listening on port 1");

    assert!(failure.retryable);
    assert!(failure.reason.contains("`mail.host`"), "{}", failure.reason);
    assert!(
        !failure.reason.contains("localhost"),
        "the reason named the host: {}",
        failure.reason
    );
}

/// Sending twice through one transport must produce two messages, not one and
/// a reused connection that the second send silently drops.
#[tokio::test]
#[ignore = "needs Mailpit: docker compose up -d mailpit"]
async fn two_messages_are_two_messages() {
    clear_inbox().await;

    let transport = SmtpTransport::new(mailpit_config());
    for n in 0..2 {
        transport
            .send(&OutgoingMessage {
                to: format!("operator{n}@thunderforge.test"),
                subject: format!("Message {n}"),
                body_text: "body".to_string(),
            })
            .await
            .expect("Mailpit accepts it");
    }

    let delivered: serde_json::Value = reqwest::get(format!("{MAILPIT_API}/messages"))
        .await
        .expect("Mailpit's API answers")
        .json()
        .await
        .expect("it answers JSON");
    assert_eq!(delivered["messages_count"], 2);
}

/// The production path, end to end inside the server: settings rows describing
/// Mailpit, a message enqueued the way a feature would enqueue one, and the
/// background sender's own tick — no override, no shortcut. Rule 2 of
/// contracts/mail.md is that the code path under test is the code path in
/// production, and this is the test that makes the claim true.
#[tokio::test]
#[ignore = "needs Mailpit: docker compose up -d mailpit"]
async fn an_enqueued_message_is_sent_by_the_tick_once_mail_is_configured() {
    clear_inbox().await;

    // No `settings::test_env` lock: it is a `std::sync::Mutex`, and holding one
    // across an await is a clippy error and a deadlock waiting to happen. This
    // test is `#[ignore]`d and run deliberately, which is the serialisation
    // that lock would have provided.
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("a connection");

    // One tick attempts a bounded batch, oldest first (`schedule::BATCH`), and
    // the other tests in this crate leave queued rows behind on purpose —
    // nothing in this product deletes an outbox row. Starting from an empty
    // table is what makes "the tick sent *this* message" the thing being
    // asserted rather than "the tick got as far as this message".
    diesel::delete(mail_outbox::table)
        .execute(&mut conn)
        .expect("the outbox is emptied");

    for (key, value) in [
        ("mail.enabled", "true"),
        ("mail.host", "localhost"),
        ("mail.port", "1025"),
        ("mail.security", "none"),
        ("mail.from_address", "no-reply@thunderforge.test"),
    ] {
        diesel::insert_into(instance_settings::table)
            .values((
                instance_settings::key.eq(key),
                instance_settings::value.eq(value),
            ))
            .on_conflict(instance_settings::key)
            .do_update()
            .set(instance_settings::value.eq(value))
            .execute(&mut conn)
            .expect("the setting is written");
    }
    drop(conn);

    let id = outbox::enqueue(
        &state,
        NewOutboxMessage {
            purpose: PURPOSE_TEST.to_string(),
            to_address: "operator@thunderforge.test".to_string(),
            subject: "ThunderForge: mail is configured".to_string(),
            body_text: "If you are reading this, it does.".to_string(),
            created_by: None,
        },
    )
    .await
    .expect("it is written down");

    schedule::tick(&state).await;

    let mut conn = state.db_pool.get().expect("a connection");
    let row = outbox::load(&mut conn, id)
        .expect("it loads")
        .expect("it is still there");
    assert_eq!(
        row.state(),
        Some(OutboxState::Sent),
        "{:?}",
        row.last_failure_reason
    );
    assert!(row.sent_at.is_some());

    // Every mail setting this test wrote is removed, because the settings
    // row-set is shared by the whole test binary and a row left behind would
    // silently configure mail for every test that runs after it.
    diesel::delete(instance_settings::table.filter(instance_settings::key.like("mail.%")))
        .execute(&mut conn)
        .expect("the settings are removed");

    let delivered: serde_json::Value = reqwest::get(format!("{MAILPIT_API}/messages"))
        .await
        .expect("Mailpit's API answers")
        .json()
        .await
        .expect("it answers JSON");
    assert_eq!(delivered["messages_count"], 1);
    assert_eq!(
        delivered["messages"][0]["To"][0]["Address"],
        "operator@thunderforge.test"
    );
}
