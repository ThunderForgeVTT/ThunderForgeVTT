# ADR-006: OAuth Linking Safety Rules

**Status:** Accepted

**Decision Date:** 2026-05-04

## Context

OAuth providers can return emails that overlap with existing local accounts. Silent merges would create account-takeover risk.

## Decision

If an OAuth identity is already linked, sign in normally. If not linked but the provider email matches an existing local user, create an `oauth_link_challenges` record and require local password confirmation before linking. Never silently merge accounts.

## Consequences

### Positive

1. Existing local accounts cannot be hijacked by email overlap alone.
2. OAuth linking becomes an explicit, auditable action.

### Negative

1. Returning users may hit an extra link-confirmation step.
2. Linking UX is more complex than consumer-style social login.

## Alternatives Considered

1. **Auto-link by matching email** — rejected as unsafe.
2. **Disallow linking entirely** — rejected because unified identity needs multiple auth methods per user.

## Migration Implications

- Requires `oauth_link_challenges`.
- Benefits from audit logging for issued and confirmed link events.

## Security Implications

- Password-confirmed linking sharply reduces takeover risk from provider-side email assertions alone.
