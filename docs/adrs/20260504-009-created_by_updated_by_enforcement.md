# ADR-009: Created-By / Updated-By Enforcement Across All Tables

**Status:** Accepted

**Decision Date:** 2026-05-04

## Context

Ownership metadata is only useful if every persisted user-generated row actually sets and maintains it consistently.

## Decision

Treat `created_by`, `updated_by`, `created_at`, and `updated_at` as mandatory contract fields for persisted user-generated tables. New writes must set them, updates must maintain them, and future migrations must add them when new tables are introduced.

## Consequences

### Positive

1. Export/delete logic remains consistent as the schema grows.
2. Future moderation and audit features start with stable metadata.

### Negative

1. Mutation code must carry authenticated user identity into persistence layers.
2. Backfills are required whenever legacy tables are upgraded.

## Alternatives Considered

1. **Best-effort metadata on some tables only** — rejected because it breaks deletion/export completeness.
2. **Database triggers only** — deferred because some values depend on authenticated request identity.

## Migration Implications

- Existing tables need backfill migrations.
- Future tables should not ship without ownership metadata.

## Security Implications

- Missing ownership metadata becomes a correctness and security risk because row authority becomes ambiguous.
