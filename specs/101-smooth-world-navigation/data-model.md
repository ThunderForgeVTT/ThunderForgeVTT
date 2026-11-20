# Data Model: Smooth World Navigation

All in memory, per tab, per signed-in user.

## WorldChrome (per world id)

| Field | Type | Source | Written by |
|---|---|---|---|
| `resumeScene` | `string \| null` | `getResumeSceneName` | shell (spec 100) |
| `interfacePackId` | `string \| null` | `getWorld` | `WorldAppearance`, appearance events (spec 100) |
| `world` | `WorldRecord \| null` | `getWorld` | route pages; overview and settings edits |
| `members` | `WorldMemberDoc[]` | `getWorldMembers` | `useWorldMembers` fetch and `refetch` |
| `claimCleared` | `boolean` | gate checks | `useActorClaimGate` |
| `playState` | `WorldPlayState \| null` | `getWorldPlayState` | `useWorldPlayState`, focus refresh |

Every field is optional: absent means "not known yet" and the page
behaves as before this spec. `null` is a known answer.

## Store

- `chrome: Record<worldId, Partial<WorldChrome>>`
- `inFlight: Map<"worldId:key", Promise>` (not state; a ref)
- `owner: userId | null` — the store is empty whenever it changes.

## Rules

- A patch equal to what is known changes nothing (no re-render).
- `members` is replaced whole, never merged.
- A refusal of `world` (R3) writes `world: null`; it does not clear the
  other fields, which the next reads correct.
