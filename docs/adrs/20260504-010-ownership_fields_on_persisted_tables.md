# ADR-010: Ownership Fields on Persisted Tables

**Status:** Accepted

**Decision Date:** 2026-05-04

## Context

ThunderForgeVTT only persists a subset of user-generated domain data today: `worlds`, `world_tokens`, `world_events`, and `policies`. Export, deletion, and resolver-level authorization need a durable ownership signal on those real tables now, without inventing persistence for future domains.

## Decision

Every currently persisted user-generated table must store:

- `created_by`
- `updated_by`
- `created_at`
- `updated_at`

Phase 1 applies that contract only to `worlds`, `world_tokens`, `world_events`, and `policies`. Mutation paths must set `created_by`/`updated_by` from the authenticated user and bump `updated_at` on writes.

## Consequences

### Positive

1. Export and deletion can be defined in terms of actual row ownership.
2. GraphQL and Axum handlers can enforce access consistently with one predicate: `created_by == authenticated_user_id`.
3. Future domains can adopt the same ownership contract when their persistence arrives.

### Negative

1. Existing write paths must be audited so ownership metadata is never skipped.
2. Ownership currently models creator control, not future collaborative membership semantics.

## Alternatives Considered

1. **Infer ownership from joins or world ancestry** — rejected because it is ambiguous for standalone rows like policies and events.
2. **Add ownership only to worlds** — rejected because export/delete also need direct ownership for tokens, events, and policies.

## Migration Implications

- Add the four ownership columns to the four persisted tables already present in the repo.
- Backfill existing rows against a known user during migration so the schema stays non-nullable.

## Security Implications

- Ownership checks become explicit instead of accidental.
- Destructive and export operations can be safely scoped to self-owned rows only.
