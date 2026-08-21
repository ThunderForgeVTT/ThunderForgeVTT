# Phase 0 Research: Configurable Multi-Provider Authentication

## 1. There's already a dead, unwired attempt at env-var OAuth config — remove it, don't extend it

**Decision**: Delete `src/server/src/config/mod.rs`'s `SupportedAuthentication`/`OAuth2Config` types and the `THUNDERFORGE_AUTHENTICATION` env var they're parsed from, rather than building this feature on top of them.

**Evidence**: `Config::from_env()` parses `THUNDERFORGE_AUTHENTICATION` as a JSON blob into `Option<Vec<OAuth2Config>>` and stores it on `Config.authentication` — but `grep` across the entire server crate shows `config.authentication` is never read anywhere after `main.rs` constructs it. It has no path to `oauth_providers`, no path to the login flow, nothing. It's inert scaffolding from an earlier, abandoned pass at this same problem.

**Rationale**: The user's actual request is explicit per-variable naming (`OAUTH_DISCORD_CLIENT_ID`, `OAUTH_KEYCLOAK_CLIENT_ID`, etc.), not a single JSON blob — a fundamentally different (and, for an operator hand-editing a `.env` file, much more usable) shape. Keeping the dead code around as unrelated scaffolding invites confusion about which mechanism is real. This repo has precedent for deleting confirmed-dead auth-adjacent code outright (`docs/SECURITY_RBAC.md`'s `RbacEngine` deletion, referenced in `MVP.md`) rather than leaving it as an unused parallel path.

**Alternatives considered**: Repurpose `OAuth2Config`/`SupportedAuthentication` as the new mechanism's shape — rejected because a single JSON-blob env var doesn't match the per-variable `OAUTH_<PROVIDER>_*` convention the spec commits to (FR-001), and would require the same amount of new parsing code anyway, just parsing JSON instead of env-var names.

## 2. The DB schema is already provider-generic — the "preset library" already exists as seeded rows

**Decision**: Treat the existing `oauth_providers` table and its Discord/Google/GitHub seed rows (migration `2026-05-02-021115-0002_seed_oauth_providers_and_credentials_fields`) as the live template source for those three presets. Add Keycloak/generic-OIDC (and any other new preset) the same way: a new seed-style migration inserting an unconfigured (`configured = false`, `enabled = false`, null credentials) row with that preset's known `authorization_url`/`token_url`/`userinfo_url`/`scopes`.

**Evidence**: `oauth_providers` has no provider-type-specific columns at all — `provider_key`, `display_name`, `authorization_url`, `token_url`, `userinfo_url`, `scopes` are all plain strings/arrays. The seed migration already inserts working Discord/Google/GitHub template rows this way, unconfigured until an admin (or, after this feature, an env var) supplies `oauth_client_id`/`oauth_client_secret`.

**Rationale**: This means FR-002's "generic template + preset library" isn't new architecture to invent — it's the schema this app already has. The only genuinely new work is (a) a second way to *fill in* `oauth_client_id`/`oauth_client_secret` (and, for the fully-generic case, the URL/scope fields too) — from env vars instead of only the admin form — and (b) resolving precedence when both sources are present.

**Alternatives considered**: A separate compiled-in Rust preset registry (const table of known providers), independent of the DB. Rejected as the primary mechanism because it would duplicate the URLs the seed migration already encodes and could drift from them; **however**, a small Rust-side lookup table is still needed as the resolution source specifically for *env-var-only* deployments where an operator sets `OAUTH_KEYCLOAK_CLIENT_ID` before any Keycloak seed row exists in a given environment's migration history — see Decision 3.

## 3. Env-var provider instances are materialized as ordinary DB rows at startup, not a parallel in-memory model

