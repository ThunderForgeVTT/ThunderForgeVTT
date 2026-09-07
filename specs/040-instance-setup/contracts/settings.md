# Contract: The declaration, the resolver, and the settings surface

One list, one rule, one place an administrator reads a value's source. This is
the contract every other contract in this directory depends on.

## The declaration (Rust, `src/server/src/settings/registry.rs`)

```rust
pub enum Backing { Row, ManifestFile(&'static str), AccessPolicy, Derived }

pub enum Requirement {
    Optional,
    RequiredAtSetup,
    /// Unset is a readiness gap and a refusal at the point of use.
    /// It is never a reason the server fails to start (FR-028).
    RequiredFor(Capability),
}

pub struct SettingDeclaration {
    pub key: &'static str,
    pub kind: Kind,
    pub backing: Backing,
    pub env_var: Option<&'static str>,
    pub requirement: Requirement,
    /// Decides encryption at rest AND every rendering, everywhere.
    pub secret: bool,
    pub default: Option<&'static str>,
    pub validators: &'static [Validator],
    pub capability: Option<Capability>,
    pub since: &'static str,
}

pub fn declarations() -> &'static [SettingDeclaration];
```

**Rules**

1. `declarations()` is the only way a setting exists. A row in
   `instance_settings` whose key is not declared does not resolve, is not
   deleted, and is reported in readiness as unrecognised.
2. `env_var` is `None` only for a `Prose` setting that has no sensible
   environment form. Every other declaration has one — the spec's edge case
   "an operator wants no configuration UI at all and to set everything by
   environment" is only true if it is literally true.
3. `secret: true` is load-bearing in four places at once: encryption at rest,
   the admin surface's rendering, the change record's redaction, and the
   readiness report. A new secret cannot forget any of them by being declared.
4. A test asserts every declaration with `secret: true` is unreachable from
   every read surface except as `set`/`not set`. It is the mechanical form of
   SC-007.

## The resolver (`src/server/src/settings/resolver.rs`)

```rust
pub enum Source { Environment, Instance, Default }

pub struct Resolved {
    pub key: &'static str,
    pub value: Option<String>,   // decrypted for internal use; never leaves the server for a secret
    pub source: Source,
    /// The variable name when `source == Environment` — FR-009's "and where it comes from".
    pub fixed_by: Option<&'static str>,
}

/// Every declared setting, resolved once. Memoised per request.
pub fn resolve_all(state: &AppState) -> Result<Settings, String>;
pub fn resolve(state: &AppState, key: &str) -> Result<Resolved, String>;
```

**Rules**

1. **Environment beats the instance's own store beats the declared default**,
   for every declared setting. This is spec 007 / ADR-041's rule, applied
   universally rather than per subsystem (FR-010).
2. Resolution happens **per request**, not at startup, so a change takes effect
   with no redeploy and no restart (FR-007). One row-set load per request,
   memoised — not a query per setting.
3. A `Secret` declaration whose stored ciphertext will not decrypt resolves as
   **unset**, with a readiness gap naming the key. Never an empty string, never
   a panic. `crypto.rs` documents that rotating `THUNDERFORGE_SECRET` produces
   exactly this.
4. `Backing::ManifestFile` reads through the existing `admin.rs` manifest path.
   `Backing::AccessPolicy` reads through `auth::instance_access::load_policy`.
   Neither store is migrated; both gain a source.
5. OAuth providers are **not** re-plumbed. ADR-041's startup materialisation
   stays; `oauth_providers.config_source` is the same answer in a different
   place, and the surface maps `"env" -> Source::Environment`,
   `"admin" -> Source::Instance` so there is one vocabulary. An SDL test
   asserts both surfaces use the same enum.

## The GraphQL surface

```graphql
type ResolvedSetting {
  key: String!
  "Absent for a secret. Present and literal for everything else."
  value: String
  "Set / not set, for a secret. Null for everything else."
  secretState: SecretState
  source: SettingSource!
  "The environment variable that fixed this value, when source is ENVIRONMENT."
  fixedBy: String
  "False when the environment has fixed it — do not offer a field (FR-009)."
  editable: Boolean!
  requirement: SettingRequirement!
  "What this setting enables, in a sentence a person can act on."
  capability: String
}

enum SettingSource { ENVIRONMENT, INSTANCE, DEFAULT }
enum SecretState { SET, NOT_SET }
enum SettingRequirement { OPTIONAL, REQUIRED_AT_SETUP, REQUIRED_FOR_CAPABILITY }

type SettingChange {
  key: String!
  previousValue: String
  newValue: String
  "True when the values above are 'set'/'not set' rather than the real ones."
  redacted: Boolean!
  changedBy: String
  changedAt: String!
  source: String!
}

extend type Query {
  "Every declared setting, resolved, with its source. Administrators only."
  instanceSettings: [ResolvedSetting!]!

  "The change history for one setting, newest first."
  instanceSettingChanges(key: String!, limit: Int): [SettingChange!]!
}

extend type Mutation {
  """
  Write one setting. Refused with a named reason when the environment has
  fixed it, when the value fails the declaration's validators, or when the
  caller is not an administrator.
  """
  updateInstanceSetting(key: String!, value: String): ResolvedSetting!
}
```

`GraphQLSystemManifest`'s existing `entries { key value editable }` gains
`source` and `fixedBy` on the same terms, so the manifest editor stops greying
a field out for an unexplained reason and starts saying "fixed by
`THUNDERFORGE_SUPPORT_EMAIL`".

## Rules

1. Every field here is behind `admin_user(ctx)?`, as the rest of the admin
   surface already is (Principle III).
2. `value` is **absent** for a secret. Not masked, not truncated, not
   length-hinted. There is no `sk-…3f9` anywhere in this feature (FR-023,
   SC-007).
3. `updateInstanceSetting` on an environment-fixed key is a **refusal naming
   the variable**, not a silent no-op. FR-009 exists because the OAuth surface
   learned this the hard way: ADR-041 records that an admin UI which renders
   env-sourced rows as editable produces edits that silently do not persist.
4. `updateInstanceSetting(key, null)` clears a stored value; the setting then
   resolves from the default, and the change record shows the transition.
5. Every successful write appends an `instance_setting_changes` row, redacted
   per the declaration (FR-008).
6. A validator refusal names **what is wrong and what would be right**, never
   the value that was submitted — a rejected password must not be echoed back
   into a browser's form history.

## What is deliberately absent

- **No bulk `updateInstanceSettings(input: [..])`.** One key, one change
  record, one refusal reason. A batch write with a partial failure is a
  question about atomicity nobody needs to answer here.
- **No `deleteInstanceSetting`.** Clearing is `updateInstanceSetting(key,
  null)`, which leaves a change record. A delete that leaves no trace is not
  available for values FR-008 requires be recorded.
- **No public read.** The one value that must be reachable without an account
  is the notice contact, and it is published on the legal page — see
  `legal-rendering.md`, which is a different surface with a different rule.
