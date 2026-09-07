# Phase 1 Data Model: Instance Setup and Configuration

Four kinds of state, and the boundaries between them are the design: **one
declaration list in code** that has no table at all, **three durable tables**,
**two existing stores that are presented and not moved**, and **one derived
report** that is deliberately never stored.

---

## 0. The setting declaration — code, not data

`src/server/src/settings/registry.rs`. This is the centre of the feature and it
is not a table, because a setting that can be added by inserting a row is a
setting that can be added without an env name, without a redaction rule and
without saying what it enables — which is FR-012 failing quietly.

```text
SettingDeclaration
  key            stable identifier            "notice.contact_email"
  kind           Text | Email | Url | Port | Bool | Enum(..) | Secret | Prose
  backing        Row | ManifestFile(key) | Derived
  env_var        the name the environment sets it by   "THUNDERFORGE_NOTICE_CONTACT_EMAIL"
  requirement    Optional
                 | RequiredAtSetup
                 | RequiredFor(capability)      -- unset is a readiness gap, never a boot failure
  secret         bool — decides encryption at rest AND every rendering, everywhere
  default        Option<value>                 -- the third precedence tier
  validators     [rule]                        -- FR-004; see § 5
  capability     what this setting enables, and what a person sees without it
  since          the version that introduced it -- FR-028's "an upgrade must ask, not break"
```

**Invariants that hold because this is one list:**

| Invariant | Requirement |
|---|---|
| Every setting has an environment variable name | FR-010, and the edge case "an operator wants no configuration UI at all" |
| Every setting can report its source | FR-011 |
| A new setting inherits precedence, redaction and readiness by existing | FR-012 |
| A `secret` setting has exactly two renderings anywhere: **set** / **not set** | FR-023, FR-027, SC-007 |
| A required setting that is unset is a readiness gap, never a startup failure | FR-028, SC-009 |

### The settings at first landing

