# ADR-021: World Metadata Schema

**Status:** Accepted

**Decision Date:** 2026-05-04

## Context

The world object must be durable enough for Phase 3 dashboards while remaining small enough to evolve safely into later scene, actor, and rules-system work. The schema needs to store only core metadata now and reserve structured placeholders for future domain objects.

## Decision

The `worlds` table and GraphQL `World` type persist the following metadata:

- `id`
- `name`
- `description`
- `gameSystemId`
- `interfacePackId`
- `createdBy`
- `updatedBy`
- `createdAt`
- `updatedAt`

The GraphQL `World` contract also exposes placeholder collections or null references for:

- `scenes`
- `actors`
- `tokens`
- `events`
- `gameSystem`
- `interfacePack`

In Phase 3 those placeholders resolve to empty arrays or null values.

## Consequences

### Positive

1. Clients can build dashboards against a stable world contract before deeper persistence arrives.
2. Metadata needed for routing, ownership, and future selector UX is durable immediately.
3. The contract leaves room for richer related objects without forcing a breaking schema change.

### Negative

1. Some fields intentionally expose placeholders rather than useful business data today.
2. The API must distinguish between stored identifier metadata and unresolved referenced objects.

## Alternatives Considered

1. **Store only `id` and `name` until later phases** - rejected because routing and dashboard UX need richer metadata now.
2. **Persist full game system and interface pack documents inside the world row** - rejected because those domains are not implemented yet and would create brittle placeholder blobs.

## Security Implications

- Restricting Phase 3 metadata to simple fields limits accidental overexposure of future domain data.
- Ownership and audit fields remain first-class metadata, preserving traceability for every world record.
