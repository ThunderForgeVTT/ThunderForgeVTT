# Phase 1 Data Model: The Sharing Attestation

Five new tables, one new column, and two things that deliberately have neither.
The line between what is stored and what is derived is the design, and it
follows a rule the moderation module set first: **status is derived from an
append-only log; only the consequence is a row.**

Nothing here uses a foreign key to `users`. That is not an oversight — see
§ "Why nothing points at a user" at the end.

---

## 1. `terms_versions` — the archive, written before it is needed

```text
terms_versions
  version_id      text PRIMARY KEY        "sharing-terms@4f2a9c1e77b03d58"
  document_slug   text NOT NULL           "sharing-terms" | "operator-responsibilities"
  body            text NOT NULL           the document, comment stripped, trimmed
  first_seen_at   timestamptz NOT NULL DEFAULT now()
```

- `version_id` is `<slug>@<first 16 hex of sha256(normalised body)>`. Normalising
  strips the leading HTML comment and trims, matching what
  `apps/web/src/legal/legalDocuments.ts`'s `sectionsOf` already does before
  rendering — so a note added to the file's explanatory comment does not mint a
  version, and a changed sentence does.
- Written by `ensure_terms_versions_recorded(state)` at **startup**, from
  `src/app/src/main.rs`, beside `ensure_admin_bootstrap_code` and
  `ensure_instance_identity`. Idempotent: insert-if-absent by primary key.
- **Nothing is ever deleted or updated here.** FR-016 is a property of that
  sentence plus the startup ordering: a version an attestation could name was
  archived before the server that would accept the attestation was serving.
- Index on `(document_slug, first_seen_at)` — the only listing anyone needs is
  "every version of this document, oldest first".

---

## 2. `attestations` — one person, one moment, one version, one act

Serves both the sharing attestation (US1–US4) and the operator acknowledgement
(US8). FR-043 says the operator record is made "on the same terms as a sharing
attestation"; making it the same table is the only way that stays true.

```text
attestations
  id                      uuid PRIMARY KEY
  purpose                 text NOT NULL         'share' | 'operator'
  subject_user_id         uuid NOT NULL         no FK, deliberately
  subject_username        text                  NULL after redaction (see below)
  terms_version_id        text NOT NULL         -> terms_versions.version_id
  publishable_kind        text                  'collection' | 'actor' | 'item' | 'ability'
                                                NULL when purpose = 'operator'
  publishable_id          uuid                  the thing published; NULL for 'operator'
  share_id                uuid                  the share row minted in the same txn
  world_id                uuid                  the world it came from; NULL for 'operator'
  attested_at             timestamptz NOT NULL DEFAULT now()
```

- **Written inside the same transaction that mints the share code.** There is no
  interleaving in which a share link exists without its attestation, and none in
  which an attestation exists for a share that failed.
- `share_id` is a plain uuid, no FK, so revoking or deleting the share leaves the
  record standing (FR-007). It is a pointer, not a dependency.
- `terms_version_id` **is** a foreign key to `terms_versions`, and it is the one
  FK in this feature. It has to be: an attestation naming a version whose text
  is gone is exactly the failure FR-008 forbids, and the archive is
  append-only, so the constraint can never block anything.
- Indexes: `(publishable_kind, publishable_id)` — the lookup a person handling a
  notice makes (FR-009, SC-003); `(subject_user_id, attested_at DESC)` — the
  person's own history; `(terms_version_id)` — "who agreed to this version".

### Redaction, and what survives an account being deleted

FR-010 and FR-037 together: the record survives, in the minimal lawful form,
and the form is stated rather than left to the implementation.

```text
on account deletion:
  subject_username  -> NULL          (the only field that names a human)
  subject_user_id   -> kept          (an opaque id that now resolves to nothing)
  everything else   -> unchanged
```

What remains is: somebody, identified only by an id no longer joinable to a
person, agreed to this exact text at this exact time in order to publish this
exact thing. That is what a notice arriving in eighteen months needs and it
names nobody.

---

## 3. `content_adoptions` — the link `copy.rs` deliberately refused to keep

**Gated on ADR-094.** Until that determination is accepted, this table does not
exist and FR-022/FR-023 are not buildable. See research.md § R6 and plan.md's
guardrail section.

