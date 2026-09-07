# Phase 0 Research: Instance Setup and Configuration

Fifteen decisions and one register of disagreements. Every one is grounded in
what the codebase does today, because this feature's whole premise is that
five subsystems each answered the same question differently and nobody wrote
the answers down together. Reading them first is not optional here — three of
the five answers are not what the spec assumes they are.

The last section, § D, is deliberate: **where the spec's requirement and the
codebase disagree, this file says so rather than quietly picking one.**

---

## R1 — Where instance configuration lives

**What is there today**, all five of it:

| Configuration | Where it actually is | Read how |
|---|---|---|
| Realm manifest (`realm_name`, `interface_pack_id`, `asset_pack_id`, `support_email`, `welcome_message`, `default_game_system_id`) | A **JSON file on disk** — `<data_path>/config/manifest.json`, seeded from `config/realm-defaults.json` (compiled in with `include_str!`) | `admin.rs:473` reads and `admin.rs:603` writes it per request; whitelist at `admin.rs:524` `editable_manifest_keys()` |
| Instance access policy | `instance_access_settings` table, singleton | `auth/instance_access::load_policy`, read per request |
| OAuth providers | `oauth_providers` rows, with `config_source` = `env` or `admin` | `admin.rs:280` `materialize_env_oauth_providers` writes env into rows at startup |
| Lore sync's GitHub App | Environment only | `repo_host.rs:165` `registration_from_env()`, called per use |
| Core server config | `THUNDERFORGE_SECRET`, `THUNDERFORGE_DATA_PATH`, `THUNDERFORGE_SECURE_COOKIES` | `config/mod.rs` `Config::from_env()`, read **once at startup** |

Three storage media, three read cadences, two precedence mechanisms.

**Decision**: add **one** table, `instance_settings`, holding a row per
configured value, and **one** declaration list in code that says what settings
exist. Everything new this feature collects lands there. Nothing existing
moves.

