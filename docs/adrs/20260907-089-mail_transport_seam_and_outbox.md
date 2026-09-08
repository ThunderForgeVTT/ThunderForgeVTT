# Mail Is A Transport Seam Behind A Durable Outbox

- **Date**: 2026-09-07
- **Status**: Accepted
- **Spec**: `specs/040-instance-setup/` (US4, FR-013 – FR-016, FR-023, FR-027),
  `contracts/mail.md`, `research.md` § R7 – § R9
- **Follows**: ADR-088, whose refusal of a process-wide cache decides how the
  transport is built
- **Governs**: whether this instance can tell somebody something, and what
  happens to the message when it cannot

## The decision

Three parts, and the third is the one that will still matter in a year.

1. **`lettre` 0.11, default features off, `tokio1-rustls-tls`.** One SMTP
   client, one TLS stack, no system libraries.
2. **A transport seam.** `MailTransport` is a trait with three implementations:
   `SmtpTransport` for the product, `Unconfigured` for an instance that has not
   been told where to send, and `CapturingTransport` for tests. Delivery is
   proved at **three levels** — capturing transport, a real SMTP server in the
   dev stack, and the operator's own browser flow — and all three are needed.
3. **A durable outbox, and nothing sends without it.** A feature that wants to
   tell somebody something calls `outbox::enqueue`, which writes a row *before*
   any transport is consulted. Nothing calls `MailTransport::send` except
   `mail::schedule`. The row has five states, and `blocked` is not `failed`.

## There was no mail at all, and that is why this is an ADR

Confirmed by search rather than assumed: across the 35-crate workspace, the
lock file and every Rust and TypeScript source, there was no `lettre`, no SMTP,
no mailer, no template and no `SMTP_*` variable. `users.email` was stored and
had never been sent to. The "outbox" hits were the client-side offline mutation
queue in `thunderforge-cache-browser`; the "notification" hits were PostgreSQL
`LISTEN`/`NOTIFY`.

So this is a new subsystem, and specs 035, 037 and 039 are all waiting to send
something through it. Choosing its shape once, in the open, is cheaper than
three features each discovering it.

## Why `lettre`, and why the TLS choice mattered more

`lettre` is the only maintained Rust SMTP client with a native async Tokio
transport, which makes the crate choice close to forced. The decision that took
argument is **rustls**.

This product's first-party dependency tree is rustls throughout — `reqwest` is
declared `default-features = false, features = [..., "rustls"]`, and
`hyper-rustls`/`tokio-rustls` are what the lock resolves. `openssl` appears only
through the legacy `websocket 0.27.1` chain, transitively. Taking `native-tls`
here would promote a system library to a **first-party** dependency of every
build and every container image, for one subsystem. Default features are off for
the same reason: they pull in the file and sendmail transports and a second TLS
stack.

Rejected, each for a stated reason:

- *An HTTP mail API — SES, SendGrid, Postmark, Mailgun.* The spec's assumption
  is explicit: mail means SMTP first, because SMTP is what a self-hoster has. An
  API provider is an account they do not. The seam leaves that door openable
  without opening it.
- *Shell out to `sendmail`.* Assumes an MTA in the container. The images this
  product ships do not have one, and the failure is silent.
- *Write SMTP by hand.* AUTH, STARTTLS negotiation and MIME encoding are not the
  interesting part, and getting them subtly wrong produces mail some receivers
  accept and others reject — which is the exact edge case FR-014 is about.

## Why the transport is a trait, and why it is behind a seam that rebuilds

An `Option<SmtpClient>` puts "did we remember to check?" at every call site, and
reconstructs the answer to "why not" at each one. A transport that **refuses
with a reason** puts that answer in one place, as a sentence an operator can
act on. This is not a new pattern in this codebase: `AppState` already holds a
`SessionAdjudicator` with a local implementation and a remote one, and
`test_support` already swaps it.

What did change from the plan is *how it is held*. `research.md` § R8 proposed
`AppState` holding `mail: Arc<dyn MailTransport>`. What shipped is a `MailSeam`
that holds an **override** — which tests set, and nothing else ever does — and
otherwise builds the transport from the settings as they resolve **right now**.

The reason is ADR-088's, applied one layer down: `AppState` is cloned into every
request, so a transport constructed once at startup would still be talking to
the SMTP host an operator corrected ten minutes ago. FR-007 says a settings
change takes effect without a restart, and a cached transport is a cached
setting wearing a different type.

## The three levels of proof, and which one is tempting to skip

1. **Unit — `CapturingTransport`.** Collects messages in memory. Proves the code
   *around* the transport: that an enqueued message reaches one, that a refusal
   is recorded rather than lost, that a body is never logged, that the outbox
   transitions and that the backoff curve is respected. Its own module docs say
   what it does not prove: **nothing at all about SMTP**.
2. **Integration — a real `SmtpTransport` against Mailpit** in `compose.yml`,
   beside `postgres` and `rustfs`. Real SMTP on 1025, a JSON API on 8025 the
   test reads the delivered message back from. This is the level that proves
   `lettre` is wired correctly, that STARTTLS negotiation happens, and that the
   From address is the one the operator set.
3. **End-to-end — the operator flow.** Configure mail in the administration
   surface, press *send a test message*, assert the outcome in the browser
   **and** assert the message arrived in Mailpit. This is the level FR-013 is
   actually written about: a test message that proves it works before anything
   depends on it.

