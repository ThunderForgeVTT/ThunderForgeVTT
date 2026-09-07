# Contract: Test structure — an unseeded instance, and a mail sink

Two additions to the harness. Both are needed because two of this feature's
user stories cannot be observed against the harness as it stands.

## 1. The unseeded-database project

**The problem**: `scripts/e2e-parallel.mjs` migrates and seeds **one** template
database (`diesel migration run`, then `demo_accounts.sql` and `e2e_demo.sql`)
and clones it per shard with `CREATE DATABASE … TEMPLATE …`. Every shard
therefore starts with `e2eadmin` already existing, so `setup_status` reports
`setup_required: false` and **US1 — "from empty database to a real instance" —
is unobservable**. It is also why there is no `setup.spec.ts` today: the story
has never been testable, not that nobody wrote it.

**The addition**: a second template, migrated and **not** seeded, and a
Playwright project pinned to it.

```js
// scripts/e2e-parallel.mjs
const TEMPLATE_DB          = "thunderforge_e2e_template";        // existing: migrated + seeded
const UNSEEDED_TEMPLATE_DB = "thunderforge_e2e_template_bare";   // NEW: migrated only
```

```ts
// apps/web/playwright.config.ts — projects
{ name: "chromium" },                       // unchanged; the seeded stack
{ name: "first-run", testMatch: /instance-setup\.spec\.ts/ }   // the bare stack
```

**Rules**

1. The bare template is **migrated**, never seeded. A test that needs a
   fixture in it is a test that belongs in the seeded project.
2. Each first-run shard gets its **own** clone of the bare template, because
   setup is completable exactly once and a shared bare database would let one
   test consume another's.
3. The bootstrap code is read from the backend's log line, as an operator
   reads it. Reading it from the database instead would skip the part of the
   flow most likely to be wrong — the hard-coded
   `http://127.0.0.1:5173/setup/{code}` URL is exactly this class of defect.
4. **No product code changes to make this work.** The same rule spec 036's
   FR-023 states for the OAuth stub: a test that exercises a test-only branch
   proves the branch. A `#[cfg(test)]` in `admin_setup.rs` would mean the
   first-run path in a release is untested.

## 2. The mail sink

**Mailpit**, added to `compose.yml` beside `postgres` and `rustfs`.

```yaml
mailpit:
  container_name: thunderforge-mailpit
  image: axllent/mailpit:latest
  restart: always
  ports:
    - "1025:1025"   # SMTP — what the server sends to
    - "8025:8025"   # HTTP API — what a test reads the message back from
```

```ts
// apps/web/e2e/fixtures/mailpit.ts
/** The messages Mailpit has received, newest first. */
export async function inbox(baseUrl: string): Promise<MailpitMessage[]>;
/** Wait for a message to <to>, or fail with what did arrive. */
export async function waitForMessage(baseUrl: string, to: string): Promise<MailpitMessage>;
/** Empty the sink between tests. */
export async function clearInbox(baseUrl: string): Promise<void>;
```

**Rules**

1. **It speaks real SMTP.** That is the entire point. A capturing transport
   inside the process proves the code around the transport and nothing about
   the transport, and the transport is where mail fails — TLS negotiation, the
   From address, authentication.
2. One SMTP port and one API port **per shard**, exactly as backends, vite
   servers and buckets already get one.
3. Tests **never** assert on a message body containing personal content. They
   assert a message arrived, to whom, and — for a test message only — its
   subject. The product does not expose bodies (see `mail.md` rule 1) and the
   suite does not build a habit of reading them.
4. `clearInbox` between tests, because the sink is shared within a shard and a
   test asserting "the newest message" against somebody else's is a flake with
   a plausible story.
5. **Never in a release image.** It is a dev and harness service, like the
   seed files.

## Three levels of mail proof, and why all three

| Level | Where | Proves | Does not prove |
|---|---|---|---|
| `CapturingTransport` | `cargo test` | the outbox, the state machine, that no body is logged, that a failure is recorded | anything about SMTP |
| `SmtpTransport` → Mailpit | `cargo test` (integration) | `lettre` is wired right, the From address, STARTTLS negotiation, authentication | the operator's experience |
| Browser → admin panel → Mailpit | Playwright | FR-013 as written: an operator configures mail, presses test, and the message arrives | — |

Level 2 is the tempting one to skip. Skipping it is how a mail subsystem ships
that has only ever talked to a mock.

## What is deliberately absent

- **No hand-written SMTP server in the test process.** It would need TLS and
  AUTH to be worth anything, at which point it is a second SMTP implementation
  with none of the review the first one has had.
- **No real mailbox.** Not reproducible, not offline, and the first CI run
  would be an abuse report.
- **No global bare-database default.** The seeded template is right for 70+
  existing specs; the bare one is for the handful that must watch an instance
  become an instance.
