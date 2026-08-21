# Phase 1 Data Model: Configurable Multi-Provider Authentication

## `oauth_providers` (existing table — one additive column)

No new tables. One new column, additive and backward-compatible:

| Column | Type | Default | Notes |
|---|---|---|---|
| `config_source` | `VARCHAR` (`'admin'` \| `'env'`) | `'admin'` | **New.** Every pre-existing row defaults to `'admin'` on migration — satisfies FR-014 with no data change. Set to `'env'` by the startup env-var scan (research.md §3) for any instance it upserts; flipped back to `'admin'` if that instance's env vars later disappear (research.md §6). |

All other existing columns (`id`, `provider_key`, `display_name`, `authorization_url`, `token_url`, `userinfo_url`, `scopes`, `oauth_client_id`, `oauth_client_secret`, `configured`, `enabled`, `created_at`, `updated_at`) are unchanged in shape and meaning. `provider_key` becomes the compound identifier for named instances (research.md §4: `<provider>` or `<provider>__<instance>`), but remains the same `VARCHAR NOT NULL UNIQUE` column — no type or constraint change.

### Validation rules (new/changed)

- `config_source = 'env'` rows: `oauth_client_id`/`oauth_client_secret`/`authorization_url`/`token_url`/`userinfo_url`/`scopes`/`display_name` are **written only by the startup env-var scan**, never by the admin GraphQL mutation (research.md §5's write-guard). `enabled` remains admin-writable regardless of `config_source`.
- `config_source = 'admin'` rows: unchanged existing behavior — the admin mutation may write any field.
- `provider_key` for a named instance MUST match the `<provider>__<instance>` shape produced by the env-var parser (research.md §4) when `config_source = 'env'`; admin-created rows (`config_source = 'admin'`) are free-form as today (no instance-key concept applies to admin-panel-only providers — multi-instance is an env-var-driven capability per FR-012's naming convention, not a new admin-panel "add instance" flow in this feature's scope).

## New built-in preset seed data (migration, following the existing pattern)

Extends the existing `2026-05-02-021115-0002_seed_oauth_providers_and_credentials_fields`-style seed with additional unconfigured template rows, via a new migration. At minimum for this feature: Keycloak/generic-OIDC. (GitLab and others may follow the same pattern in later, config-only migrations per FR-002's "low-effort addition" framing — not required to land in this feature's first migration.)

Keycloak is a self-hosted, per-deployment provider — unlike Discord/Google/GitHub's fixed public endpoints, its `authorization_url`/`token_url`/`userinfo_url` depend on the operator's own realm. The seed row therefore cannot hard-code real URLs the way Discord/Google/GitHub's rows do; instead:

- The seed row exists primarily so the admin panel has a discoverable, unconfigured "Keycloak" entry to fill in by hand (existing admin flow, unchanged).
- The **env-var path** for Keycloak requires an additional non-secret setting beyond `CLIENT_ID`/`CLIENT_SECRET` — an issuer/base URL (`OAUTH_KEYCLOAK_ISSUER_URL`, e.g. `https://idp.example.com/realms/myrealm`) — from which the standard OIDC endpoint paths (`/protocol/openid-connect/auth`, `/protocol/openid-connect/token`, `/protocol/openid-connect/userinfo`) are derived at parse time, rather than requiring the operator to type all three URLs out.

## Entity summary (maps to spec.md's Key Entities)

- **Auth Provider Instance** (spec.md) = one `oauth_providers` row. `config_source` is the persisted form of "configuration source"; `provider_key`'s compound shape is the persisted form of "instance key"; all other spec-level attributes (template, enabled/disabled, display label, connection settings) map directly to existing columns.
- **User Identity** = existing `users` table. Unchanged.
- **Linked Provider Identity** = existing `user_oauth_accounts` table (`provider_id` FK to `oauth_providers.id`). Unchanged — a named instance is just another `oauth_providers` row, so linking works identically to any existing provider.

## State transitions

```text
(no row) --startup env scan detects instance--> config_source='env', configured=true, enabled=true
config_source='env' --admin toggles enabled--> config_source='env', enabled=<admin's choice> (unchanged on restart)
config_source='env' --operator removes that instance's env vars, restart--> config_source='admin' (all prior values retained)
config_source='admin' --admin fills in credentials via panel--> config_source='admin', configured=true (existing behavior)
config_source='admin' --operator later sets matching env vars, restart--> config_source='env' (env vars now authoritative; prior admin-entered values are overwritten by the scan per research.md §3)
```
