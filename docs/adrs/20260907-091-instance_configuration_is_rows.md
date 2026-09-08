# Instance Configuration Is Rows — Not The Manifest File, And Emphatically Not `instance_identity`

- **Date**: 2026-09-07
- **Status**: Accepted
- **Spec**: `specs/040-instance-setup/` (FR-001, FR-008, FR-012, FR-023,
  FR-028), spec 039 FR-051, `data-model.md` § 1 – § 2, `research.md` § R1,
  § R2, § R6
- **Follows**: ADR-088, which decided how a value resolves once it has a home
- **Governs**: where a configured value is stored, and what is deliberately
  left where it already is

## The decision

**One table, `instance_settings`, one row per configured value, keyed by a
string that `settings::registry` declares.** Everything this feature collects
lands there. Alongside it, `instance_setting_changes` — append-only, one row per
write, holding the transition rather than always the value.

And two refusals, which are the substance:

- **Not the realm manifest file.** The six editable manifest keys stay in
  `manifest.json`, read and written by the path that has always owned them.
  They are **presented** in the settings surface, not migrated into it.
- **Not `instance_identity`.** Not extended, not joined to, and the operator
  record does not sit beside it.

## Why rows, and why a key rather than a column

`data-model.md` § 1 is a two-column table plus provenance: `key`, `value`,
`created_by`/`created_at`, `updated_by`/`updated_at`. There is deliberately no
`is_secret` column, no `type` column and no `source` column. The declaration
decides all three, and a second answer stored beside the value could disagree
with it — `models.rs` says so at the struct.

The alternative was a **wide singleton table, one column per setting**. It was
rejected for one concrete reason: every new setting becomes a migration, and
FR-028 — an upgrade that introduces a required setting must not stop an
instance starting — becomes a schema question instead of a declaration question.
With rows, a new required setting is a declaration and a readiness gap. With
columns it is a `NOT NULL` somebody has to get right against every existing
deployment.

The consequence rows buy, and the reason the declaration list in ADR-088 is
worth its weight: **a row whose key nothing declares does not resolve.** It is
never deleted — an operator who downgraded and upgraded again keeps their
values, which is the posture ADR-041 already took for provider rows — it is
simply inert, and readiness reports it as unrecognised.

## Why not the manifest file

The manifest is a JSON file at `<data_path>/config/manifest.json`, seeded from
`config/realm-defaults.json`, holding `realm_name`, `interface_pack_id`,
`asset_pack_id`, `support_email`, `welcome_message` and
`default_game_system_id`.

Three reasons it is not where new configuration goes:

1. **Spec 039's FR-051 forbids it in as many words.** The operator identity must
   be "stored by the instance, not compiled in and **not left in a file**".
2. **It sits under `data_path`.** That is gitignored, and in the container
   deployment this feature exists for it is whatever volume the operator did or
   did not mount. An instance's identity must not be able to vanish with a
   volume while its database survives.
3. **A file has no provenance.** FR-008 wants who changed what, from where, and
   what it was before. JSON keys do not carry `updated_by`.

## Why the six manifest keys are presented rather than migrated

This is the decision most likely to be re-litigated by somebody who finds the
`ManifestFile` backing and thinks it is half-finished work. It is not.

Migrating them would be **a data migration of an operator's live values, for no
requirement's sake.** The spec's own assumption is that "the realm manifest's
existing editable keys stay editable, and this feature gathers them into the
same surface rather than replacing them", and its Out of Scope entry is
"changing what any existing setting means". FR-024 forbids requiring an existing
deployment to be reconfigured.

So `Backing::ManifestFile(key)` reads the file the existing path reads and the
manifest editor keeps writing it. What the keys *gain* is declarations: an
environment-variable name they never had (`THUNDERFORGE_REALM_NAME`,
`THUNDERFORGE_SUPPORT_EMAIL`, …), a validator, a stated requirement, and a
`source` in one vocabulary. FR-009 and FR-011 are the existing `editable` flag
grown a reason — a key the environment has taken over now reads *fixed by
`THUNDERFORGE_SUPPORT_EMAIL`* rather than being greyed out for no stated cause.

