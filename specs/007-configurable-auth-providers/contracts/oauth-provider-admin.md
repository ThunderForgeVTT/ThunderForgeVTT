# Contract: Admin OAuth Provider GraphQL Surface (extended)

## Existing, unchanged authorization — extended output/input shape

### `oauthProviders` query (admin-only, existing query, extended output)

```graphql
type GraphQLOAuthProvider {
  id: UUID!
  providerKey: String!
  displayName: String!
  authorizationUrl: String!
  tokenUrl: String!
  userinfoUrl: String
  scopes: [String!]!
  oauthClientId: String
  configured: Boolean!
  enabled: Boolean!
  hasClientSecret: Boolean!
  updatedAt: DateTime!
  # New this feature:
  configSource: OAuthConfigSource!   # ADMIN | ENV
}

enum OAuthConfigSource {
  ADMIN
  ENV
}

oauthProviders: [GraphQLOAuthProvider!]!
```

- Authorization unchanged: admin-only (`admin_user(ctx)?`), exactly as today.
- `configSource: ENV` rows are the ones this feature's startup scan upserted from `OAUTH_*` env vars (research.md §3). `configSource: ADMIN` covers every pre-existing row (FR-014) and every row an admin configures with no matching env vars.
- `providerKey` for a named instance is the compound form (`keycloak__work`) described in research.md §4 — the admin UI is expected to render this as "Keycloak (work)" or similar, not the raw string, but that's a presentation concern for the frontend, not a contract change.

### `updateOauthProvider` mutation (admin-only, existing mutation, source-aware write guard)

```graphql
input GraphQLOAuthProviderConfigInput {
  displayName: String
  oauthClientId: String
  oauthClientSecret: String
  enabled: Boolean
  userinfoUrl: String
  scopes: [String!]
}

updateOauthProvider(providerId: UUID!, config: GraphQLOAuthProviderConfigInput!): GraphQLOAuthProvider!
```

- Input shape is **unchanged** — no new fields added to the input. What changes is server-side handling based on the target row's `configSource`:
  - `configSource: ADMIN` — unchanged existing behavior; every field in the input that's present gets written, exactly as today.
  - `configSource: ENV` — **only `enabled` is applied**. Any other field present in the input (`displayName`, `oauthClientId`, `oauthClientSecret`, `userinfoUrl`, `scopes`) is silently ignored (not erased, not written) rather than rejecting the whole request — the admin UI is expected to have already disabled those inputs client-side per `configSource`, so a well-behaved client never sends them for an `ENV` row in the first place; the server-side guard exists so this can't be bypassed by calling the mutation directly.
- The returned `GraphQLOAuthProvider` always reflects the row's real, persisted values after the write — for an `ENV` row, this means the response shows the env-sourced credentials/URLs unchanged even if the caller's input tried to set different ones, so the client isn't shown a false "your edit was saved."

## Unchanged: OAuth start/callback REST-ish routes

`/authentication/oauth/{provider_key}/start`, `/authentication/oauth/{provider_key}/callback`, and the setup-flow equivalents are **unchanged in shape**. `provider_key` already accepts any string matching an `oauth_providers.provider_key` row — a named instance's compound key (`keycloak__work`) needs no new route or parameter, it's just a `provider_key` value like any other. Same for the existing `redirect_uri` query parameter the client already supplies on `start` — no server-side redirect-URI derivation is added (research.md §4 / spec.md FR-013 is satisfied by the client's existing `${window.location.origin}/oauth/callback/${providerKey}` construction in `apps/web/src/api/auth.ts`).
