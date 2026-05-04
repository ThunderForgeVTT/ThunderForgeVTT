# ADR-014: Placeholder Domain Objects in the API Contract

**Status:** Accepted

**Decision Date:** 2026-05-04

## Context

The product roadmap already names `scenes`, `actors`, `asset_packs`, and `game_systems`, but those domains do not yet have persistence in this checkout. The export and GraphQL contracts still need forward-compatible placeholders so clients can integrate incrementally.

## Decision

Expose placeholder sections for:

- `scenes`
- `actors`
- `asset_packs`
- `game_systems`

These placeholders:

- are versioned
- expose a status marker indicating placeholder-only support
- appear in the export contract now
- do not introduce persistence, resolvers with backing queries, or fake seed data

## Consequences

### Positive

1. Clients can code against the eventual contract shape early.
2. The backend avoids pretending unsupported domains are real persisted resources.
3. Future migrations can replace placeholders incrementally instead of reshaping the entire export contract.

### Negative

1. Some API fields intentionally return empty arrays for now.
2. Consumers must distinguish placeholder presence from implemented persistence.

## Alternatives Considered

1. **Omit future domains entirely** — rejected because it would cause avoidable contract churn later.
2. **Add stub tables now** — rejected because it would jump ahead to persistence that the repo does not yet support.

## Migration Implications

- None for the placeholder-only phase.
- Future persistence can adopt the placeholder field names and versioning.

## Security Implications

- Empty placeholder sections expose no additional user data.
- Avoiding fake persistence reduces the risk of partially secured future tables.
