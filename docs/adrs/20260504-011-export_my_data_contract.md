# ADR-011: Export-My-Data Contract

**Status:** Accepted

**Decision Date:** 2026-05-04

## Context

ThunderForgeVTT needs a self-service export that works against the data the system actually persists today while still reserving schema space for domains that have not been implemented yet.

## Decision

Expose a self-only export contract through:

- GraphQL `exportMyData`
- Axum `GET /user/data/export`

The payload includes:

- a versioned manifest
- the authenticated user profile
- owned `worlds`
- owned `world_tokens`
- owned `world_events`
- owned `policies`
- placeholder arrays for `scenes`, `actors`, `asset_packs`, and `game_systems`

The Axum endpoint supports JSON download directly and ZIP wrapping of that JSON payload.

## Consequences

### Positive

1. The export surface is stable before every future table exists.
2. GraphQL and file download paths share the same underlying export builder.
3. Clients can prepare for future domains without fake persistence.

### Negative

1. Placeholder sections are intentionally empty until those domains are implemented.
2. ZIP currently packages JSON only, not binary assets.

## Alternatives Considered

1. **Wait until every domain exists** — rejected because users need an exportable account surface now.
2. **Return raw table dumps** — rejected because it leaks internal schema details and bypasses contract versioning.

## Migration Implications

- Depends on ownership metadata existing on the persisted tables in scope.
- Does not require separate export shadow tables.

## Security Implications

- Only the authenticated user may export their own data.
- There is no admin override export path in this contract.
- Export payloads must be treated as sensitive user data.