| Key | Kind | Backing | Env var | Requirement |
|---|---|---|---|---|
| `operator.name` | Text | Row | `THUNDERFORGE_OPERATOR_NAME` | RequiredAtSetup |
| `operator.contact_email` | Email | Row | `THUNDERFORGE_OPERATOR_CONTACT_EMAIL` | RequiredAtSetup |
| `operator.jurisdiction` | Prose | Row | `THUNDERFORGE_OPERATOR_JURISDICTION` | RequiredFor(publish_terms) |
| `notice.contact_name` | Text | Row | `THUNDERFORGE_NOTICE_CONTACT_NAME` | RequiredFor(publish_beyond_world) |
| `notice.contact_email` | Email | Row | `THUNDERFORGE_NOTICE_CONTACT_EMAIL` | RequiredFor(publish_beyond_world) |
| `notice.contact_postal_address` | Text | Row | `THUNDERFORGE_NOTICE_CONTACT_ADDRESS` | RequiredFor(publish_beyond_world) |
| `legal.terms_change_notice` | Prose | Row | — | Optional |
| `legal.community_addendum` | Prose | Row | — | Optional |
| `legal.minimum_age_statement` | Prose | Row | — | Optional |
| `mail.enabled` | Bool | Row | `THUNDERFORGE_SMTP_ENABLED` | Optional |
| `mail.host` | Text | Row | `THUNDERFORGE_SMTP_HOST` | RequiredFor(send_mail) |
| `mail.port` | Port | Row | `THUNDERFORGE_SMTP_PORT` | RequiredFor(send_mail) |
| `mail.security` | Enum(none, starttls, implicit) | Row | `THUNDERFORGE_SMTP_SECURITY` | RequiredFor(send_mail) |
| `mail.username` | Text | Row | `THUNDERFORGE_SMTP_USERNAME` | Optional |
| `mail.password` | **Secret** | Row | `THUNDERFORGE_SMTP_PASSWORD` | Optional |
| `mail.from_address` | Email | Row | `THUNDERFORGE_SMTP_FROM_ADDRESS` | RequiredFor(send_mail) |
| `mail.from_name` | Text | Row | `THUNDERFORGE_SMTP_FROM_NAME` | Optional |
| `github_app.global.client_id` | Text | Row | `GLOBAL_GITHUB_APP_CLIENT_ID` | Optional |
| `github_app.global.slug` | Text | Row | `GLOBAL_GITHUB_APP_SLUG` | Optional |
| `github_app.global.private_key` | **Secret** | Row | `GLOBAL_GITHUB_APP_PRIVATE_KEY[_FILE|_BASE64]` | Optional |
| `github_app.sync.*` | as above | Row | `SYNC_GITHUB_APP_*` | Optional |
| `github_app.feedback.*` | as above | Row | `FEEDBACK_GITHUB_APP_*` | Optional (spec 037) |
| `realm_name` | Text | ManifestFile | `THUNDERFORGE_REALM_NAME` | Optional |
| `support_email` | Email | ManifestFile | `THUNDERFORGE_SUPPORT_EMAIL` | RequiredAtSetup |
| `welcome_message` | Text | ManifestFile | `THUNDERFORGE_WELCOME_MESSAGE` | Optional |
| `interface_pack_id` | Text | ManifestFile | `THUNDERFORGE_INTERFACE_PACK_ID` | Optional |
| `asset_pack_id` | Text | ManifestFile | `THUNDERFORGE_ASSET_PACK_ID` | Optional |
| `default_game_system_id` | Text | ManifestFile | `THUNDERFORGE_DEFAULT_GAME_SYSTEM_ID` | Optional |
| `instance.access_policy` | Enum(open, invite_only, closed) | Row (spec 035's table) | `THUNDERFORGE_INSTANCE_ACCESS_POLICY` | Optional |

The six `ManifestFile` rows are the existing `editable_manifest_keys()`,
unchanged underneath: still seeded from `config/realm-defaults.json`, still
written through `admin::update_manifest_key`, still backfilled by
`backfill_missing_seeds`. What changes is that they now resolve through the
same resolver and report a source. `instance.access_policy` likewise keeps
spec 035's own table and its `instance_access_events` audit trail; the registry
declares it so it is visible in one list and obeys FR-009/FR-010, and its
writes still go through `update_instance_access_policy`.

**`support_email` is currently read by nothing** — it is stored, editable, and
has never been displayed. This feature is the first consumer.

---

## 1. `instance_settings` — durable, one row per configured value

```text
instance_settings
  key            text        PRIMARY KEY   -- must be a declared key; an undeclared row does not resolve
  value          text        NOT NULL      -- ciphertext (v1.<nonce>.<ct>) when the declaration says secret
  updated_by     uuid        NULL REFERENCES users(id)
  updated_at     timestamp   NOT NULL
  created_by     uuid        NULL REFERENCES users(id)
  created_at     timestamp   NOT NULL
```

- **Provenance columns per Principle III and ADR-009/ADR-010.** Nullable
  because a row may be written by the system during an upgrade with no actor;
  a null actor renders as "the instance" and never as a blank.
- **A row for a key the registry does not declare is inert.** It is not
  deleted (an operator downgrading and upgrading again would lose their
  values), it simply does not resolve and is reported as unrecognised in
  readiness. Same posture ADR-041 took for provider rows: retain, do not
  delete.
- **Encryption at rest** uses `crypto.rs` unchanged —
  `encryption_key_from_config_secret(THUNDERFORGE_SECRET)` then
  `encrypt_secret`. The spec's assumption "secrets are stored the way the
  instance already stores secrets" resolves to this module, which is already
  what `users.two_factor_secret_encrypted` and
  `user_oauth_accounts.access_token_encrypted` use. Note the existing
  counterexample: `oauth_providers.oauth_client_secret` is plaintext. New
  secrets do not follow it.
- **Consequence inherited, not introduced**: rotating `THUNDERFORGE_SECRET`
  makes every stored ciphertext unreadable, which `crypto.rs` documents. A
  secret that fails to decrypt resolves as *unset* with a readiness gap naming
  the key — never as an empty string, and never as a panic.

## 2. `instance_setting_changes` — append-only

```text
instance_setting_changes
  id                  uuid       PRIMARY KEY (v7)
  key                 text       NOT NULL
  previous_value      text       NULL   -- literal for a normal setting; 'set'/'not set' for a secret
  new_value           text       NULL   -- same rule
  redacted            bool       NOT NULL -- true when the declaration is secret; the reader is told
  changed_by          uuid       NULL REFERENCES users(id)
  changed_at          timestamp  NOT NULL
  source              text       NOT NULL CHECK (source IN ('setup','admin','system'))
```

FR-008 wants "what it was before"; FR-023 forbids ever printing a credential.
For a secret those conflict, and the resolution is that the record captures the
**transition**, not the value: an operator investigating a stale notice address
needs the old address, and nobody needs the old SMTP password. `redacted` is
stored rather than recomputed so the record stays truthful if a declaration's
`secret` flag ever changes.

Index on `(key, changed_at DESC)` — the question is always "what happened to
this setting", never "what happened at 14:02".

Reusing spec 035's `instance_access_events` was rejected: it is a
`CHECK`-constrained record of three specific access events, and its migration
comment records a deliberate omission of any email column. Widening it would
make that constraint meaningless.

## 3. `mail_outbox` — every message, whether or not it can be sent

```text
mail_outbox
  id                     uuid       PRIMARY KEY (v7)
  purpose                text       NOT NULL   -- 'test', and whatever 035/037/039 name later
  to_address             text       NOT NULL
  subject_encrypted      text       NOT NULL
  body_encrypted         text       NOT NULL
  state                  text       NOT NULL CHECK (state IN ('queued','blocked','sending','sent','failed'))
  attempts               integer    NOT NULL DEFAULT 0
  last_attempt_at        timestamp  NULL
  next_attempt_at        timestamp  NULL
  last_failure_reason    text       NULL       -- operator-facing prose; never a password, never a fragment
  sent_at                timestamp  NULL
  created_by             uuid       NULL REFERENCES users(id)
  created_at             timestamp  NOT NULL
```

### States, and why `blocked` is separate from `failed`

```text
                    ┌──────────────────────────────────────────┐
   enqueue ──> queued ──(mail configured)──> sending ──> sent   │
      │           │                             │               │
      │           │                             └──(error)──> queued  [retry, backoff]
      │           │                                              │
      │           └──(no mail configured)──> blocked             └──(attempts exhausted)──> failed
      │                                          │
      └──────────────────────────────────────────┘  (configure mail → blocked returns to queued)
```

`blocked` is the state FR-015 is about. With no mail configured the message is
**not discarded and not failed** — it is visibly waiting on a setting, and
configuring mail releases it. That distinction is the difference between "the
instance could not tell them yet" and "the instance threw it away", and it is
why a feature that wants to notify somebody may enqueue unconditionally.

`failed` is terminal after the backoff array is exhausted, and stays visible.

### Backoff

`const BACKOFF_SECONDS: [i64; 9] = [30, 60, 120, 300, 600, 900, 1800, 2700, 3600]`
— the same array `lore_sync/schedule.rs` uses, deliberately, so there is one
retry curve in the product to reason about. Selection is
`due_now(conn, now) -> Result<Vec<Due>, String>`, a function **outside** the
spawned loop, for the reason that file states: "a promise enforced by an `if`
inside a loop inside a spawned task is a promise nothing can test."

### What an operator can see, and what nobody can

| Field | In the admin surface | Reason |
|---|---|---|
| `to_address` | **yes** | An operator investigating delivery needs to know whether it went to the right address |
| `purpose`, `state`, `attempts`, timestamps, `last_failure_reason` | **yes** | FR-015, FR-016 |
| `subject_encrypted` | **only for `purpose = 'test'`** | A subject about a person is content |
| `body_encrypted` | **never, by any surface, ever** | FR-016 |

Both are stored encrypted with `crypto.rs` because retry needs the message it
failed to send, and re-rendering later would make the retry a different
message. The honest boundary is recorded in research.md § D5: this defends the
admin surface, the logs and a leaked backup — not the operator, who holds the
key and the database.

---

## 4. Readiness — derived, never stored

```text
ReadinessReport
  capabilities: [ Capability {
      key            "send_mail" | "publish_beyond_world" | "publish_terms" | "sync_lore" | "feedback"
      available      bool
      gaps: [ Gap { setting_key, env_var, what_to_set, what_is_limited } ]
  } ]
  unrecognised_settings: [key]      -- rows the registry no longer declares
  source_flips: [ { setting_key, was, now } ]   -- an env var vanished or appeared since last boot
```

No table, no cache, no `is_ready` column. It is a pure function of the registry
and the resolver, computed per request. A stored flag is a second source of
truth for a question that has one, and it goes stale in the direction that
hurts: reporting ready after a credential was revoked.

`instanceRepositoryIntegration { configured, operatorGuidance }` — the existing
lore-sync answer built from `RegistrationProblem::guidance()` — becomes one
capability in this report rather than a separate surface with a separate
vocabulary.

**Redaction (FR-027)**: a gap names a key and a variable. It never names a
value, a fragment or a length, **including for settings that are set**. There
is no masked preview anywhere in this feature.

`source_flips` exists for the spec's edge case "a container is redeployed with
a fresh environment and an existing database": the value silently reverts from
the environment to the stored row, and the operator is told that it did rather
than discovering it.

---

## 5. Validation rules, traced to requirements

| Rule | Requirement |
|---|---|
| A required setting that is blank after trimming is refused at setup | FR-004 |
| A contact email at a reserved TLD (`.local`, `.example`, `.invalid`, `.test`) is refused | FR-004 |
| The shipped defaults `stewards@thunderforge.local` and `dmca@thunderforge.example` are refused by name | FR-004, research.md § R15 |
| Setup does not complete while any `RequiredAtSetup` declaration is unset | FR-002, FR-003 |
| Setup does not complete while the first administrator's `two_factor_confirmed_at` is null | FR-002a (flow owned by spec 041) |
| Setup completion is one transaction, and the second concurrent caller gets `409 setup_complete` | Edge case: two people open setup at once |
| A bootstrap code that has not been consumed is reused across a restart, not regenerated | FR-006 |
| An environment-set value cannot be written through the admin surface | FR-009 |
| Environment beats the stored row beats the declared default, for every declared setting | FR-010 |
| Every resolved value reports `Environment` \| `Instance` \| `Default` | FR-011 |
| A secret renders as `set` / `not set` and nothing else, in every surface | FR-023, FR-027 |
| A change to any setting writes an `instance_setting_changes` row | FR-008 |
| A message is enqueued even with no mail configured, and lands in `blocked` | FR-015 |
| No surface returns `body_encrypted` | FR-016 |
| The server starts with every setting unset | FR-028, SC-009 |
| A subsystem GitHub application resolves whole; a partial one falls back to global, said out loud | FR-019, FR-021, US5.4 |
| `SYNC_GITHUB_APP_*` alone continues to configure lore sync with no other change | FR-024 |
| A share beyond a world is refused while the notice contact is unset, naming what is missing | FR-026, spec 039 FR-053 |

## Entity relationships

```text
SettingDeclaration (code) ──1:0..1──> instance_settings row
                          ──1:0..1──> manifest.json key           (the six existing ones)
                          ──1:N────> instance_setting_changes
                          ──N:1────> Capability ────> ReadinessReport (derived)

User ──1:N──> instance_setting_changes   (changed_by)
User ──1:N──> instance_settings          (created_by / updated_by)

Feature (035 / 037 / 039) ──enqueues──> mail_outbox ──sent by──> MailTransport
                                                     └──blocked by──> unresolved mail.* settings

github_app.<scope>.* ──resolves to──> RegisteredApp ──used by──> lore sync | feedback | …
```
