# One Precedence Rule For Every Setting, And Two Mechanisms Implementing It

- **Date**: 2026-09-07
- **Status**: Accepted
- **Spec**: `specs/040-instance-setup/` (FR-007, FR-009 – FR-012, FR-024,
  FR-028), `contracts/settings.md`, `research.md` § R5, § R6, § D3
- **Extends**: ADR-041, which decided "the environment wins" for OAuth
  providers and implemented it for OAuth providers only
- **Governs**: where any configured value in this product comes from, and how
  an operator finds out

## The decision

**For every declared setting: the environment beats the instance's own store
beats the declared default.** One rule, one vocabulary — `environment`,
`instance`, `default` — resolved **per request**, never cached beyond it.

A setting exists because a declaration exists in `settings::registry`. A row in
`instance_settings` whose key nothing declares does not resolve. It is not
deleted and it is not an error; it is inert, and readiness reports it as
unrecognised.

And the second half, which is the part that would have been easy to leave
implicit: **ADR-041's OAuth materialisation is kept, not rewritten.** There are
now two mechanisms implementing one rule, deliberately.

## Why one rule had to be written down at all

Before this feature an instance's configuration was in five places with three
read cadences and two precedence mechanisms:

| Configuration | Where | Read |
|---|---|---|
| Realm manifest — six keys | `<data_path>/config/manifest.json` | per request, through `admin::read_system_manifest` |
| Instance access policy | `instance_access_settings`, singleton | per request |
| OAuth providers | `oauth_providers` rows with `config_source` | **materialised at startup** from `OAUTH_*` |
| Lore sync's application | the environment, nowhere else | per use |
| Core server config | `THUNDERFORGE_SECRET`, `_DATA_PATH`, `_SECURE_COOKIES` | **once, at startup** |

Nowhere could an operator see the whole of it, and "the environment wins" was
true of exactly one of the five. FR-010 says *every* setting, and research.md
§ D3 is honest that this sentence is larger than it sounds: it is what forced
environment-variable names onto the six manifest keys, which had never had any.

The declaration list is what makes FR-012 ("a new setting inherits this rule")
enforceable rather than aspirational. `SettingDeclaration` has no default for
`env_var`, `secret` or `requirement`, so a setting cannot be added that quietly
opts out of a precedence rule, a redaction rule, or a statement of what it
enables. A registry test asserts every key is unique, every non-`Prose`
declaration has an environment variable, and no two declarations read the same
one — which is what stops the list becoming a place two settings share a
variable and disagree about what it means.

## Why a resolver rather than a materialisation

ADR-041 implemented precedence by *writing*: `materialize_env_oauth_providers`
parses `OAUTH_*` at startup and writes rows stamped `config_source = "env"`, and
the admin update path refuses to overwrite one.

That cannot satisfy FR-007, which requires a change to take effect without a
redeploy or a restart. And once the row exists, "the environment set this" and
"an administrator typed the same value" are the same row with a column beside
it that has to be trusted to still be true.

So resolution reads. `resolve_all` loads the row-set once, the manifest once and
the access policy once, and answers for every declaration from that snapshot.
Callers hold the returned `Settings` for the life of a request — that *is* the
memoisation, and it is the only one.

**There is deliberately no process-wide cache**, and the resolver's own module
docs give the reason in a sentence worth quoting into this record: *a cached
setting is a setting that keeps its old value after somebody changes it.* That
is precisely the failure FR-007 describes. It is also why the mail subsystem
(ADR-089) holds a seam that builds a transport from the settings as they resolve
now, rather than an `Arc<dyn MailTransport>` built once at startup and cloned
into every request with a stale SMTP host inside it.

## Why the OAuth materialisation is kept

Rewriting it would be a data migration of live `oauth_providers` rows on every
existing deployment, in service of a refactor. FR-024 exists to forbid exactly
that: an existing deployment must not be reconfigured to keep working.

What is kept is the *mechanism*, not a second answer. `config_source` is the
same three-word vocabulary expressed in a different place, and an SDL-level test
asserts both surfaces report a source from the same enum, so they cannot drift
on the words even though they differ on the machinery.

The cost is recorded rather than hidden: **two mechanisms, one rule.** A
contributor reading `admin.rs` and a contributor reading `settings/resolver.rs`
are looking at two implementations of the same sentence. The mitigation is that
the observable behaviour is identical — the environment wins, the source is
visible, an environment-fixed field is not offered as editable — and that the
divergence is one paragraph in `resolver.rs`'s docs away from anyone who finds
either half.

