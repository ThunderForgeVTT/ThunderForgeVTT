# ADR-002: User Data Ownership Model

**Status:** Accepted

**Decision Date:** 2026-05-04

## Context

Phase 1 needs self-service export and deletion, plus future policy enforcement. That requires a concrete ownership model for persisted user-generated resources.

## Decision

For currently persisted user-generated tables, ownership is modeled with:

- `created_by`
- `updated_by`
- `created_at`
- `updated_at`

Phase 1 applies this only to the persisted tables already present in the repo: `worlds`, `world_tokens`, `world_events`, and `policies`.

## Consequences

### Positive

1. Export and deletion can be defined against actual ownership.
2. Export/delete behavior matches the data the repo actually persists today.
3. Audit and moderation logic can reason about authorship directly from each row.

### Negative

1. Every mutation path must keep ownership metadata current.
2. Future collaborative ownership will need an additional contract when that persistence exists.

## Alternatives Considered

1. **Membership only, no row authorship** — rejected because export/delete need creator attribution for non-world rows.
2. **Infer ownership from world ancestry alone** — rejected because policies and events still need direct authorship metadata.

## Migration Implications

- Add ownership fields to current persisted domain tables.
- Backfill existing rows from an existing user or fail loudly if attribution is impossible.

## Security Implications

- Ownership metadata narrows which rows a user may export or destroy.
- Explicit ownership makes destructive actions auditable and less ambiguous.