One rule was needed to make this honest and it was not in the research: **a
manifest value identical to the shipped seed resolves as `Default`, not as
`Instance`.** The file on disk begins as a copy of `realm-defaults.json`, so
without the comparison against `admin::shipped_manifest_defaults()` every
unconfigured instance would report a support address it had chosen. That is
exactly the case FR-004's placeholder rules exist to catch — the shipped
`stewards@thunderforge.local` is refused by name — and the comparison is what
gives readiness anything to notice.

An operator sees one list. Underneath, a manifest key is still a manifest key,
and nothing about the seed file or the existing editor changed.

## Why emphatically not `instance_identity`

`src/server/src/instance_identity.rs` is a singleton holding one v4 UUID,
generated on first read. Its own module docs say what it is for and what it
costs: it exists so a lore-sync binding can say "this world, on *this*
deployment" (spec 034 FR-036g), and **it is published into an issue on a
repository that may be public** — which is why it is v4 rather than v7, so it
does not leak when the instance was first started. That reasoning is ADR-049's.

Three reasons not to put an operator's name, postal address and notice email
beside it. Any one of them is sufficient.

1. **It is published.** A table whose contents are written into a public
   repository issue is the last place in this product to store contact details.
   The blast radius of one careless `SELECT *` in `lore_sync` would be an
   operator's home address on GitHub.
2. **It is machine identity, not human identity.** It answers *which
   deployment*, not *who runs it*, and the two have different lifetimes.
   Restoring a backup onto a second machine is **supposed** to carry the
   instance id — the module says so — and is emphatically not supposed to carry
   an assertion that the same person operates the copy.
3. **It has no provenance and must not gain any.** Principle III requires new
   tables to carry `created_by`/`updated_by`. `instance_identity` deliberately
   has neither, because there is no actor: it generates itself. FR-008 requires
   the operator identity carry exactly that provenance. Adding nullable
   provenance columns to a self-generating singleton to serve a different
   concern is how a table stops meaning one thing.

A bespoke `operator_record` table was also rejected. The operator's name and
contact are settings — they resolve by ADR-088's rule, they are required at
setup, they are validated by the same refusals, and they appear in the same
list. A table for them would be a second mechanism for six values, and the whole
point of this feature was that there were already five.

## The change record, and the one place FR-008 and FR-023 conflict

`instance_setting_changes` is append-only: key, previous, new, who, from which
surface (`setup` / `admin` / `system`, held as an enum in code and a `CHECK` in
the migration so a typo fails twice), and whether the row was redacted.

FR-008 wants "what it was before". FR-023 forbids ever printing a credential.
For a secret those are in direct conflict, and the resolution is that the record
captures the **transition**: the literal previous address for a notice contact,
and the words `set` / `not set` for an SMTP password. An operator investigating
a stale notice address needs the old address. Nobody needs the old password, and
storing it doubles the number of places it can leak from.

`redacted` is **stored rather than recomputed at read time**, so a row stays
truthful if a declaration's `secret` flag is ever changed later. A row that says
it was redacted was redacted when it was written.

There is deliberately no delete that leaves no trace. Clearing a setting writes
`value: None`, the setting falls back to its default, and the record shows that
transition like any other.

## Consequences

- Secrets are encrypted through the existing `crypto.rs` — one implementation,
  for the reason that module's own docs give. A ciphertext that will not decrypt
  resolves as unset with `undecryptable` set, never as an empty string and never
  as a panic; rotating `THUNDERFORGE_SECRET` is a real thing operators do.
- Three backings mean three write paths. This decision does not unify writing;
  it decides where *new* values live and guarantees all three are read by one
  rule (ADR-088).
- The reserved key prefix in `registry` keeps the instance's own bookkeeping
  rows out of the unrecognised report, so an internal row is not mistaken for a
  setting somebody left behind.
- The migration is reversible — `up` → `down` → `up` leaves the schema clean,
  asserted by a test — because FR-028's upgrade promise starts with a migration
  an operator can back out of.

## What this does not solve

It does not give the manifest keys provenance. A change to `realm_name` through
the manifest editor still writes a file, and `instance_setting_changes` records
what went through the settings mutation. Closing that gap means migrating the
manifest, which is the thing this decision declines to do; if it is ever worth
doing, it is worth doing as its own change with its own data migration, not as a
side effect of a settings surface.
