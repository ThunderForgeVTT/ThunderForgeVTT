# ADR-041: Environment-Variable OAuth Provider Configuration, Layered on the Existing Admin-Panel Model

**Date:** 2026-08-21
**Status:** ACCEPTED
**Participants:** ThunderForgeVTT Team

---

## Problem Statement

ADR-017 established a DB-backed, admin-panel-only contract for configuring OAuth providers (`oauth_providers` table, `oauthProviders`/`updateOauthProvider` GraphQL surface), explicitly rejecting an environment-variable-only alternative at the time ("operators need a runtime admin surface for persisted provider records").

Spec 007 (`specs/007-configurable-auth-providers/`) asks for env-var-driven provider configuration (`OAUTH_<PROVIDER>_CLIENT_ID`, etc.) as an operator-facing, zero-admin-panel-touch path — not as a replacement for ADR-017's runtime admin surface, but as an additional, deploy-time configuration source that coexists with it. This also introduces two concepts the existing model doesn't have: multiple named instances of the same provider type (e.g. two Keycloak realms), and a precedence rule for when a provider is configured through both sources at once. Per Constitution Principle IV, this combination — a new configuration-source concept plus a trust/precedence rule affecting an authentication boundary — is architecturally significant enough to warrant its own ADR rather than proceeding as an unrecorded extension of ADR-017.

## Decision

Add environment variables as a second configuration source for `oauth_providers` rows, with **environment variables always taking precedence** over admin-panel-entered values for the same provider instance. This does not replace or narrow ADR-017's admin surface — admin-panel configuration remains fully available for any provider instance that has no matching environment variables, and the enable/disable toggle remains admin-editable even for an env-configured instance.

### Mechanism

1. **One additive column**: `oauth_providers.config_source` (`'admin' | 'env'`, default `'admin'`). Every row that existed before this feature defaults to `'admin'` with no data migration — satisfying "no disruption to already-working logins."
2. **Startup-time materialization, not a parallel runtime model**: at server startup, `OAUTH_*` environment variables are parsed (naming scheme below) and upserted directly into `oauth_providers` rows tagged `config_source = 'env'`. Env-derived instances are ordinary DB rows — the existing UUID-keyed admin GraphQL surface (`oauthProviders`, `updateOauthProvider`) works for them unchanged, just with a write guard: only `enabled` is writable through the admin mutation on an `'env'`-sourced row; every other field is env-authoritative and re-asserted on every restart.
3. **Provider-generic, not a fixed list**: the existing `oauth_providers` schema already has no provider-type-specific columns (`authorization_url`/`token_url`/`userinfo_url`/`scopes` are plain fields) — Discord/Google/GitHub are already seeded as unconfigured template rows this way. New built-in presets (Keycloak/generic-OIDC and others) follow the same seed-migration pattern. A fully-generic provider (no built-in preset) is supported by accepting the operator's own full endpoint set via env vars under an arbitrary provider name.
4. **Multi-instance via provider_key, not new columns**: a named instance's `provider_key` becomes the compound form `<provider>__<instance>` (e.g. `keycloak__work`) — still a single opaque string in the existing `provider_key UNIQUE` column, requiring no route or schema change. The env-var naming scheme is `OAUTH_<PROVIDER>_[<INSTANCE>_]<FIELD>`.
5. **No new redirect-URI mechanism**: the frontend already derives each provider's OAuth callback URL client-side from `window.location.origin` + `provider_key` (`apps/web/src/api/auth.ts`). A named instance's login button simply carries its compound `provider_key`, and the existing derivation produces a distinct, correct callback URL with zero new code.
6. **Removing env vars doesn't delete data**: if a previously env-sourced instance's environment variables disappear at the next restart, its row's `config_source` reverts to `'admin'` (values retained, now admin-editable) rather than being deleted — deleting would cascade-orphan any already-linked `user_oauth_accounts` (`ON DELETE CASCADE`), which a configuration change alone must never do.

