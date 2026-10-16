# Contract: world chrome hooks

What pages and hooks may call. All throw outside a `WorldChromeProvider`.

## `useWorldChrome(worldId, key, load): WorldChrome[key] | undefined`

Spec 100's hook, unchanged in shape. Returns the known value at once,
`undefined` before the first answer, and calls `load` on mount and on
`worldId` change (joined with any read already in flight for the key).
A rejected `load` keeps the known value. `load` must be stable.

## `useRememberWorldChrome(): (worldId, patch) => void`

Writes values learned another way: live events, a mutation's answer.

## `useKnownWorld(worldId): { world, known }` (new)

`world: WorldRecord | null` — `null` before the first answer and for a
missing or refused world. `known` is false only before the first answer,
which is the only time a page shows its full-screen loader. Applies R3.

## `useWorldMembers(worldId)` / `useWorldRole(worldId, world)`

Unchanged results. `loading` is true only while no member list is known
for the world.

## `useActorClaimGate(worldId, world)`

Unchanged result. `cleared` is true at once when the world's gate cleared
earlier in this tab; the re-check still runs and may redirect.

## `useWorldPlayState(worldId)`

Unchanged result, from the store; the focus refresh writes the store.
