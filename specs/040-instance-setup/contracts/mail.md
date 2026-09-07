# Contract: Mail

There is none today. This is the whole of it: a transport behind a trait, an
outbox nothing bypasses, a background sender, and one mutation that proves it
works.

## The transport seam

```rust
// src/server/src/mail/mod.rs

pub struct OutgoingMessage {
    pub to: String,
    pub subject: String,
    pub body_text: String,
}

/// Why the transport is a trait rather than an `Option<SmtpClient>`:
/// `AppState` already holds `adjudicator: Arc<LocalAdjudicator>` with a
/// `remote.rs` sibling for exactly this shape, and `test_support.rs` already
/// swaps it. An `Option` would put "did we remember to check?" at every call
/// site; `Unconfigured` puts the answer in one place and makes it a reason.
#[async_trait]
pub trait MailTransport: Send + Sync {
    async fn send(&self, message: &OutgoingMessage) -> Result<(), DeliveryFailure>;
    /// What readiness reports. Never a host, never a credential.
    fn availability(&self) -> Availability;
}

pub enum Availability { Ready, Unconfigured { missing: Vec<&'static str> } }

pub struct DeliveryFailure {
    /// Operator-facing prose naming what to fix. Never a password, never a
    /// fragment of one, never a length (FR-014).
    pub reason: String,
    pub retryable: bool,
}
```

Three implementations:

| Implementation | Where | Used by |
|---|---|---|
| `SmtpTransport` | `mail/smtp.rs`, `lettre` with `tokio1-rustls-tls` | the product |
| `Unconfigured` | `mail/mod.rs` | any instance with `mail.*` unresolved — refuses with the list of missing settings |
| `CapturingTransport` | `mail/capture.rs`, `#[cfg(any(test, feature = "test-support"))]` | Rust unit tests |

`AppState` gains `mail: Arc<dyn MailTransport>`, rebuilt when a `mail.*`
setting changes so FR-007 holds with no restart.

**`lettre` with rustls, not native-tls.** The first-party dependency tree is
rustls throughout — `reqwest` is declared `default-features = false, features
= ["form", "json", "rustls"]`. `openssl` appears in the lock only through the
legacy `websocket 0.27.1` chain. Adding it as a first-party dependency would
put a system library in every build and image for one subsystem.

## The outbox

**Nothing calls `MailTransport::send` directly.** A feature that wants to tell
somebody something enqueues:

```rust
// src/server/src/mail/outbox.rs
pub fn enqueue(conn: &mut PgConnection, message: NewOutboxMessage) -> Result<Uuid, String>;
```

This is what makes FR-015 true by construction rather than by discipline. With
no mail configured the row lands in `blocked` and is visible; configuring mail
releases it. A message is never discarded.

The sender is `mail::schedule::spawn_mail_task(app_state)`, registered in
`src/app/src/main.rs` beside the five `spawn_*_task` calls already there, and
modelled directly on `lore_sync/schedule.rs`: a `tokio::time::interval`, the
same nine-step backoff array, and selection extracted as
`due_now(conn, now) -> Result<Vec<Due>, String>` outside the loop — that file's
own reasoning being that "a promise enforced by an `if` inside a loop inside a
spawned task is a promise nothing can test."

## The GraphQL surface

```graphql
type MailAvailability {
  ready: Boolean!
  "Setting keys that are unset, by name. Never a value."
  missing: [String!]!
  "What is limited while mail is unavailable, in sentences."
  limitedFeatures: [String!]!
}

type TestMailResult {
  delivered: Boolean!
  "On failure: what to fix. Never a password or any fragment of one (FR-014)."
  reason: String
  outboxId: UUID!
}

type OutboxEntry {
  id: UUID!
  purpose: String!
  toAddress: String!
  "Present only for purpose = 'test'. A subject about a person is content."
  subject: String
  state: OutboxState!
  attempts: Int!
  lastAttemptAt: String
  nextAttemptAt: String
  lastFailureReason: String
  sentAt: String
  createdAt: String!
}

enum OutboxState { QUEUED, BLOCKED, SENDING, SENT, FAILED }

extend type Query {
  mailAvailability: MailAvailability!
  "Newest first. Administrators only."
  mailOutbox(state: OutboxState, limit: Int): [OutboxEntry!]!
}

extend type Mutation {
  """
  Send a message to the address given, using the settings as they resolve now,
  and report the outcome. The proof FR-013 asks for, before anything depends
  on delivery.
  """
  sendTestMail(to: String!): TestMailResult!

  "Try a blocked or failed message again now, rather than waiting for backoff."
  retryOutboxMessage(id: UUID!): OutboxEntry!
}
```

## Rules

1. **No field returns a message body. Ever.** Not for a test message, not for
   an administrator, not in a diagnostic, not in a log line. `OutboxEntry` has
   no body field and it is not an oversight (FR-016).
2. **`sendTestMail` goes through the outbox like everything else**, so its
   failure is recorded where every other failure is, and so the code path
   under test is the code path in production. A test that exercises a shortcut
   proves the shortcut.
3. **A failure reason names what to fix and nothing else.** "Authentication
   was refused by the server at the configured host — check
   `mail.username` and `mail.password`." Not the host, not the username, not
   the password, not its length (FR-014, SC-004). A test asserts the reason
   for every `lettre` error class contains no configured value.
4. **The instance runs with no mail.** `mailAvailability.ready = false` and
   `limitedFeatures` lists what cannot happen. Nothing refuses to start,
   nothing refuses to serve, no world becomes unplayable (FR-017).
5. **`sendTestMail` is administrators-only and rate-limited.** It sends mail to
   an arbitrary address on request, which is an open relay if it is neither.
   Reuse the existing limiter shape from `graphql/share_rate_limit.rs` rather
   than adding a second one — the precedent spec 035 set for invitation
   redemption.
6. **Retry is idempotent.** A message that succeeded is never sent twice by a
   retry; `sent_at` is the guard, written in the same transaction that marks
   the state.
7. **Changing a `mail.*` setting rebuilds the transport** and moves `blocked`
   messages back to `queued`. Configuring mail is what unblocks the queue, and
   an operator should see it happen.

## Failure shapes

| Situation | Result |
|---|---|
| No mail configured, a feature enqueues | Row in `blocked`, named missing settings, nothing discarded |
| Host unreachable | `queued`, retried on the backoff curve, reason recorded |
| Authentication refused | `queued`, then `failed` after the curve; reason names the settings to check |
| Connects, and the receiver rejects the message | Reason carries the receiver's refusal text verbatim; not retryable if the code is permanent |
| `THUNDERFORGE_SECRET` rotated, password will not decrypt | `mail.password` resolves as unset; readiness says so; nothing sends and nothing crashes |
| An operator asks for a test with wrong settings | `TestMailResult { delivered: false, reason: … }`, and a row they can find later |

## What is deliberately absent

- **No templates, no HTML, no per-feature message catalogue.** The spec is
  explicit: this feature owns whether a message can be sent, not what any
  message says. Specs 035, 037 and 039 own theirs. `body_text` is plain text
  and a later feature may add a richer form.
- **No provider APIs.** SMTP first, per the spec's assumption. The trait is
  what leaves the door open.
- **No bounce handling, no unsubscribe, no delivery receipts.** A self-hosted
  instance sending operational mail to its own members does not need a mailing
  list. Adding any of them would be a different feature with a different
  privacy analysis.
