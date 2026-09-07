# Contract: GitHub App credentials at two scales

**Spec 040 owns collecting and editing these.** This contract owns only the
environment form, the resolution rule, and the record of where each value came
from — the three things this feature must be able to depend on. 040's FR-018 to
FR-024 and this document must agree; where they disagree, 040 is the one that
moves, because the operator's surface is 040's to design and the resolver is
this feature's to build.

---

## The variables

Three prefixes, five variables each, following the shape
`src/server/src/repo_host.rs` already established for `SYNC_GITHUB_APP_*` —
which itself follows spec 007's `OAUTH_<PROVIDER>_*`.

| Suffix | What it is |
|---|---|
| `_CLIENT_ID` | The application's **client ID**, which the assertion is issued by. Not the numeric application id: GitHub's own documentation says "Use of the client ID is recommended", and naming the variable for the recommended value means an operator copying what GitHub puts in front of them lands in the right place |
| `_SLUG` | The application's URL slug — the last segment of its `github.com/apps/…` address. Not interchangeable with the identifier |
| `_PRIVATE_KEY_FILE` | A path to the PEM. **Highest precedence** — the form Docker and systemd secrets use, and an operator who has gone to that trouble must not be silently overridden by a stale inline value |
| `_PRIVATE_KEY_BASE64` | The PEM, base64-encoded. A **declared** encoding, so a value here that is not base64 is an error against *this* variable rather than a confusing complaint about another one |
| `_PRIVATE_KEY` | The PEM itself, or base64 of one, or a PEM with literal `\n` escapes |

Prefixes: **`GLOBAL_GITHUB_APP_`**, **`SYNC_GITHUB_APP_`**,
**`FEEDBACK_GITHUB_APP_`**.

`SYNC_GITHUB_APP_*` keeps its exact current meaning. FR-024 and 040's FR-024
are satisfied because nothing about it changed, not because somebody was
careful to preserve it.

---

## Resolution

```rust
pub enum AppScope { Global, Sync, Feedback }
pub enum Field { ClientId, Slug, PrivateKey }

pub struct ScopedApp {
    pub app: GitHubApp,
    pub scope: AppScope,
    /// Which prefix each field was actually taken from. FR-029.
    pub sources: Vec<(Field, &'static str)>,
}

pub fn registration_for(scope: AppScope)
    -> Result<ScopedApp, Vec<RegistrationProblem>>;
```

1. **Per field, not per set.** For each of the three fields, the subsystem's own
   prefix wins if it sets it; otherwise the global prefix supplies it (FR-025).
2. **The private key's three forms are resolved within a prefix first**, in the
   existing precedence (`_FILE`, then `_BASE64`, then `_PRIVATE_KEY`), before
   the prefix falls back. An operator who set `FEEDBACK_..._PRIVATE_KEY_FILE`
   gets that file, not the global inline value.
3. **Every field records its prefix** in `sources`. FR-029 and 040's FR-021 are
   this vector, rendered — which is what makes "a half-configured application is
   never *silently* completed" a property rather than a promise.
4. **Every problem is returned, not the first**, as `registration_from_env`
   already does, so an operator fixes their configuration in one pass instead
   of discovering the next missing variable after each restart.
5. **The key is parsed here, not at first use** (FR-028). `GitHubApp::new`
   performs the RSA parse and a failure is
   `RegistrationProblem::UnreadablePrivateKey`, reported alongside the missing
   variables rather than surfacing during somebody's submission.

### Why per-field and not per-set

A feedback application with a slug and no key would, under per-set resolution,
silently fall back to the global application's identity **and** the global
application's key — a third application that the operator never configured and
cannot see. Per-field resolution with a recorded source makes that state
visible instead of impossible-to-notice, which is what FR-029 asks for.

---

## The check that must not travel

`registration_from_env()` today pushes
`RegistrationProblem::GitBinaryMissing` when
`crate::lore_sync::git::git_is_available()` is false. That is correct for lore
sync, which drives `git` directly, and **wrong for every other scope**:
feedback creates an issue over HTTPS and needs no git at all.

The git probe therefore stays with the sync scope and does not enter the shared
resolver. A container without `git` must report a healthy feedback destination,
because it has one.

---

## Diagnostics

`RegistrationProblem` already satisfies FR-027 — every variant names a variable
and what it is for, and **none carries a key, a fragment of one, or its
length**. It gains the prefix so the message says which one it is complaining
about:

```text
FEEDBACK_GITHUB_APP_SLUG is not set, and GLOBAL_GITHUB_APP_SLUG is not set
either. It is the application's URL slug — the last segment of its
github.com/apps/… address — and is different from its client ID.
```

Rules, all testable:

1. No diagnostic prints a value, a fragment of one, or its length — including
   "the key is 1,704 characters", which is a fingerprint.
2. A diagnostic naming a fallback names **both** variables, so an operator who
   set the global one knows why it did not apply.
3. The resolution report renders `sources` verbatim: field, prefix, one line
   each. It is a table an operator reads, not a sentence somebody writes.

---

## What this feature does **not** own

- **The ADR.** Spec 040's plan reserves it ("credential resolution — scope
  outside, source inside"), and a second ADR for one decision is how two
  documents begin disagreeing. This feature builds the resolver because
  FR-023 to FR-030 need it and 040 is not being built yet; whichever lands
  first writes the ADR under 040's number.
- **The setup screen, the admin form, and editing.** Spec 040, User Story 5.
- **The blast-radius explanation** — "this application will act for lore sync
  and feedback" — is 040's FR-020, rendered wherever global credentials are
  set. FR-026 states the requirement; 040 provides the surface. What this
  feature owes 040 is a list of scopes it can render, which is `AppScope`
  itself: adding a fourth subsystem adds a variant, and the surface that
  enumerates it cannot go stale.
- **Environment-wins-over-stored** (040's FR-010). This resolver reads the
  environment; when 040 adds stored values it wraps this function rather than
  replacing it.
