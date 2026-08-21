# Implementation Plan: Configurable Multi-Provider Authentication

**Branch**: `007-configurable-auth-providers` | **Date**: 2026-08-21 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/007-configurable-auth-providers/spec.md`

**Note**: This template is filled in by the `/speckit-plan` command; its definition describes the execution workflow.

## Summary

Adds an environment-variable configuration source for OAuth login providers, layered on top of the existing DB-backed `oauth_providers` admin panel (ADR-017) — env vars always win for the fields they set. Providers are generic (one OAuth2/OIDC template underneath), with a small library of built-in presets (Discord/Google/GitHub already DB-seeded; Keycloak/generic-OIDC and others added the same way) that pre-fill known endpoints, plus a fully-generic fallback for any other OAuth2/OIDC provider. Multiple named instances of the same provider template are supported from day one via an env-var naming convention. Username/password stays untouched and always visible. The redirect/callback URI is already client-derived from `window.location.origin` + `provider_key` (`apps/web/src/api/auth.ts`) — this feature just needs `provider_key` to also carry the instance key, so no new redirect-URI mechanism is needed. The core mechanism requires one small, additive DB migration (a `config_source` column) and no new tables.

## Technical Context

**Language/Version**: Rust 2024 edition (`src/server`), TypeScript/React (`apps/web`) — unchanged.

**Primary Dependencies**: Existing stack only — Axum, async-graphql, Diesel/PostgreSQL, `reqwest` (this app's OAuth2 code is hand-rolled against these, no `oauth2` crate). No new dependency required.

**Storage**: PostgreSQL, existing `oauth_providers` table. One additive migration: a `config_source` column (`'admin' | 'env'`, default `'admin'`) so pre-existing rows need no data change (satisfies FR-014 automatically) and the admin panel can tell which fields are env-authoritative (FR-008). No new tables — env-derived provider instances are represented as ordinary `oauth_providers` rows (upserted at server startup from process env), not a separate in-memory-only model, so the existing admin GraphQL surface (UUID-keyed `updateOauthProvider`) keeps working unchanged for them, just with a source-aware write guard.

**Testing**: `cargo check`/`cargo test` (server — new env-var-parsing/precedence/preset-resolution unit tests, colocated per this repo's existing `#[cfg(test)]` convention in `auth/mod.rs`), `tsc`/`vite build` (web), Playwright (`apps/web/e2e/` — new spec covering sign-in-screen button rendering across env-var/admin-panel/multi-instance combinations and the admin panel's read-only/masked display for env-sourced rows).

**Target Platform**: Linux server (unchanged).

**Project Type**: Web application — existing `src/server` / `apps/web` layout.

**Performance Goals**: Provider-list resolution (sign-in screen, admin panel) stays within the existing `configured_oauth_providers` query's latency budget — env-var scanning happens once at startup (upsert into DB), not per-request, so steady-state reads are unchanged DB queries.

**Constraints**: Constitution Principle III — env-var-derived provider config is resolved and upserted server-side only at startup; the client never supplies or influences provider endpoint/credential data, only the (already-existing) `redirect_uri` it wants used for its own callback. Secrets (FR-009) are never round-tripped to the browser in any form, masked or otherwise, beyond a `has_client_secret: bool`, matching the existing `GraphQLOAuthProvider` shape.