### Schema change

```sql
ALTER TABLE oauth_providers
  ADD COLUMN config_source VARCHAR NOT NULL DEFAULT 'admin';
```

Plus a new seed-style migration (following `2026-05-02-021115-0002_seed_oauth_providers_and_credentials_fields`'s exact pattern) adding at least a Keycloak/generic-OIDC template row.

### Authorization

Unchanged from ADR-017/Principle III: `oauthProviders`/`updateOauthProvider` remain admin-gated (`admin_user(ctx)?`). The env-var scan is a server-startup-only operation with no client input path — the client never supplies or influences provider endpoint/credential data, only the (pre-existing) `redirect_uri` it wants used for its own callback.

## Consequences

### Positive

1. Operators get true zero-admin-panel-touch provider enablement (the feature's core ask) without ADR-017's runtime admin surface losing any capability.
2. The schema change is minimal (one column) because the existing `oauth_providers` table was already provider-generic — no new tables, no new provider-type enum to maintain.
3. Multi-instance and redirect-URI handling both fall out of existing mechanisms (`provider_key`'s opaque-string nature, the client's existing origin-based redirect derivation) rather than requiring new ones.

### Negative

1. `config_source = 'env'` rows are a partial exception to "the admin panel can edit any provider" — the admin UI must render this distinction clearly (masked/read-only fields) or an admin's apparently-successful edit will silently not persist.
2. The env-var naming parser (longest-known-preset-prefix, then known-field-suffix, remainder-is-instance-key) is new, non-trivial string-parsing logic that needs its own unit test coverage — there is no existing precedent for this exact scheme elsewhere in the codebase.

## Alternatives Considered

1. **Environment-variable-only provider setup** (ADR-017's original rejected alternative) — still rejected, for the same reason: operators need the runtime admin surface. This ADR keeps that reasoning intact by making env vars *additive*, not a replacement.
2. **A single JSON-blob env var** (`THUNDERFORGE_AUTHENTICATION`, already dead-coded in `src/server/src/config/mod.rs`, parsed but never wired to anything) — rejected; doesn't match the explicit per-variable `OAUTH_<PROVIDER>_*` convention this feature commits to, and the dead code is being removed as part of this feature rather than revived.
3. **Purely in-memory env-derived provider instances, unioned with DB rows at read time** — rejected; would require a parallel, non-DB-backed admin query/mutation surface (env rows would have no `provider_id` to address through the existing UUID-keyed contract), doubling the admin API's shape for no behavioral benefit over startup materialization.
4. **A separate compiled-in Rust preset registry, independent of the DB seed rows** — rejected as the primary mechanism (would duplicate and risk drifting from the seed migration's URLs); a small Rust-side lookup is still used, but only to resolve presets in environments where a given preset's seed row hasn't been migrated yet, not as the source of truth once seeded.

## Security Implications

- Client-supplied data never determines provider endpoint or credential configuration — only the (pre-existing, unchanged) `redirect_uri` the client wants used for its own callback, which the server already validates against the requested provider before use.
- Secrets are never exposed back to the browser from either configuration source — `hasClientSecret: Boolean!` remains the only client-visible signal, unchanged from ADR-017.
- The `config_source`-aware write guard prevents an admin-panel edit from silently appearing to override an operator's deploy-time credential without actually taking effect unnoticed — the mutation's response always reflects the row's real, persisted values.

## Related ADRs

- ADR-017: OAuth Provider Configuration Contract (extended, not superseded — this ADR adds a second configuration source alongside it)
- ADR-001: Unified Authentication Model (unchanged — every auth path, including every provider instance added by this feature, still resolves to the same `users` row)
- ADR-006 / ADR-008: OAuth linking safety rules, bootstrap-admin exception (unchanged, continue to apply)

## References

- Spec 007: `specs/007-configurable-auth-providers/` (research.md §1-§6, data-model.md, contracts/oauth-provider-admin.md)
