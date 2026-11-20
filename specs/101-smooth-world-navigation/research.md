# Research: Smooth World Navigation

## R1 — Shared state mechanism

- **Decision**: widen spec 100's `WorldChromeProvider` (React context,
  per-world record, stale-while-revalidate).
- **Rationale**: it already serves the shell and appearance; the app has
  no state library; six kinds of data is small.
- **Alternatives**: TanStack Query — the right tool past a handful of
  kinds, with dedupe, retries and devtools, but a new dependency the owner
  approves first (FR-010). Zustand — a store without the fetch half; it
  saves nothing over context here. Module-level cache — survives sign-out
  and is invisible to React, so it would need its own subscription.

## R2 — Two components asking for the same key at once

- **Decision**: the provider holds a map of in-flight promises keyed
  `worldId:key`; a second ask joins the first.
- **Rationale**: a route page and its shell both read the world on mount;
  without it every page change doubles the request.
- **Alternatives**: one owner per key (fragile as pages change).

## R3 — When the remembered world is wrong

- **Decision**: `getWorld` resolving `null`, or rejecting with a
  `GraphQLRequestError` that carries a server code, replaces the
  remembered world with `null` (the page shows its not-found state). Any
  other rejection (transport, no response) keeps what was known.
- **Rationale**: FR-008; `GraphQLRequestError.codes` is empty for
  transport failures by design (`api/graphqlClient.ts`).

## R4 — Signing out or switching account

- **Decision**: the provider sits inside `AuthProvider` in both mains,
  reads `user?.id`, and starts from an empty store when it changes
  (reset during render, as `useResetOnChange` does), also dropping
  in-flight answers for the previous user.
- **Rationale**: FR-007; a role or claim must never carry across accounts.

## R5 — Claim gate memory

- **Decision**: remember `claimCleared: true` only after the gate passes;
  render cleared from it; always re-check; on "no claim" write `false` and
  redirect as today. Errors keep the gate's existing fail-open.
- **Rationale**: FR-005; the gate decides navigation, not permission.

## R6 — Edits on one page, read on another

- **Decision**: pages that save the world or members call `remember` with
  the server's answer (or `refresh` the key when the mutation returns
  less).
- **Rationale**: FR-004; otherwise the next page shows the pre-edit
  record until its background read lands.
