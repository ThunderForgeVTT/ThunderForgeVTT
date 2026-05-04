# ADR-001: Unified Authentication Model (Local + OAuth)

**Status:** Accepted

**Decision Date:** 2026-05-04

## Context

ThunderForgeVTT supports both local credentials and OAuth providers, but later authorization, world ownership, policies, and audit trails must all operate on a single internal identity model.

## Decision

Use one `users` table as the canonical identity record. Local login/register and OAuth login/link flows both resolve to that same row, and every successful auth path issues the same DB-backed cookie session.

## Consequences

### Positive

1. Local and OAuth users are not separate account classes.
2. 2FA, world ownership, and policy checks operate on one `user_id`.
3. Downstream APIs do not need auth-method-specific branching.

### Negative

1. OAuth cannot silently create or merge accounts without explicit rules.
2. Linking logic is more complex than a pure social-login-only system.

## Alternatives Considered

1. **Separate local and OAuth user tables** — rejected because it would complicate permissions, ownership, and data export/deletion.
2. **Provider-first identities with optional local password attachment** — rejected because the repo already has local-first bootstrap and password flows.

## Migration Implications

- No second user table is introduced.
- OAuth identities remain in `user_oauth_accounts`, linked by `user_id`.
- Future domains should reference `users.id` only.

## Security Implications

- A unified user record prevents silent duplicate-account drift.
- Shared 2FA and session policy reduces inconsistent enforcement across auth methods.