Level 2 is the one it would be tempting to skip, and skipping it is how a mail
subsystem ships that has only ever talked to a mock. Rejected alternatives: a
fake SMTP server inside the Rust test process (a second SMTP implementation with
none of the review a real one has had), the capturing transport alone (proves
everything except the part that fails), and a real mailbox (not reproducible,
not offline, and the first CI run would be an abuse report).

The cost is one more container in `compose.yml` and one more service per e2e
shard in `scripts/e2e-parallel.mjs` — which already hands each shard a backend
port, a Vite port and a bucket. It is recorded in the plan's Complexity Tracking
rather than waved through.

## Why `blocked` is a separate state from `failed`

This is the load-bearing distinction in the outbox, and it is a distinction
about **sentences an operator reads**, not about bookkeeping.

- **`blocked`** — this instance has nowhere to send yet. It never tried. The fix
  is a setup step, and configuring mail releases the message.
- **`failed`** — it tried, the retry curve ran out, and it is not going. The fix
  is an incident: a credential, a firewall, a receiving server refusing.

Collapsing them would make an instance that has never had a mail server
indistinguishable from one whose mail server rejects it. Those are the two most
common states a self-hosted instance is in, they have nothing in common, and an
operator told only "not sent" has to go and find out which.

The state also makes FR-015 true *by construction* rather than by everybody
remembering to check. Because enqueue happens before the transport is consulted,
"the instance has no mail configured" cannot be a path on which a message is
discarded — there is no such path. It is a row sitting in `blocked` with a
reason. That is what lets specs 035, 037 and 039 be built against a queue that
exists whether or not the operator has configured a server yet.

Two states in the shipped enum were not in the research and are worth naming.
**`sending`** exists so a message in flight is not started twice by a tick that
overlaps the previous one. **`queued`** with a future `next_attempt_at` is where
the backoff curve lives, so waiting and having-failed-permanently are not the
same row either. `may_be_retried()` is the single place that says a `sent`
message is never re-sent (`contracts/mail.md` rule 6) and an in-flight one is not
restarted.

## Why the scheduler copies lore sync exactly

`lore_sync/schedule.rs` was the only retry loop in this product, and it is a
good one. `mail::schedule` takes its shape deliberately: the same
`BACKOFF_SECONDS = [30, 60, 120, 300, 600, 900, 1800, 2700, 3600]`, the same
extracted `due_now(conn, now)` selection function outside the spawned task, the
same `spawn_*_task` registration in `src/app/src/main.rs`.

The reason for extracting selection is quoted from that file because it applies
here verbatim: *a promise enforced by an `if` inside a loop inside a spawned task
is a promise nothing can test.* The promises here are that a message is never
sent twice, that one waiting on the curve is left alone, and that a `blocked`
message is not attempted against a mail server that does not exist. Pulled out,
each is one assertion against a database instead of an argument about a loop.

There should be **one** retry curve in this product to reason about, not two
that drift. The curve ends rather than growing forever for that file's reason:
a message refused nine times over about two hours is being refused for a reason
retrying will not fix, and the operator surface has had it in front of them the
whole time. The only deliberate difference is the tick — 15 seconds against lore
sync's 30 — because a person pressing *send a test message* is watching, and an
empty outbox costs one indexed query.

Rejected: sending inline from the calling mutation (a slow SMTP server becomes a
slow mutation, and the failure dies with the request), and an external queue.
`schedule.rs` already refused a broker for lore sync with the reason that applies
word for word here: a queue or an external scheduler would be a new deployment
component for a loop the process can hold itself. A self-hosted VTT should not
gain RabbitMQ to send a password reset.

## Privacy: what is stored, what is shown, and what this actually promises

The body and the subject are stored **encrypted** with `crypto.rs`
(`v1.<nonce>.<ciphertext>`, AES-256-GCM), and no administrative surface returns
the body, ever. What an operator sees is recipient, purpose, state, attempt
count, last failure reason and timestamps.

The body is stored at all — rather than re-rendered on retry — because a retry
must send the message that failed, not a later approximation of it. Re-rendering
would require every calling feature to reproduce a message identically at an
arbitrary future time, which makes the retry a *different* message.

The subject is disclosed for exactly one case: a message the instance generated
about itself, which today means the test message. A subject about a person is
content.

**Stated honestly, because a security control that overclaims is a lie:** this
defends the administration surface, the logs and a leaked backup. It does not
defend against the operator, who holds `THUNDERFORGE_SECRET` and the database.
FR-016 is a requirement about the *product's surfaces*, and this is what the
product can actually promise.

Failure prose gets the same treatment from the other direction. A `lettre`
error's `Display` is a debugging string, and stringifying it is how a credential
reaches a screen. So `classify` reduces an error to one of a closed set of
*situations* and `failure_for` turns a situation into an operator sentence —
which lets the FR-014 test enumerate every situation rather than every error a
server might send. `scrub` is the belt to that braces: every configured value is
removed from whatever prose results, including a receiving server's verbatim
refusal text. A rule enforced by remembering is a rule that holds until the next
contributor. `MailConfig` deliberately does not derive `Debug`, for the same
reason.

## What this does not solve

There are **no templates, no HTML and no per-feature message catalogue.** This
subsystem owns *whether* a message can be sent; specs 035, 037 and 039 own what
theirs say. `body_text` is plain text, and a richer form is a later feature's
decision rather than a column nobody fills in yet.

It also does not make delivery *likely*. SPF, DKIM, DMARC and whether a
self-hosted instance's mail reaches an inbox rather than a spam folder are the
operator's problem and outside this product. What the instance can promise is
that it tried, that it says what happened, and that it did not throw the message
away.
