# Credential Scope Before Source, And An Application Resolves Whole

- **Date**: 2026-09-07
- **Status**: Accepted
- **Spec**: `specs/040-instance-setup/` (FR-019 – FR-024, US5),
  `contracts/github-applications.md`, `research.md` § R10
- **Follows**: ADR-088, which made "the environment beats the instance's store
  beats the declared default" one rule for every declared setting
- **Amends**: spec 037's FR-029, which assumed field-level merging
- **Governs**: which GitHub application a subsystem acts as, and what a
  half-written one does

## The decision

Two rules, and the second is the one that changed code that already existed.

**1. Scope is the outer axis; source is the inner one.**

```text
For subsystem S, take the first COMPLETE application:
  1. S,      environment    <S>_GITHUB_APP_*      e.g. SYNC_GITHUB_APP_*
  2. S,      instance       github_app.<s>.*
  3. global, environment    GLOBAL_GITHUB_APP_*
  4. global, instance       github_app.global.*
```

**2. An application resolves whole, or is stepped over whole.** A subsystem
application with a client ID and no private key does not borrow the global
one's key. It is reported incomplete, naming the settings it is missing, and
the subsystem falls through to the global application *entire*, saying so.

## Why scope goes outside

FR-010 says the environment beats the instance's store. FR-019 says the
subsystem beats the global. An operator who sets `GLOBAL_GITHUB_APP_*` in their
compose file *and* configures a feedback application in the administration
screens has satisfied the antecedent of both, and the two rules point opposite
ways. The spec does not decide it, and it has to be decided.

Specific intent wins, and the source rule applies within it.

The alternative — source outside — lets a broad `GLOBAL_GITHUB_APP_*` silently
override the subsystem application somebody just deliberately configured. That
is the exact surprise spec 040 exists to eliminate: a value an operator set, on
a screen, that does nothing, for a reason nothing on the screen states. The
cost of the choice we made is the mirror image and it is much smaller: an
operator who wants the global application to win for a subsystem clears that
subsystem's own settings, which is a thing they can see and do.

This is one rule bending, not two rules coexisting. ADR-088's precedence is
intact and still governs every *setting*; this governs which *set of settings*
is consulted, and only for credentials that exist at two scales.

## Why an application resolves whole

This is the rule that had to change working code, so the reasoning matters.

`repo_host/scoped.rs`, written hours earlier for spec 037's feedback delivery,
resolved **per field**, and its own module comment argued for it: resolving
whole sets "would satisfy FR-025's sentence and break its point". That is a
good argument against the *wrong* alternative. It is arguing that a feedback
application with a slug and no key must not fall back to the global identity
alone — and it is right about that. But its fix was to complete the subsystem's
half-application with the global's private key, and a client ID from one
registration signed by a key from another is not an application at all. It is
an authentication failure that reads like a bad key: the host returns 401, the
operator inspects the key they configured, the key is fine, and nothing
anywhere says the two halves came from different registrations.

Spec 040's US5 acceptance scenario 4 asks for the opposite in as many words —
"rather than a half-configured application being silently completed" — and
FR-021 asks that where a resolved application draws on more than one source the
operator can see which value came from where. Under whole resolution FR-021 is
satisfied at the granularity that actually exists: the operator is shown which
*application*, from which scope, with each field's source and fixing variable,
and an incomplete one is reported by name.

Spec 037's FR-029 ("when resolution draws on both global and specific values")
is the sentence that assumed merging. It cannot hold alongside US5.4; its
pointer now names this contract.

## What this changed in the tree

- `src/server/src/github_apps.rs` owns the vocabulary — `AppScope`, `Field`,
  `setting_key`, `CredentialProblem`, `ScopedApp`, `registration_for`. One
  vocabulary for the product, not one per subsystem.
- `src/server/src/repo_host/scoped.rs` keeps the effects half — the calls
  delivery makes against the host — and re-exports the rest, so every existing
  caller keeps its path and `feedback::deliver` is untouched.
- `repo_host::registration_from_env()` is **unchanged** and is step 1 for lore
  synchronisation. A deployment with only `SYNC_GITHUB_APP_*` set resolves
  byte-identically to what it resolved before (FR-024), asserted against that
  function rather than against a constant, because a constant would agree with
  whatever the new code did.
- Nine declarations in `settings::registry` — three scopes by three fields —
  already existed and were not duplicated. They are the only place a variable
  name for these credentials is written down.

## Consequences

- A subsystem's settings are load-bearing even when incomplete: they suppress
  nothing, but they *are* reported, and the operator surface says the subsystem
  is using the global application and why.
- Adding a subsystem is one `AppScope` variant, three declarations and one
  arm of `setting_key`. `actsFor` grows on its own, because it is computed from
  the resolution delivery gets rather than from a list.
- The `..._PRIVATE_KEY_FILE` form stays an environment form. The path is read
  at resolution time and the key is never written to the database, so a Docker
  secret stays a Docker secret. The instance store holds the inline forms only.
  Making `_FILE` a storable field would need a tenth declaration and a tenth
  variable name, and two registry invariants — every declaration has an
  environment variable, and no two declarations read the same one — say plainly
  that it would be a new variable rather than a new home for an old one.
