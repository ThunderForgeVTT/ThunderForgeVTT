# Contract: Test structure — many clients, one account, and a provider stub

## The multi-client fixture

`apps/web/e2e/fixtures/clients.ts`:

```ts
/** Another signed-in client for an account that is already signed in. */
export async function openAnotherClient(
  browser: Browser,
  creds: Credentials,
  kind: "context" | "tab",
  from?: Page,          // required when kind === "tab"
): Promise<Page>;
```

- `kind: "context"` opens a fresh browser context — its own cookie jar — and
  **signs in for real**. This is the second-browser case, and signing in is
  what makes the test evidence: copying `storageState` would pass whether or
  not the eviction was removed.
- `kind: "tab"` opens another page in an existing context, sharing the session
  as a real second tab does.
- The caller says which it wants. A test that means "a second browser" and a
  test that means "a second tab" are asserting different things (FR-017).

**Rules**

1. No registration. The fixture never creates an account (FR-016).
2. Usable under `scripts/e2e-parallel.mjs` unchanged — no serialisation, no
   worker pinning (FR-018).
3. Specs that register a second account only to obtain a second window move
   onto this; specs that genuinely need two different people stay as they are
   (FR-019). `inviteAndJoinAsPlayer` is in the second group and does not move.
4. One spec asserts the eviction has not returned: sign in in a second
   context, then act in the first (FR-020). It fails loudly if a login ever
   revokes again.

## The OAuth provider stub

A test-only HTTP service started by the harness, one port per shard, exactly
as backends and vite servers already get one.

| Route | Behaviour |
|---|---|
| `GET /authorize` | Redirect straight back to the product's callback with a fixed code |
| `POST /token` | Return a fixed access token |
| `GET /userinfo` | Return the email the scenario chose — verified, unverified, or absent |

**Rules**

1. Seeded as an `oauth_providers` row pointing at the stub's URLs. The table
   already carries `authorization_url`, `token_url` and `userinfo_url` per
   provider, so this needs no new column.
2. **No product code changes.** `exchange_authorization_code_with_provider`
   already takes the provider as an argument; a stub that required a
   `#[cfg(test)]` branch inside `oauth.rs` would test the branch and not the
   flow (FR-023).
3. Ships in the harness and seed data. Never in a release.
4. Four scenarios must be reachable: first-login provisioning (ADR-042),
   linking to an existing account behind password confirmation (ADR-006),
   refusal when no verified email is returned, and refusal by a closed
   instance (spec 035 T056 — struck from the deferred register once green,
   FR-024).