**Rationale**: the spec's own assumption — "the realm manifest's existing
editable keys stay editable, and this feature gathers them into the same
surface rather than replacing them" — plus its Out of Scope entry, "changing
what any existing setting means". A migration of the manifest file into rows
would be a data migration of an operator's live values for no requirement's
sake. The declaration list is what makes FR-012 ("a new setting inherits this
rule") enforceable rather than aspirational: a setting that is not declared
does not resolve, so there is no way to add one that quietly opts out.

**Alternatives rejected**:

- *Wide singleton table, one column per setting.* Every new setting is a
  migration, and FR-028 (an upgrade introducing a required setting must not
  stop an instance starting) becomes a schema question instead of a
  declaration question.
- *Put it in the realm manifest file.* Spec 039's FR-051 requires the operator
  identity be "stored by the instance, not compiled in and **not left in a
  file**". It also sits under `data_path`, which is gitignored and, in the
  container deployment this feature exists for, is whatever volume the
  operator did or did not mount. An instance's identity must not be able to
  vanish with a volume while its database survives.
- *Reuse `instance_identity`.* See R3.

## R2 — Why `instance_identity` is the wrong table

`src/server/src/instance_identity.rs` is a singleton table holding one v4
UUID, generated on first read. Its module docs are explicit about what it is
for and what it costs: it exists so a lore-sync binding can say "this world,
on *this* deployment" (spec 034 FR-036g), and **it is published into an issue
on a repository that may be public** — which is why it is v4 rather than v7,
so it does not leak when the instance was first started (ADR-049's reasoning).

**Decision**: do not extend it, do not join to it, do not put the operator
record beside it.

**Rationale**: three separate reasons, any one sufficient.

1. **It is published.** A table whose contents are written into a public
   repository issue is the last place to add an operator's name, postal
   address and notice email. The blast radius of a careless `SELECT *` in
   `lore_sync` would be an operator's contact details on GitHub.
2. **It is machine identity, not human identity.** It answers "which
   deployment", not "who runs it". The two have different lifetimes: restoring
   a backup onto a second machine is *supposed* to carry the instance id (the
   module says so) and is emphatically not supposed to carry an assertion that
   the same person operates the copy.
3. **It has no provenance and must not gain any.** Principle III requires new
   tables to carry `created_by`/`updated_by`. `instance_identity` deliberately
   has neither, because there is no actor — it generates itself. FR-008
   requires the operator identity carry exactly that provenance. Adding
   nullable provenance columns to a self-generating singleton to serve a
   different concern is how a table stops meaning one thing.

## R3 — The operator record and the notice contact

**Decision**: they are settings rows like any other, under declared keys, not
a bespoke table:

```
operator.name                     required
operator.contact_email            required
operator.jurisdiction             optional (free prose)
notice.contact_name               required before publishing beyond a world
notice.contact_email              required before publishing beyond a world
notice.contact_postal_address     required before publishing beyond a world
```

`support_email` is **not** one of these. It stays where it is, a realm
manifest key, and is presented in the same surface (R6). The distinction is
real: `support_email` is "somebody will help you with the app"; the notice
contact is "this is the person a copyright claim is served on", which
17 U.S.C. § 512(c)(2) treats as a different thing with a different address
requirement.

**Rationale**: the change record (FR-008), the source reporting (FR-011) and
the redaction rule (FR-023/FR-027) are all properties this feature has to give
*every* setting. A dedicated `operator` table would need each of them
implemented a second time, and the second implementation is the one that
forgets redaction.

**Alternatives rejected**:

- *Fold the notice contact into `support_email`.* Legally distinct, and spec
  039's FR-053 gates publication on the notice contact alone — an instance may
  perfectly well have a support address and no designated agent.

## R4 — Making the published pages name a real operator

**What is there today**, and it is the sharpest constraint in this feature:

- `apps/web/src/legal/legalDocuments.ts` reads `legal/*.md` with
  `import.meta.glob(..., { eager: true })` — **resolved by Vite at build
  time**. There is no fetch and nothing is read at runtime. Its module docs
  state the invariant plainly: this text is *ours*, compiled in, which is why
  it may be rendered without the server's sanitizing markdown pipeline, and
  "nothing here should ever render a document that did not come from this
  glob."
- `legal/terms-of-service.md` carries **9** `[OPERATOR — …]` markers and
  `legal/privacy-policy.md` carries **5**.
- The DMCA designated agent is not prose at all: it is hard-coded JSX in
  `apps/web/src/pages/legal/DmcaCompliancePage.tsx:74-101` — literally
  `Copyright Agent, ThunderForge`, `[Configure via instance legal/compliance
  settings before launch]` and `dmca@thunderforge.example`.
- `apps/web/src/legal/__tests__/legalDocuments.test.ts` asserts the markers
  **survive rendering**, on the stated reasoning that "a page that omits who
  holds your data while reading as complete is worse than one that visibly has
  a blank."

**Decision**: keep the prose compiled in, and substitute a **closed set of
named tokens** into it at render time from the instance's settings, with three
rules:

1. The token set is closed and declared in code — one token per settings key,
   nothing arbitrary. A token whose setting is unset renders as the existing
   visible marker, not as an empty string. The existing test's invariant
   survives verbatim.
2. Substituted values are rendered as **text**, never as markup, and never
   through `LegalProse`'s inline parser. An operator name containing `[x](y)`
   must not become a link in the terms of service. This is the trust boundary
   the glob's docs are protecting, and it is the reason this needs an ADR
   rather than a helper function.
3. The DMCA agent designation stops being JSX literals and reads the same
   settings, since the page already documents those three values as "instance
   configuration values rendered as a definition list… a value the operator
   supplies rather than something anyone writes." That comment has been
   waiting for this feature.

**Alternatives rejected**:

- *Write the values into the markdown at setup.* Requires the running
  container to write into its own source tree, which is exactly the "operator
  edits markdown in a repository" failure the spec's Context rejects.
- *Serve `legal/*.md` from the server so it can be templated there.* Would
  move trusted prose onto a runtime read path and force it through the
  sanitizing pipeline, undoing a deliberate decision documented at length in
  `legalDocuments.ts`. It also gains nothing: the substitution is the only
  dynamic part.
- *Free-form template variables.* An operator-authored template in a legal
  document is an injection surface in the one document nobody re-reads.

**The part this does not solve is in § D2.**

## R5 — One precedence rule, and the fact that there are already two mechanisms

**What is there today**: spec 007/ADR-041's "environment wins" is implemented
by **materialisation**, not by resolution. `admin.rs:280`
`materialize_env_oauth_providers` parses `OAUTH_*` at startup, writes
`oauth_providers` rows with `config_source = "env"`, and
`admin.rs:219-222` refuses to let an admin update overwrite an env-sourced
row. `repo_host.rs`, by contrast, reads the environment **per use** and has no
row at all.

**Decision**: FR-010's universal rule is implemented as a **resolver**, read
per request, in a new `settings` module:

```
resolve(key) -> Resolved { value, source: Environment | Instance | Default }
```

Environment beats the instance row beats the declared default. `Config::from_env()`
and `materialize_env_oauth_providers` are **not** rewritten; instead OAuth's
`config_source` column is the same answer expressed differently, and the
settings surface reports both through one `source` field.

**Rationale**: FR-007 requires a change to take effect "without redeploying or
restarting", which a resolver gives for free and a startup materialisation
cannot. FR-024 forbids requiring an existing deployment to be reconfigured,
which rewriting OAuth's mechanism would risk for no requirement's benefit.

**Consequence recorded rather than hidden**: there are now two mechanisms
implementing one rule. That is a smaller cost than a migration of every
deployment's provider rows, and the observable behaviour — env wins, the
source is visible, an env-fixed field is not offered as editable — is
identical. An SDL-level test asserts both surfaces report a source from the
same enum, so they cannot drift on the vocabulary.

**Alternatives rejected**:

- *Materialise everything into rows at startup, like OAuth.* Breaks FR-007 for
  every setting, and makes "the environment set this" indistinguishable from
  "an admin typed the same value" once the row exists.
- *Resolve everything, and drop OAuth's materialisation.* A migration of live
  provider rows on every deployment, for a refactor. FR-024 exists to prevent
  exactly this.

## R6 — What the settings surface covers, and what it only *presents*

**Decision**: the resolver reads three backing stores behind one interface,
decided per declaration:

- `Row` — `instance_settings`. Everything new.
- `ManifestFile` — the six `editable_manifest_keys()`, still read from and
  written to `manifest.json` through the existing `admin.rs` path.
- `Derived` — values that are not stored at all (readiness, R11).

**The surface already has half of this.** The GraphQL query `systemManifest`
returns `entries { key value editable }` — an `editable` flag per key, which
`ManifestEditor.tsx` already renders. FR-009 and FR-011 are that flag grown a
reason: `editable` becomes `source` plus `fixedBy`, so a key an environment
variable has taken over reads "fixed by `THUNDERFORGE_SUPPORT_EMAIL`" rather
than being greyed out for no stated reason.

**Rationale**: this is the spec's "gathers them into the same surface rather
than replacing them", made mechanical. An operator sees one list; a manifest
key is still a manifest key underneath, so nothing about the existing admin
manifest editor or its seed file changes.

**Consequence**: the six manifest keys gain env-variable names they did not
have (`THUNDERFORGE_REALM_NAME`, `THUNDERFORGE_SUPPORT_EMAIL`, …) because
FR-010 says *every* setting, and the spec's edge case "an operator wants no
configuration UI at all and to set everything by environment" is only true if
that is literally true. When such a variable is set, the manifest editor shows
the key as fixed rather than editable — FR-009.

## R7 — Mail: there is none, and what adding one costs

**Confirmed by search, not assumed.** Across every `Cargo.toml` in the
35-crate workspace, `Cargo.lock`, and all Rust/TS sources: no `lettre`, no
`smtp`, no mailer, no queue, no template, no `SMTP_*` environment variable.
The only occurrences of the word in the repository are in this spec and its
checklist. `users.email` is stored and has never been sent to. The
"notification" hits are all PostgreSQL `LISTEN`/`NOTIFY`; the "outbox" hits
are all the client-side offline mutation queue in
`crates/thunderforge-cache-browser/src/outbox.rs`. **There is no server-side
job table, queue or retry machinery of any kind except one** — see R9.

So this is a new subsystem, and the plan treats it as the largest single piece
of the feature.

**Decision (crate)**: `lettre` with `tokio1-rustls-tls` and
`smtp-transport`, default features off.

**Rationale**: it is the only maintained SMTP client in the Rust ecosystem
with a native async Tokio transport, and the TLS choice matters here more than
the crate choice does. The first-party dependency tree is rustls throughout —
`reqwest 0.13.4` is declared `default-features = false, features = ["form",
"json", "rustls"]`, and `hyper-rustls`/`tokio-rustls` are what the lock
resolves. `native-tls`/`openssl` appear only through the legacy `websocket
0.27.1` chain. Pulling OpenSSL in as a *first-party* dependency would add a
system library to every build and container image for one subsystem.

**Alternatives rejected**:

- *An HTTP mail API (SES, SendGrid, Postmark, Mailgun).* The spec's assumption
  is explicit — "mail means SMTP first. It is what a self-hoster has". An API
  provider is an account a self-hoster does not have. The transport seam in R8
  leaves the door open without opening it.
- *Shell out to `sendmail`.* Assumes an MTA in the container. The images this
  product ships do not have one, and the failure is silent.
- *Write SMTP by hand.* Authentication, STARTTLS negotiation and MIME encoding
  are not the interesting part, and getting them subtly wrong produces mail
  that some receivers accept and others reject — the exact failure mode
  FR-014 and the "connects but is rejected by the receiving side" edge case
  are about.

## R8 — Mail: the transport seam, and how a test proves delivery

**Decision**: `AppState` holds `mail: Arc<dyn MailTransport>`. Two
implementations ship: `SmtpTransport` (product) and `CapturingTransport`
(test-support only), plus the `Unconfigured` case which is a transport that
refuses with a named reason rather than an `Option` the callers have to
remember to check.

**Rationale**: this is not a new pattern here. `AppState` already holds
`adjudicator: Arc<LocalAdjudicator>` with a `remote.rs` sibling for exactly
this reason, and `test_support.rs` already swaps it. `oauth.rs` gets its
test-only provider from a **seeded row** rather than a `#[cfg(test)]` branch —
spec 036's FR-023 states the principle: a test that exercises a test-only
branch proves the branch. The transport seam is the same shape one layer down.

**Decision (how a test proves it), three levels, and all three are needed**:

1. **Unit** — `CapturingTransport` collects messages in memory. Proves the
   callers: that a failure is recorded, that a body is never logged, that the
   outbox transitions. Proves nothing about SMTP.
2. **Integration** — a real `SmtpTransport` against a real SMTP server in the
   dev stack. **Mailpit** added to `compose.yml` beside `postgres` and
   `rustfs`. It speaks real SMTP on 1025 and exposes a JSON API on 8025 that a
   test reads the delivered message from. This is the level that proves
   `lettre` is wired correctly, that STARTTLS negotiation works, and that the
   From address is what the operator set.
3. **End-to-end** — the operator flow: configure mail in the admin surface,
   press "send a test message", assert the outcome in the browser **and**
   assert the message arrived in Mailpit's API. This is the level FR-013 is
   actually written about: "a test message that proves it works before
   anything depends on it."

Level 2 is the one it would be tempting to skip, and skipping it is how a mail
subsystem ships that has only ever talked to a mock.

**Alternatives rejected**:

- *A fake SMTP server inside the Rust test process.* Would need TLS and AUTH
  handled by hand to be worth anything, at which point it is a second SMTP
  implementation with none of the review a real one has had.
- *Only the capturing transport.* Proves the code around the transport and
  nothing about the transport, which is where the failures are.
- *Point tests at a real mailbox.* Not reproducible, not offline, and the
  first e2e run in CI would be an abuse report.

**Cost accepted**: one more container in `compose.yml` and one more service
per e2e shard in `scripts/e2e-parallel.mjs`, which already gives each shard a
port for a backend, a vite server and a bucket. It is recorded in the plan's
Complexity Tracking rather than waved through.

## R9 — Mail: the outbox, and how a failure becomes visible

FR-015 and FR-016 are the requirements that decide the shape here: with no
mail configured the instance **must not silently discard** messages, a failure
must be visible to an operator, and the record must not expose a message's
contents to anyone it was not addressed to.

**What is there today to copy**: exactly one thing, and it is a good one.
`src/server/src/lore_sync/schedule.rs` is a `tokio::time::interval` loop
spawned once from `src/app/src/main.rs`, with:

- `const BACKOFF_SECONDS: [i64; 9] = [30, 60, 120, 300, 600, 900, 1800, 2700, 3600]`
  and `next_attempt_after(last_attempt, consecutive_failures)`;
- selection extracted as `due_now(conn, now) -> Result<Vec<Due>, String>`, a
  function outside the spawned task, on stated reasoning: "a promise enforced
  by an `if` inside a loop inside a spawned task is a promise nothing can
  test";
- per-attempt outcomes persisted in `lore_sync_runs`.

**Decision**: `mail_outbox`, one row per message, with a
`mail::schedule::spawn_mail_task` modelled directly on that file — same
backoff shape, same extracted `due_now`, same `spawn_*_task` registration in
`src/app/src/main.rs` beside the other five. Sending is **always** through the
outbox; nothing calls the transport directly.

**Rationale**: this is what makes FR-015 true by construction. A feature that
wants to tell somebody something inserts a row. If mail is unconfigured the
row sits in `blocked` with a reason an operator can read — which is the
difference between "not sent" and "silently discarded", and it means specs
035, 037 and 039 can be built against a queue that exists whether or not the
operator has configured a server yet.

**Decision (privacy)**: the outbox stores the rendered body encrypted with
`crypto.rs` (`v1.<nonce>.<ciphertext>`, AES-256-GCM), and **no administrative
surface returns it, ever**. What an operator sees is: recipient, purpose,
state, attempt count, last failure reason, timestamps. Subject is stored
redacted-by-default and shown only for messages the instance itself generated
about itself (a test message), never for a message about a person.

**Stated honestly**: encryption here defends against the admin surface, the
logs and a leaked backup — not against the operator, who holds
`THUNDERFORGE_SECRET` and the database. FR-016's "without exposing the
contents of somebody's message to anyone it was not for" is a requirement
about the *product's surfaces*, and this is what the product can actually
promise. Claiming more would be a lie in a security control.

**Alternatives rejected**:

- *Send inline from the calling mutation.* A slow or failing SMTP server
  becomes a slow or failing mutation, and a failure is lost with the request.
- *An external queue (Redis, RabbitMQ, a job runner).* `schedule.rs` already
  rejected this for lore sync with the reason that applies here verbatim: "a
  queue or an external scheduler would be a new deployment component for a
  loop the process can hold itself." A self-hosted VTT should not gain a
  broker to send a password reset.
- *Store the body in the clear.* One `SELECT *` in a diagnostic away from
  FR-016 being false.
- *Do not store the body at all, re-render on retry.* Requires every calling
  feature to be able to reproduce a message identically at an arbitrary later
  time, which makes the retry a different message than the one that failed.

## R10 — GitHub applications at two scales

**What is there today**: `repo_host.rs` and its `RegistrationProblem` enum are
already, almost exactly, the diagnostic contract FR-023 asks for — every
variant names a variable, `guidance()` produces prose naming variables and
never values, `registration_from_env` returns **every** problem rather than the
first "so an operator fixes their configuration in one pass", the private key
is accepted in three forms with a documented precedence (`_FILE` > `_BASE64` >
inline), and it is **parsed at startup, not at first use** because "a key that
is present but not a key is the failure a presence check calls configured."

**Decision (naming)**: `GLOBAL_GITHUB_APP_*` for the instance-wide
application — the name spec 037 already proposed, adopted rather than
re-invented — with `SYNC_GITHUB_APP_*` and `FEEDBACK_GITHUB_APP_*` as the
subsystem forms. Instance-stored equivalents live in `instance_settings` under
`github_app.global.*` and `github_app.<subsystem>.*`, with the private key
encrypted through `crypto.rs`.

**Decision (resolution order)**: **scope is the outer axis, source is the
inner one.**

```
1. subsystem, environment      SYNC_GITHUB_APP_*
2. subsystem, instance         github_app.sync.*
3. global,    environment      GLOBAL_GITHUB_APP_*
4. global,    instance         github_app.global.*
```

**The spec does not decide this and it has to be decided.** FR-010 says the
environment wins over the instance; FR-019 says the subsystem wins over the
global. When an operator sets a global app in the environment *and* a
subsystem app in the admin screens, those two rules point opposite ways.
Putting scope outside means the more specific intent wins and the source rule
applies within it. The alternative — source outside — would let a broad
`GLOBAL_GITHUB_APP_*` silently override the subsystem-specific application an
operator had just deliberately configured, which is the surprise this whole
spec exists to eliminate. Recorded here, and it belongs in the ADR.

**Decision (an application resolves whole, never field by field)**: a
subsystem application that is partially specified does **not** borrow the
missing halves from the global one. It resolves as *incomplete*, is reported
as incomplete naming the missing variables, and the subsystem falls back to
the global application entire, saying so.

**Rationale**: US5's own acceptance scenario 4 asks for exactly this — "rather
than a half-configured application being silently completed". A client id from
one registration with a private key from another is not an application; it is
an authentication failure that reads like a bad key, which is the failure mode
`repo_host.rs` already warns about for the id/slug confusion.

**Decision (validation, FR-022)**: "validated when configured" means (a) the
private key is **parsed** on save, exactly as `registration_from_env` parses
it at startup, and (b) an explicit, operator-initiated live check against the
host is offered and its result recorded. It does **not** mean an automatic
network call on save. A save that requires reachable GitHub cannot be made
from an air-gapped or firewalled deployment, and a configuration screen that
refuses to store a correct value because a network was down is worse than one
that stores it and says it has not been proven yet.

**Decision (FR-024, back-compat)**: `SYNC_GITHUB_APP_*` keeps working
unchanged and keeps winning for lore sync. `registration_from_env()` keeps its
signature and gains a sibling, `registration_for(subsystem)`, that applies the
order above; the existing function becomes the first step of it. A deployment
that sets only `SYNC_GITHUB_APP_*` sees no behaviour change at all, which is
the acceptance test for FR-024.

## R11 — Readiness is derived, and the publish gate is not advisory

**Decision**: readiness is computed on demand from the declaration list — no
table, no cache, no stored "is ready" flag. Each declared setting names the
capability it enables and what a person sees when it is missing; the readiness
report is that list, filtered.

**The precedent to copy** is `instanceRepositoryIntegration`
(`graphql/queries/lore_sync.rs:285`), which returns
`RepositoryIntegrationStatus { configured: Boolean!, operatorGuidance:
String }` by asking `registration_from_env()` and joining every
`RegistrationProblem::guidance()`. That is one subsystem answering "can I do
this, and if not what do I set" in exactly the shape FR-025 wants for all of
them. The readiness report is that query generalised over the declaration
list, and `instanceRepositoryIntegration` becomes one row in it rather than a
separate answer.

**Rationale**: a stored readiness flag is a second source of truth for a
question whose answer is a pure function of configuration, and it goes stale
in exactly the direction that hurts — reporting ready when a credential was
revoked.

**Decision (FR-026 / spec 039 FR-053)**: the gate on publishing beyond a world
is enforced **server-side at the mutation boundary**, in the same place the
authorization decision for that mutation is made, not in the client. The
operations gated are the ones that make content reachable outside its world —
collection shares and singleton artefact shares (`mutations_collection_shares.rs`,
`mutations_actor_shares.rs`, `mutations_item_shares.rs`,
`mutations_ability_shares.rs`), which ADR-069/070/071 already treat as one
family. A refusal names the missing setting.

**Rationale**: Principle III. Spec 039 depends on this being a gate rather than
a warning, and the checklist notes the requirement is stated in both specs on
purpose. A client-side check is a warning.

**Explicitly not gated**: anything inside a world. The spec is emphatic —
"playing privately MUST remain possible; an instance with nobody to notify is
a private instance, not a broken one."

**Redaction (FR-027)**: readiness reports which key is unset and what to set,
by name. It never reports a value, a fragment, or a length — including for
settings that *are* set, where the temptation is a masked preview. There is no
`sk-…3f9`. A declaration marked secret has exactly two renderings anywhere in
the product: *set* and *not set*.

## R12 — First run: extend the ceremony, do not replace it

**What is there today**, and it is more than the spec's summary suggests:

- `auth/admin_bootstrap.rs` generates a one-time code at startup while setup
  is incomplete, hashes it into `admin_bootstrap_setup` (a singleton, `id = 1`,
  columns `setup_completed_at`, `admin_code_hash`, `admin_code_generated_at`),
  and logs `visit: http://127.0.0.1:5173/setup/<code>`.
- `auth/admin_setup.rs` exposes **REST**, not GraphQL: `setup_status`,
  `admin_setup_basic`, `admin_setup_oauth_start`, `admin_setup_oauth_callback`.
  `admin_setup_basic` takes username, email, password and the admin code —
  that is the whole ceremony.
- `SetupCallbackPage.tsx` finishes the OAuth-provisioned-administrator path.
- The web side is `apps/web/src/pages/setup/SetupPage.tsx` (782 lines) at
  `/setup` and `/setup/:code`, reached through `useSetupStatus` and
  `services/auth.ts` — **REST only; the setup screen touches no GraphQL.**
- Routes are registered in `auth/mod.rs::router()`:
  `GET /authentication/setup/status`, `POST /authentication/setup/basic`,
  `POST /authentication/setup/oauth/{provider_key}/start`,
  `GET /authentication/setup/oauth/{provider_key}/callback`.

**Decision**: setup becomes a multi-step flow over the existing endpoints
rather than a new subsystem. Each step **writes its settings rows as it is
completed**, so FR-006's resumability is a consequence of the storage rather
than a session to keep alive. Completion — `setup_completed_at` — is written
only when every required declaration resolves and the second factor is
confirmed.

**Rationale**: the bootstrap code, the singleton, the "setup has already been
completed" guard and the OAuth-provisioned-admin path all already exist. The
requirement that is missing is what setup *collects*, not how it is reached —
so the work is fields and steps, not a new subsystem. Two defects in what is
there are fixed on the way past, below.

**Decision (the abandoned-setup defect, spec Edge Case 2)**: today,
`ensure_admin_bootstrap_code` generates a **fresh** code on every start while
setup is incomplete. Close the browser, restart the container, and the link
the operator had is dead with no message saying why. Fix: reuse the existing
unconsumed code when one is present, and offer a deliberate regeneration. This
is a small change and it is the difference between FR-006 being true and being
claimed.

**Decision (the concurrency defect — the spec's edge case "two people open
setup at the same moment on a fresh deployment")**: `admin_setup_basic` checks
"does an admin exist", then checks username and email uniqueness, then
inserts — **three statements on one pooled connection with no
`conn.transaction(...)` around them.** Two concurrent POSTs holding the same
valid bootstrap code can both pass the check. The only backstop is the unique
constraint on `users.username` / `users.email`, which surfaces as a generic
`500 setup_error`. `instance_identity::instance_id` in the same codebase shows
the pattern that fixes it — `INSERT … ON CONFLICT DO NOTHING` against a
singleton, chosen precisely because "a check-then-insert would have a window
between the two, and first launch is exactly when several requests arrive at
once". Setup completion is wrapped in one transaction, and the losing caller
gets `409 setup_complete` rather than a 500. Small, and it is the spec's own
edge case.

**Decision (FR-002a, the second factor)**: **spec 041 owns the enrolment flow
and this feature does not design it.** 040 owns one thing about it: setup's
definition of finished. Concretely — `setup_completed_at` is not written until
`users.two_factor_confirmed_at` is non-null for the first administrator. The
seam is one predicate, named in `contracts/setup.md`, and 041's FR-001a
("one flow, reachable from account settings, from first-run setup, and from a
sign-in that requires enrolment") is what supplies the step. See § D1 for what
this means for sequencing, because it is a real dependency and not a note.

## R13 — Recording a change

**Decision**: `instance_setting_changes`, append-only: the key, who, when, the
source it was written to, and the previous value **redacted according to the
declaration** — the literal previous value for a non-secret setting, and the
word `set`/`not set` for a secret one.

**Rationale**: FR-008 wants "what it was before" and FR-023 forbids ever
printing a credential. For a secret those are in direct conflict, and the
resolution is that the audit records the *transition*, not the value. An
operator investigating a bad notice address needs the old address; nobody
needs the old SMTP password, and storing it doubles the number of places it
can leak from.

**Rejected**: reusing `instance_access_events`. It is a typed, `CHECK`-
constrained record of three specific access events, and its own migration
comment records a deliberate omission of any email column. Widening it into a
general audit log would make that constraint meaningless.

## R14 — Upgrading an instance that predates a required setting

**Decision**: a declaration's `required` flag is enforced **at the point of
use and at setup**, never at startup. The server always boots. A required
setting that is unset produces: a readiness gap, a refusal from the operations
that need it (with the name of what to set), and a prompt in the admin
surface. It never produces a failure to start.

**Rationale**: FR-028 and SC-009 verbatim. It is also what
`repo_host.rs` already does for `git`: the module says the server "says so at
startup if it is missing, and does not refuse to boot: every other part of the
product works without it." Same rule, generalised.

## R15 — What "blank or an obvious placeholder" means (FR-004)

**Decision**: a small, declared refusal list per setting kind, not a heuristic.
For a contact email: syntactically invalid, `.local`/`.example`/`.invalid`/
`.test` reserved TLDs (RFC 2606/6761), and the shipped defaults themselves —
`stewards@thunderforge.local`, `dmca@thunderforge.example`. For an operator
name: empty after trimming, or one of the shipped placeholder strings.

**Rationale**: `config/realm-defaults.json` ships `support_email:
"stewards@thunderforge.local"` and the DMCA page ships
`dmca@thunderforge.example`. The single most likely way to publish a page
naming nobody is to accept the default unchanged, so the defaults are
explicitly on the refusal list. A cleverer placeholder detector would reject
somebody's real name; a reserved-TLD rule and a defaults list are both exactly
right and both explainable in the error message.

---

## § D — Where the spec and the codebase disagree

Recorded rather than resolved by fiat, because each of these is a decision
somebody should make deliberately.

### D1 — FR-002a makes 040's headline story depend on unbuilt work

FR-002a requires setup to take the first administrator through enrolling a
second factor and forbids completion without it. Spec 041 owns that flow, and
041 states plainly that **no enrolment interface exists anywhere in the
application** — "the only two-factor code in `apps/web/src` is the login
challenge step and the admin policy switch; the setup endpoints are called
from nowhere."

So US1 — "from empty database to a real instance, in one pass", the story the
spec calls "the feature" — **cannot be completed until spec 041's US1 ships**.
Everything else in 040 can.

Three ways forward, and the plan takes the third:

1. Build 041's enrolment first, then 040. Correct ordering, and it delays
   every part of 040 that has nothing to do with 2FA.
2. Ship 040's setup without the gate and add it later. Rejected: it produces a
   released version whose administrators have no second factor, and 041's
   FR-031 then has to take them through enrolment on next sign-in — real work
   created by shipping in the wrong order.
3. **Build 040 in full, with the completion predicate present and the step
   supplied by 041.** `setup_completed_at` is gated on
   `two_factor_confirmed_at` from the first commit; until 041's flow exists,
   that gate is satisfiable only through the existing
   `two_factor_setup_start`/confirm endpoints, so 040 is *finishable* but not
   *pleasant*, and 041 makes it pleasant. The two land in either order and the
   invariant is never wrong.

This is the one place a reader should look before scheduling.

### D2 — FR-005 cannot be satisfied by collecting a name and an email

FR-005: "the instance's published pages MUST name the operator and the contact
that were entered, with no placeholder text and no file edited."

Of the 14 `[OPERATOR — …]` markers in `legal/`, **4 are data** that setup can
collect — operator name (×2), contact email (×2). The other **10 are prose an
operator has to write**, and no field collects them:

- the governing jurisdiction (`terms-of-service.md:124` — "This is not
  optional");
- how the operator will announce a change to the terms, and to the privacy
  policy;
- whether the instance is invite-only or private, said in the terms;
- anything specific to the community;
- the warranty disclaimer and the liability limitation, which `legal/README.md`
  flags as "the ones most likely to need changing" per jurisdiction;
- whether the instance is directed at children, and any minimum age.

Setup collecting five fields does not make the terms of service
placeholder-free, and pretending otherwise would produce a page that reads as
complete while its governing-law clause names nobody — which
`legal/README.md` identifies as strictly worse than a visible blank.

**Proposed reading, for the owner to confirm**: split the markers into
*fillable values* (required at setup, refused if placeholder — FR-004) and
*operator prose blocks* (optional, offered at setup, editable afterwards,
each one a readiness gap while unset). FR-005 then reads: the published pages
name the operator and the contact with no placeholder **in the values FR-002
collects**, and every remaining prose block is visibly outstanding and listed
in readiness. That is buildable, honest, and still removes the source edit.

**What is not proposed**: collecting free markdown from an operator and
rendering it into the legal pages through `LegalProse`. See R4's rule 2.

### D3 — FR-010's "every setting" is larger than it sounds

Applied literally, FR-010 requires an environment variable for every setting
in the product — including the six realm-manifest keys, which have never had
one, and the instance access policy from spec 035, which is deliberately a
runtime decision an operator changes from a screen while people are trying to
register.

R6 takes the literal reading (a declared env name for every setting, because
the spec's own edge case asks for an instance configured entirely by
environment). The consequence worth stating: **an operator who sets
`THUNDERFORGE_INSTANCE_ACCESS_POLICY` loses the ability to change the policy
from the screen**, which is the correct behaviour under FR-009 and will
nonetheless surprise somebody. It is surfaced as "fixed by the environment"
with the variable named, per FR-009, and that is the mitigation.

### D4 — FR-021 and US5.4 pull in opposite directions

FR-021: "where a resolved application draws on more than one source, the
operator MUST be able to see which value came from where" — which presupposes
field-by-field merging. US5's acceptance scenario 4 asks that a partially
specified application not be "silently completed".

R10 resolves it in favour of the acceptance scenario: applications resolve
whole. FR-021 is then satisfied at the granularity that exists — the operator
is shown which *application*, from which scope and which source, and which
variables an incomplete one is missing. If field-level merging is genuinely
wanted, FR-021 needs restating and US5.4 needs withdrawing; they cannot both
hold.

### D5 — FR-016's promise is bounded by who the operator is

FR-016 requires a delivery failure be visible "without exposing the contents
of a message to anyone it was not addressed to". The operator has the
database and `THUNDERFORGE_SECRET`, from which the encryption key is derived
(`crypto.rs`: "one key per instance"). R9 implements the strongest version the
product can actually enforce — nothing in any surface, log or diagnostic ever
returns a body — and this is written down so the requirement is not later read
as a claim that the operator cannot read their users' mail. On a self-hosted
instance, they can. Out of Scope already rules out an external secrets
manager, which is the only thing that would change this.

### D6 — `support_email`'s current default is already published

`config/realm-defaults.json` ships `support_email:
"stewards@thunderforge.local"` and every existing deployment has it unless
somebody changed it. FR-004 refuses placeholders *at setup*, which does not
run again on an existing instance. So an upgraded instance keeps a support
address at a reserved TLD, silently.

**Decision**: it becomes a readiness gap on upgrade, not a refusal (FR-028
forbids a refusal). The instance runs, and the administrator is told the
support address is still the shipped default. This is the mechanism SC-008 is
describing — "says so before somebody discovers it at the moment they needed
it".

**And a sharper finding underneath it**: `support_email` is currently read by
**nothing**. It is defined in `realm-defaults.json`, listed in
`editable_manifest_keys()`, editable through `updateManifestKey`, and there is
no consumer of it anywhere in the server or the web client. It is a
stored-and-editable setting that has never been displayed to anybody. FR-002
requires setup to collect a support address; this feature is also the first
thing that will ever use one, which means "the support address is wrong" has
never been observable. The readiness entry and a place it is actually rendered
are both part of the work.
