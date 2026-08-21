# Contract: Scene-Switch Loading/Error State (client-side, no server API change)

This is not a server contract change — no new endpoint, query, or mutation. It documents the client-side state contract `WorldPage.tsx` (or an extracted hook) must expose, since User Story 4 has no GraphQL surface of its own.

## State shape

```ts
type SceneLoadState =
  | { status: "loading"; sceneId: string }
  | { status: "ready"; sceneId: string }
  | { status: "error"; sceneId: string; failedResource: "background" | "walls" | "lights" | "tokens"; retry: () => void };
```

- Transitions to `loading` immediately when `sceneId` changes (SceneSwitcher selection).
- Transitions to `ready` only once background image, walls, lights, and tokens have all loaded successfully for the current `sceneId`.
- Transitions to `error` if any one of them fails — background image failure takes priority in the reported `failedResource` if multiple fail simultaneously, since it's the most visually disruptive (spec.md Acceptance Scenario emphasizes background image as the primary example).
- `retry()` re-invokes the same loaders for the same `sceneId` without changing `sceneId` — re-entering `loading`, then `ready` or `error` again.
- If `sceneId` changes again while in `loading` or `error` state (GM switches scenes again before the prior switch resolved), the state immediately reflects the new `sceneId` — the prior in-flight load's eventual resolution is discarded/ignored (edge case already documented in spec.md).

## Verification

- Manual/Playwright: select a scene, confirm a loading indicator renders immediately and clears once fully rendered (FR-011/FR-012).
- Manual/Playwright: simulate a background-asset fetch failure (e.g. point at a non-existent asset), confirm the error state renders with a retry action, and confirm clicking retry re-attempts and can succeed once the underlying resource is fixed (FR-013/FR-013a).
- Manual/Playwright: a connected player's client shows the same loading/error/ready sequence as the GM's, without a manual reload (Acceptance Scenario 3).
