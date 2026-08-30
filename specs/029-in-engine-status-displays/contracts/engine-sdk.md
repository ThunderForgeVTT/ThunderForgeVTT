# Contract: Engine UI SDK (TypeScript)

The typed boundary the application uses to drive in-engine presentation.
Replaces hand-built JSON passed to `apply_world_command`.

Types are **generated** from `thunderforge-canvas-core` (see
[research.md §1–3](../research.md)); the shapes below are the contract those
types must express, not a second hand-maintained copy.

---

## Why this exists

Today: `apply_world_command(jsonString)`. Every shape crossing it is mirrored
by hand in TypeScript, and the two drift. A drifted field fails **silently** —
the engine deserializes what it can and ignores the rest — so the symptom is a
display that does not appear, with no error anywhere. This feature adds a
large amount of new surface, and adding it to that boundary would multiply the
failure rather than contain it.

---

## Envelope

Every command carries the SDK version. The engine rejects a mismatch outright
and never partially applies (FR-019).

```ts
interface EngineCommand<T> {
  sdkVersion: number;
  command: T;
}
```

A single integer, deliberately: both sides ship in one bundle, so there is no
independent release cadence for semantic versioning to describe. The version
exists to catch a stale bundle, which is precisely the case the current
boundary fails silently on.

---

## Commands

### `setResourceDefinitions`

Declares what the active game system tracks. Sent once per system load.

```ts
type ResourceKind = "bar" | "counter";

interface ResourceDefinition {
  id: string;
  label: string;
  kind: ResourceKind;
  order: number;
  allowStacking: boolean;
}

interface SetResourceDefinitions {
  type: "setResourceDefinitions";
  definitions: ResourceDefinition[];
}
```

**Rejections** (reported via the event callback, never silent — FR-020):

- duplicate `id`
- `kind: "counter"` with `allowStacking: true`

### `setTokenStatus`

The resolved, already-entitlement-filtered status for one token. The engine
does not compute disclosure; it draws what it is given.

```ts
type DisclosureState = "visible" | "greyed" | "percentage" | "chunked";

// Generated. `Option<T>` becomes `T | null` — present and explicitly null,
// not omittable. Verified against ts-rs 12.0.1 output (research §2).
interface ResourceEntry {
  current: number;
  max: number | null;
  label: string | null;
}

type ResolvedResource =
  | { definitionId: string; disclosure: "visible"; entries: ResourceEntry[] }
  | { definitionId: string; disclosure: "greyed" }
  | { definitionId: string; disclosure: "percentage"; proportion: number }
  | { definitionId: string; disclosure: "chunked"; quarter: 0 | 1 | 2 | 3 | 4 };

interface SetTokenStatus {
  type: "setTokenStatus";
  tokenId: string;
  resources: ResolvedResource[];
}
```

**The union is the contract.** `ResolvedResource` is discriminated on
`disclosure` so a payload that carries `entries` alongside `"chunked"` does
not type-check. Over-disclosure becomes unrepresentable rather than forbidden
by a rule somebody has to remember — which is the whole reason for generating
a discriminated union rather than a JSON-Schema-derived shape that narrows
poorly.

**Rejections**: unknown `definitionId`; `current` outside `0..=max`
(FR-002d — a defect to report, not a state to render); more than one entry
where the definition forbids stacking.

### `clearTokenStatus`

```ts
interface ClearTokenStatus {
  type: "clearTokenStatus";
  tokenId: string;
}
```

Removes all status furniture for a token. Distinct from an empty `resources`
list only in intent; both draw nothing (FR-007).

### `setDisplayAppearance`

```ts
interface DisplayAppearance {
  barHeight: number;
  barWidth: number;
  offsetAboveToken: number;
  gapBetweenBars: number;
  fills: Record<string, [number, number, number]>; // definitionId → linear sRGB
  undisclosedFill: [number, number, number];
  background: [number, number, number];
}

interface SetDisplayAppearance {
  type: "setDisplayAppearance";
  appearance: Partial<DisplayAppearance>;
}
```

Supplied by the application (FR-022) so a later theming feature has something
to configure. Omitted fields keep the documented defaults, which live in
exactly one place (FR-023).

---

## Read surface

```ts
/** What the engine would draw for a token, for tests and the React panel. */
function getTokenStatus(tokenId: string): TokenStatus | null;

/** Every token currently carrying status furniture. */
function listTokenStatus(): TokenStatus[];
```

This is FR-021's testing surface and is also how the React corner panel
observes state without becoming a second source of truth (Constitution I). A
test can assert what _would_ be drawn without rendering a pixel — which
matters because the engine's own tests never execute.

---

## Errors

Reported through the existing `set_event_callback` channel. Silent discard is
not acceptable (FR-020).

```ts
interface EngineSdkError {
  type: "sdkError";
  code:
    | "versionMismatch"
    | "unknownDefinition"
    | "duplicateDefinition"
    | "stackingNotAllowed"
    | "valueOutOfRange"
    | "malformed";
  message: string;
  command?: string;
}
```

`versionMismatch` is fatal for that command and applies nothing. The others
reject the offending command and leave prior state intact — a bad update must
not blank a display that was previously correct.

---

## What this contract deliberately excludes

- **Mutation.** The SDK displays; it does not change resource values. Editing
  goes through the existing GraphQL mutations, server-authoritative
  (Principle III).
- **Disclosure decisions.** The engine receives resolved status. It cannot
  coarsen, because coarsening on the client means the exact value was on the
  client.
- **Theming UI**, user-authored themes, per-world palettes.
- **Arbitrary widgets** or scripting inside the engine.