Two alternatives were rejected explicitly:

- *Materialise everything at startup, like OAuth.* Breaks FR-007 for every
  setting, and makes an operator's typed value indistinguishable from an
  environment variable the moment the row is written.
- *Resolve everything and drop the materialisation.* The migration FR-024
  forbids.

## Three backing stores, one interface — and what changed from the plan

`research.md` § R6 proposed three backings: `Row`, `ManifestFile` and `Derived`.
What shipped is `Row`, `ManifestFile` and **`AccessPolicy`**, and `Derived` is
not a backing at all.

- `Backing::Row` — `instance_settings`. Everything this feature adds.
- `Backing::ManifestFile(key)` — the six `admin::editable_manifest_keys()`,
  still read from and written to `manifest.json` by the path that has always
  owned them. **Presented, not migrated** (ADR-091).
- `Backing::AccessPolicy` — spec 035's `instance_access_settings` singleton,
  read here so the policy appears in one list and obeys FR-009 and FR-010,
  written only through `setInstanceAccessPolicy`. It was not in the research
  because § R6 counted storage media and missed that the policy is a *setting*
  an operator looks for in the same list.
- Derived values became `crate::readiness`, a module rather than a backing. A
  value that is computed from the others is not a fourth place a value is
  stored, and modelling it as one would have put "what can this instance not
  do" behind an interface whose whole purpose is answering "where did this come
  from".

`Resolved::editable()` therefore has **two** reasons to refuse a field, and they
are different sentences: the environment has fixed the value (FR-009, and the
surface says which variable), or the value is somebody else's to write — the
access policy keeps spec 035's own mutation and its own `instance_access_events`
audit trail, and a second write path would produce policy changes that trail
never saw.

## Two resolution rules that were not in the research and are load-bearing

**A manifest value identical to the shipped seed resolves as `Default`, not as
`Instance`.** `config/realm-defaults.json` is compiled in, and the manifest file
on disk starts as a copy of it. Without this rule an instance that had never
been configured would report `support_email` as an instance answer, and
readiness could not tell a support address somebody chose from one that came
with the box. The comparison is against `admin::shipped_manifest_defaults()`,
and it is the reason FR-004's placeholder rules have anything to bite on.

**An unreadable secret resolves as unset, and says so.** A ciphertext that will
not decrypt sets `undecryptable: true` and `value: None`. The two alternatives
are an empty string that reads as configured and a panic on a settings read.
`crypto.rs` names the cause plainly — rotating `THUNDERFORGE_SECRET` makes every
stored ciphertext unreadable — so this is a state a real operator reaches, and
readiness names the key rather than the server falling over. A resolver test
asserts precedence for **every** declared setting rather than a sample, and
asserts this case by name.

## Consequences

- Adding a setting is a declaration, and nothing else. It cannot omit its
  environment variable, its redaction rule or its requirement, and it is
  reported in readiness the day it is added.
- `RequiredFor(capability)` is never a reason the server fails to start
  (FR-028, SC-009). An unset required setting is a readiness gap and a refusal
  at the point of use. An upgrade that introduces one leaves the instance
  running and tells the operator what is now missing.
- `resolve(key)` for a single setting costs the same full load as `resolve_all`.
  That is stated in its own docs so a caller wanting several holds a `Settings`
  instead of calling it in a loop, and it is the honest price of refusing a
  cache.
- The environment is process-global and `cargo test` is threaded, so every test
  in this feature that touches a variable or a settings row serialises behind
  one lock (`settings::test_env`). Without it the suite passes alone and fails
  together, which is worse than failing.
- **One deliberate exception exists and is recorded elsewhere.**
  `admin_bootstrap.rs` reads `THUNDERFORGE_PUBLIC_URL` directly, outside the
  registry, because the setup link has to be right before anything is
  configured and before the database is necessarily readable. ADR-093 records
  it, including the condition under which it should become a declaration.

## What this does not solve

It does not unify *writing*. Three backings have three write paths — the
settings mutation, the manifest editor, and spec 035's policy mutation — and
this decision only guarantees that all three are read by one rule and reported
in one vocabulary. An operator sees one list; underneath, a manifest key is
still a file and a policy is still spec 035's table.

It also does not make the five original mechanisms into one. It makes them
*declared*. That is a smaller claim than "unified configuration", and it is the
one that could be made without migrating any operator's live values.