**Decision**: At server startup, after running migrations, scan `OAUTH_*` env vars (Decision 4's naming scheme), resolve each detected provider instance's endpoint/scope data (from the matching seeded preset row if one exists for that base provider name, or from the operator's own fully-specified generic env vars), and `INSERT ... ON CONFLICT (provider_key) DO UPDATE` the resulting row into `oauth_providers` with the new `config_source = 'env'` marker — mirroring the existing seed migration's `ON CONFLICT` shape exactly. The startup upsert **updates** `display_name`/`authorization_url`/`token_url`/`userinfo_url`/`scopes`/`oauth_client_id`/`oauth_client_secret`/`configured`/`config_source` on every restart (env vars are always re-authoritative for these), but **only sets `enabled = true` on first insert** — an admin's later `enabled = false` toggle (FR-006) must survive a restart, not get silently flipped back on by the next env-var re-scan.

**Evidence/Rationale**: The existing admin GraphQL surface (`oauthProviders` query, `updateOauthProvider(providerId: UUID!, ...)` mutation) is entirely UUID/DB-row-keyed — see `src/server/src/graphql/queries/admin.rs` and `src/server/src/graphql.rs`'s `AdminMutation::update_oauth_provider`. Materializing env-derived instances as real rows means this entire existing contract keeps working unchanged for them (the admin panel can list, view, and toggle `enabled` on an env-sourced row through the exact same query/mutation an admin-configured row uses), and FR-008's "admin panel shows env-sourced values as read-only" becomes a client-side (and mutation-handler-enforced) rendering/write-guard concern, not a second data-fetching path to build and keep in sync with the first.

**Alternatives considered**: Keep env-derived instances purely in-memory, never touching the DB, and union them with DB rows at every read (login screen, admin panel, oauth start/callback). Rejected: this would require a parallel non-DB-backed admin query/mutation surface (env rows have no `provider_id` to address), doubling the admin API's shape for no behavioral benefit, and would need its own request-time caching to avoid re-parsing env vars on every login-screen load.

## 4. Env var naming/parsing scheme for provider + optional instance key + field

**Decision**: `OAUTH_<PROVIDER>_[<INSTANCE>_]<FIELD>`, where:
- `<PROVIDER>` matches a known preset name (`DISCORD`, `GITHUB`, `GOOGLE`, `KEYCLOAK`, ...) case-insensitively, **or** is treated as a fully-generic provider name if a `_TYPE=generic` marker or (simpler, chosen here) the full generic field set (`_AUTHORIZATION_URL`, `_TOKEN_URL`, `_CLIENT_ID`, `_CLIENT_SECRET` at minimum) is present under that name.
- `<INSTANCE>` is optional. The parser matches the longest known preset-name prefix, then a known `<FIELD>` suffix (`CLIENT_ID`, `CLIENT_SECRET`, `LABEL`, and preset-specific fields like `ISSUER_URL` for Keycloak/generic-OIDC-shaped presets) at the end; anything left between them is the instance key verbatim (e.g. `OAUTH_KEYCLOAK_WORK_CLIENT_ID` → provider `keycloak`, instance `work`, field `client_id`). No instance segment (`OAUTH_KEYCLOAK_CLIENT_ID`) means the default/unnamed instance.
- The resulting `provider_key` stored in the DB is `<provider>` for the default instance, or `<provider>__<instance>` (double underscore, lower-cased) for a named one — kept as a single opaque string so it slots into the existing `provider_key UNIQUE` column and the existing `{provider_key}` route path segment with zero routing changes.

**Rationale**: This reuses the existing route shape (`/authentication/oauth/{provider_key}/start`) and the existing client-side `redirect_uri` construction (`apps/web/src/api/auth.ts`'s `startOAuthLogin`, which already builds `${window.location.origin}/oauth/callback/${providerKey}` from whatever `provider_key` the login-screen button carries) completely unchanged — a named instance's button just carries `provider_key = "keycloak__work"` and the whole existing redirect flow works with no new code path. This is also why FR-013 (redirect URI) needed no new mechanism: the app already auto-derives it client-side from origin + provider_key.

**Alternatives considered**: A separate `OAUTH_INSTANCES` env var listing instance names to disambiguate parsing. Rejected as an unnecessary extra variable — the longest-known-preset-prefix-then-known-suffix parse is unambiguous in practice (preset names and field suffixes are both from small, fixed vocabularies) and keeps every setting on its own single env var, matching the spec's own examples.

## 5. `config_source` write-guard in `update_oauth_provider`

**Decision**: `update_oauth_provider`'s handler checks the target row's `config_source` before applying `GraphQLOAuthProviderConfigInput`. For `config_source = 'env'` rows: only `enabled` is applied from the input; any other field present in the input is ignored (not erased — just not written), and the response reflects the row's real (env-sourced) values so the client isn't shown a false success. For `config_source = 'admin'` rows: unchanged existing behavior.

**Rationale**: Matches FR-008/FR-006 exactly — env vars win for credential/endpoint fields, but the enable/disable toggle stays admin-editable regardless of source. Doing this as a value-level guard inside the existing mutation (rather than rejecting the whole request) keeps the admin UI's existing single "Save" affordance working without a special-cased second mutation.

## 6. Removing an env var for a previously-env-sourced instance

**Decision**: The startup scan only *adds/updates* rows for env vars it currently finds; it never deletes a row. If a previously `config_source = 'env'` row's env vars are gone at the next restart, the startup routine flips that row's `config_source` back to `'admin'` (leaving its already-persisted `oauth_client_id`/secret/URLs exactly as they were) rather than deleting it or leaving it permanently `'env'`-locked with no env vars to justify that lock.

**Rationale**: Directly resolves the edge case documented in spec.md ("operator changes or removes the environment variables at the next deploy... reasonable default: the previously-entered... values are preserved and take effect immediately once the env vars are gone"). Deleting the row outright would cascade-delete any `user_oauth_accounts` linked through it (`ON DELETE CASCADE`), silently breaking existing users' logins — never acceptable for a config change alone.
