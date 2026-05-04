# ADR-017: OAuth Provider Configuration Contract

**Status:** Accepted

**Decision Date:** 2026-05-04

## Context

OAuth providers are persisted in ThunderForgeVTT already, but Phase 2 requires an administrative contract for reviewing and updating provider configuration from the app. The contract must fit the current schema while avoiding unsafe secret handling patterns.

## Decision

OAuth provider administration will use explicit GraphQL queries and mutations over the existing `oauth_providers` persistence model.

- admin queries return configured providers and their status
- admin mutations update one provider at a time by provider identifier
- the frontend uses typed forms instead of a generic key-value editor

## Consequences

### Positive

1. Provider management matches the rest of the admin GraphQL surface.
2. Single-provider mutations keep validation and audit expectations simple.
3. The UI can present clear enabled/configured state without exposing unrelated config internals.

### Negative

1. The contract is limited to the current provider table shape.
2. Adding new provider fields requires coordinated backend and frontend updates.

## Alternatives Considered

1. **Environment-variable-only provider setup** — rejected because operators need a runtime admin surface for persisted provider records.
2. **Manifest-only OAuth configuration** — rejected because provider state already has a dedicated persistence model and should not be duplicated.

## Security Implications

- Only admins may query or mutate provider configuration.
- The UI should prefer status-oriented displays and carefully scoped editable fields for secrets.
- Provider updates must stay explicit per provider to reduce accidental bulk misconfiguration.