```text
content_adoptions
  id                    uuid PRIMARY KEY
  source_entity_type    text NOT NULL      'world_actor' | 'world_item' | ...
  source_entity_id      uuid NOT NULL
  copy_entity_type      text NOT NULL
  copy_entity_id        uuid NOT NULL
  destination_world_id  uuid NOT NULL
  adopted_by            uuid NOT NULL      no FK
  adopted_at            timestamptz NOT NULL DEFAULT now()
```

- Written by `collections/copy.rs` and by the three singleton copy impls, inside
  the transaction that creates the copy. One row per copied entity, not per
  collection.
- Entity types are the moderation vocabulary, obtained through
  `collections::moderation_entity_type`. A member type that maps to `None` —
  today `"scene"` — records no adoption, which is the same documented gap
  spec 038's FR-026c names, arriving here unchanged. It is recorded in the
  migration comment so the next reader does not think it was forgotten.
- Index on `(source_entity_type, source_entity_id)` and on
  `(copy_entity_type, copy_entity_id)`. The second is what makes a copy-of-a-copy
  reachable: the walk is transitive.

### The rule this table must never break

**No user-facing surface may read it.** No query, no field, no subscription, no
route, no admin listing, in either direction. The only reader is
`moderation::reach`, entered from a takedown or counter-notice that already
named an entity id. ADR-069's determination rests on non-enumerability;
ADR-094's determination rests on preserving that, and a test asserting the SDL
exposes no field of this shape is the mechanism that keeps it true.

---

## 4. `content_moderation_actions` — one new column

The existing table is unchanged in every other respect. Its append-only
event-log shape, its lack of foreign keys, its `case_id` grouping and its seven
action types all stay exactly as they are.

| Column | Change | Why |
|---|---|---|
| `parent_case_id` | **new**, nullable uuid | A case opened against an adopted copy names the case it was fanned out from. Nullable because every case that exists today has no parent, and every case opened by a notice is still a root. |

One new action type joins the seven: `content_disabled_as_copy`.

- `is_disabled_status` treats it as disabled, so `effective_status` and
  `filter_visible` need no change at all.
- `repeat_infringer_flags_impl` never sees it, because a child case is written
  with `account_id = NULL` and that function filters on a non-null account. **An
  adopter accrues no strike for content somebody else uploaded** — FR-023b as a
  property of the row rather than a rule somebody remembers.

**Why child cases and not more rows in the parent's case**: `effective_status`
takes the latest event per *entity* and `repeat_infringer_flags_impl` takes the
latest event per *case*. Putting copy rows in the source's case would make the
second function's answer depend on which copy was touched last, corrupting the
counting FR-027 requires be reused unchanged. Separate cases leave both
functions untouched.

---

## 5. `account_terminations` — the only stored piece of standing

```text
account_terminations
  id                     uuid PRIMARY KEY
  account_id             uuid NOT NULL           no FK
  opened_at              timestamptz NOT NULL DEFAULT now()
  deletion_due_at        timestamptz NOT NULL    opened_at + window
  strike_count_at_open   integer NOT NULL
  requires_human         boolean NOT NULL        snapshot of the setting at open
  appeal_state           text NOT NULL DEFAULT 'none'   'none'|'open'|'upheld'|'rejected'
  appeal_filed_at        timestamptz
  appeal_resolved_at     timestamptz
  appeal_resolved_by     uuid
  closed_at              timestamptz
  closed_reason          text     'appeal_upheld'|'strikes_aged_out'|'deleted'|'admin_reversed'
```

- **At most one open termination per account** — a partial unique index on
  `account_id WHERE closed_at IS NULL`. Two windows for one account is the bug
  that produces two deletion dates.
- `requires_human` is snapshotted at open rather than read at execution, so an
  operator flipping the setting does not retroactively change the terms somebody
  was told about at the start of their window (FR-030, FR-036).
- `deletion_due_at` is computed once at open, the same way
  `restoration_due_at` is computed once when a counter-notice is forwarded —
  changing `MODERATION_TERMINATION_WINDOW_DAYS` later does not move a window
  already running.

### What is *not* stored

`strikes`, `may_publish` and `is_disabled` are **derived on every read**:

```text
strike_count(account)  = moderation::strike_count(conn, account)   -- extracted from
                                                     repeat_infringer_flags_impl
standing(account)      = { strikes,
                           may_publish: strikes < SUSPEND_PUBLISHING_AT
                                        && no open termination,
                           disabled:    an open termination exists,
                           termination: the open row, if any }
```