**Scale/Scope**: Single-digit to low-tens of configured provider instances per deployment — no scale concern. Touches `src/server/src/config/`, `src/server/src/auth/mod.rs`, `src/server/src/graphql/admin_types.rs`, one new migration, and `apps/web/src/api/auth.ts` (provider_key → instance-aware key) plus the admin panel's `OAuthProviderForm.tsx` (read-only state for env-sourced rows). No `src/engine` changes.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **Principle I (ECS owns simulation)**: N/A — this feature touches auth/admin only, no canvas/engine code.
- **Principle II (Plugin-modular engine)**: N/A — same reason.
- **Principle III (Ownership & authorization at the data boundary)**: PASS. `update_oauth_provider` remains admin-gated (`admin_user(ctx)?`, unchanged). The new write guard (reject/ignore credential-field edits on `config_source: 'env'` rows, still allow the `enabled` toggle per FR-006) is enforced in the same server-side mutation handler, not the client. Env-var scanning is a startup-time server process operation with no client input path.
- **Principle IV (Real ADRs and specs before divergent implementation)**: **GATE ITEM** — this introduces a new config-precedence model and an env-var-driven provider-instance concept, which is architecturally significant per the constitution's own examples ("new subsystem... changing an ownership boundary" — this changes how provider config is sourced/trusted). **Resolution**: ADR-041 drafted as part of this planning phase (see `docs/adrs/20260821-041-env_var_oauth_provider_configuration.md`), landing in the same change set as this spec per the constitution's requirement. Gate re-passes with the ADR present.
- **Principle V (Verify before done)**: Plan calls for `cargo check`/`cargo test` (server, new parsing/precedence logic), `tsc`/`vite build` (web), and live Playwright verification of the sign-in screen and admin panel across the env-var/admin-panel/multi-instance matrix before this feature is reported done.

**Gate result**: PASS, with ADR-041 satisfying Principle IV. No Complexity Tracking entries needed — the one schema change is additive and minimal, not a structural deviation.

## Project Structure

### Documentation (this feature)

```text
specs/007-configurable-auth-providers/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── contracts/
│   └── oauth-provider-admin.md   # Phase 1 output — extended admin GraphQL contract
├── quickstart.md        # Phase 1 output
└── tasks.md              # Phase 2 output (/speckit-tasks — not created by this command)

docs/adrs/
└── 20260821-041-env_var_oauth_provider_configuration.md   # New ADR (Principle IV)
```

### Source Code (repository root)

```text
src/server/
├── migrations/
│   └── <timestamp>_add_oauth_config_source/   # New: config_source column, default 'admin'
├── src/
│   ├── config/
│   │   └── mod.rs                # Remove dead THUNDERFORGE_AUTHENTICATION/SupportedAuthentication/
│   │                              # OAuth2Config scaffolding (parsed, never wired to anything — see
│   │                              # research.md); add new OAuth env-var scanning/parsing module here
│   │                              # or a sibling `config/oauth_env.rs`.
│   ├── auth/
│   │   └── mod.rs                # Startup-time upsert of env-derived provider instances into
│   │                              # oauth_providers; provider_key now instance-aware
│   │                              # (base key, or base_key + delimiter + instance key)
│   ├── graphql/
│   │   ├── admin_types.rs        # GraphQLOAuthProvider gains config_source; write-guard in the
│   │   │                         # update_oauth_provider mutation path
│   │   └── models.rs (or models.rs at crate root)  # OAuthProvider/NewOAuthProvider gain config_source
│   └── models.rs
└── migrations/<existing oauth seed migration untouched; new preset rows (Keycloak/generic-OIDC,
    GitLab, ...) added via a new seed-style migration following the existing Discord/Google/GitHub
    pattern>

apps/web/src/
├── api/auth.ts                   # redirect_uri construction already instance-key-ready
│                                  # (uses provider_key verbatim) — confirm/adjust if instance-key
│                                  # delimiter needs client-side awareness for display only
└── pages/admin/components/
    └── OAuthProviderForm.tsx     # Read-only/masked rendering for config_source: 'env' rows,
                                   # enabled toggle stays editable
```

**Structure Decision**: Existing `src/server` / `apps/web` layout, unchanged. No `src/engine` involvement. The one schema change (`config_source` column) is additive; no new tables, matching the "extends, doesn't replace" framing in spec.md's Assumptions.

## Complexity Tracking

> **Fill ONLY if Constitution Check has violations that must be justified**

No unjustified violations — table intentionally omitted. (Principle IV's gate item is resolved via ADR-041, not a tracked exception.)
