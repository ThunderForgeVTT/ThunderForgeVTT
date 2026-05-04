# ADR-003: User Data Export Contract

**Status:** Accepted

**Decision Date:** 2026-05-04

## Context

ThunderForgeVTT needs a user-facing “download all my data” capability that works now against real persisted data, while leaving room for future domains such as scenes and actors.

## Decision

Expose `GET /user/data/export` for authenticated self-export. Support:

- `format=json`
- `format=zip`

The payload includes a manifest, the authenticated user profile, and all currently persisted user-created rows. Future domains that do not yet have persistence are represented as empty versioned sections.

## Consequences

### Positive

1. Export shape is stable before every future table exists.
2. JSON is easy to inspect; ZIP provides a delivery format for future asset packaging.
3. The endpoint is limited to self-service only.

### Negative

1. Some export sections are placeholders until future persistence arrives.
2. ZIP currently wraps JSON rather than a full binary asset archive.

## Alternatives Considered

1. **Wait until every domain exists** — rejected because privacy/export obligations start with Phase 1 identity.
2. **Database dump per user** — rejected because it would leak internal schema coupling and non-user rows.

## Migration Implications

- Depends on ownership metadata being present on persisted user-generated tables.
- Does not require separate export shadow tables.

## Security Implications

- Only the authenticated user may export their data.
- No admin override path exists in Phase 1.
- Export responses should be treated as sensitive downloads.