This is what makes FR-035 free. A strike ageing past the 365-day lookback
changes the count because the calendar moved; nothing has to notice. The sweep
then closes the termination with `closed_reason = 'strikes_aged_out'`, and the
account is usable again without anybody asking.

### The ladder

| Strikes | Consequence | Setting |
|---|---|---|
| 1 | Warned. A notice row; publishing unaffected. | `MODERATION_STRIKE_WARN_AT` = 1 |
| 2 | Publishing suspended. Play, edit and read are untouched (FR-019). | `MODERATION_STRIKE_SUSPEND_PUBLISHING_AT` = 2 |
| 3 | Account disabled; window opens. | `MODERATION_REPEAT_INFRINGER_THRESHOLD` = 3, **existing** |

All four values are environment variables with a parse-or-default fallback,
matching the three that exist in `moderation/mod.rs` exactly, including the
behaviour that an unparseable value falls back silently.

### State transitions

```text
                     ┌──────────────────────────────────────────────┐
                     │                                              │
 strikes < 2 ──> may publish                                        │
 strikes = 2 ──> publishing suspended ──(a strike ages out)─────────┘
 strikes = 3 ──> termination opened, account disabled
                   │
                   ├──(appeal filed)──> appeal_state = 'open'
                   │                      │
                   │                      ├──(upheld)──> closed: appeal_upheld
                   │                      │              strike removed, account restored
                   │                      └──(rejected)─> appeal_state = 'rejected';
                   │                                     deletion resumes from due date
                   │
                   ├──(a strike ages out)──> closed: strikes_aged_out, restored
                   ├──(admin reverses)──────> closed: admin_reversed, restored
                   │
                   └──(due, no open appeal, !requires_human)──> closed: deleted
```

**The appeal pauses, it does not extend** (FR-034). An appeal open at
`deletion_due_at` blocks execution until it resolves; a rejected appeal resumes
from the date that was already past, so deletion is never the outcome of the
instance being slow, and filing on day 29 does not buy a second window.

**The last administrator** (FR-039): if opening a termination would disable the
only account with `is_admin = true`, the row is written with
`requires_human = true` regardless of the setting and the account is **not**
disabled. An administrator is told. If that administrator is the only one, the
instance says so and does nothing, which is the honest outcome — a product that
locks itself out to enforce a rule has enforced nothing.

---

## 6. `account_notices` — because there is no other way to tell anybody

There is no notifications table, no mailer and no SMTP anywhere in this
codebase; spec 040 owns delivery and has no plan yet. This is the durable half.

```text
account_notices
  id            uuid PRIMARY KEY
  account_id    uuid NOT NULL       no FK
  kind          text NOT NULL       'strike_recorded' | 'publishing_suspended'
                                    | 'account_disabled' | 'appeal_resolved'
                                    | 'share_taken_down' | 'adopted_copy_disabled'
                                    | 'adopted_copy_restored'
  subject_ref   jsonb               what it was about: case id, entity, world
  payload       jsonb               the values the rendered text needs
  created_at    timestamptz NOT NULL DEFAULT now()
  read_at       timestamptz
```

- Rows are never deleted on read. `read_at` is for the badge, not for cleanup.
- The **text is not stored** — `kind` plus `payload` is rendered by the client,
  so a wording fix does not require rewriting history. This is the opposite
  choice from `attestations`, and deliberately: an attestation's value is that
  the exact words are fixed; a notice's value is that it was produced.
- `adopted_copy_disabled` goes to the **adopter** and, per FR-023b, must render
  as an explanation and not an accusation. That is a copy-writing requirement
  with a test, not a schema field.
- When spec 040 delivers mail, it delivers these rows. The table is the queue.

---

## 7. Entity relationships

```text
legal/*.md ──(include_str! + sha256)──> terms_versions ──1:N──> attestations
                                                                    │
Account ──1:N──> attestations (purpose='share')                     │
Account ──0:1──> attestations (purpose='operator')  [per instance]  │
attestation ──0:1──> share row (collection|actor|item|ability)  ────┘  no FK

SourceEntity ──1:N──> content_adoptions ──1:1──> CopyEntity
CopyEntity   ──1:N──> content_adoptions            (a copy of a copy)

content_moderation_actions ──self──> parent_case_id   (a case fanned to a copy)

Account ──0:1──> account_terminations (open)   ──> deletion_due_at
Account ──1:N──> account_notices
Account ──derived──> Standing { strikes, may_publish, disabled }
```

---

## 8. Validation rules, traced to requirements

| Rule | Requirement |
|---|---|
| Every `create*ShareLink` mutation takes an attestation, enforced by an SDL guard test | FR-001, FR-002 |
| An attestation is written per publish, never reused across publishes | FR-003 |
| A publish with no attestation is refused, whatever its origin | FR-011 |
| A publish naming an unarchived version id is refused | FR-012 |
| A refusal names neither a valid version nor the expected one | FR-013 |
| The attestation row is written by the server inside the share transaction | FR-014 |
| The version id changes when and only when the normalised words change | FR-015 |
| A `terms_versions` row is never updated or deleted | FR-016, FR-017 |
| An attestation resolves through `terms_version_id` to archived text, never to the constant | FR-008 |
| Revoking or deleting a share leaves its attestation | FR-007 |
| Deleting an account nulls `subject_username` and keeps the rest | FR-010, FR-037 |
| Publishing is refused at `SUSPEND_PUBLISHING_AT`, before the threshold | FR-018 |
| Suspension and disablement change no content permission | FR-019, FR-038 |
| Standing is derived; nothing writes `may_publish` | FR-020, FR-035 |
| A takedown opens a child case per adopted copy, `account_id = NULL` | FR-022, FR-023, FR-023b |
| A child case names one entity, never a world or a collection | FR-023a, FR-023c |
| A forwarded counter-notice fans `restoration_due_at` into every child case | FR-023d |
| A takedown writes a notice to the sharer and never touches their attestation | FR-024 |
| A strike is an upheld, unrestored case within the existing lookback | FR-027 |
| Every strike writes an `account_notices` row before the next one can land | FR-028 |
| `myStanding` answers for the caller at any time | FR-029 |
| At most one open termination per account, enforced by a partial unique index | FR-030 |
| A disabled account reaches exactly four surfaces, by allowlist | FR-031 |
| Downloading writes nothing to the termination row | FR-032 |
| An upheld appeal closes the termination and removes the strike | FR-033 |
| An open appeal blocks execution past the due date | FR-034 |
| A termination whose strikes fell below the threshold closes itself | FR-035 |
| Deletion is `delete_user_data_owned`, minus other people's worlds | FR-036, FR-038 |
| The last administrator is never disabled automatically | FR-039 |
| Four ladder values are operator-configurable, as the three existing ones are | FR-040 |
| Setup does not complete without an operator attestation | FR-041, FR-043 |
| The operator statement is versioned in `terms_versions` like any other | FR-044 |
| Publishing is refused when the instance has no notice contact | FR-053 |
| The words shown come from `legal/`, via the server | FR-026 |

---

## Why nothing points at a user

Every new table carries `account_id`, `subject_user_id` or `adopted_by` **with
no foreign key**. That is the precedent
`2026-08-23-150000-0000_create_content_moderation_actions/up.sql` set and wrote
down: FR-013 of spec 015 requires history to survive the deletion of the world,
the account and the entity, so the table has no foreign keys at all.

This feature needs the same property for stronger reasons. FR-010 requires an
attestation to survive the deletion of the person who made it; FR-037 requires
what survives to be stated. A cascade would delete the evidence at the moment
the account is deleted, which is precisely the moment the evidence is most
likely to matter. A `SET NULL` would lose the ability to say two attestations
were made by the same person, which is part of what a record is for.

The single exception is `attestations.terms_version_id -> terms_versions`, and
it is an exception because the constraint enforces FR-008 and can never fire:
the archive is append-only and is written at startup, before the attestation
that would name it can be accepted.

---

## One inherited property, recorded not fixed

`moderation::effective_status` materialises a `content_restored` row **on a read
path**, with no transaction and no advisory lock, so two concurrent reads past
`restoration_due_at` can both insert one. This predates the feature; the fan-out
in research.md § R6 increases the number of entities it can happen to, since a
source and its copies each restore on their own next read.

It is benign — a duplicate restoration restores, and `effective_status` reads
the latest row either way — and it is not this feature's to fix. Recorded here
so that the next person to see two `content_restored` rows in one case knows it
was known.
