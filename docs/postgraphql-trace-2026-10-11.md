# Who calls `postGraphQL`, traced 2026-10-11

The knowledge graph (graphify, 2026-10-11) ranked `postGraphQL()` the
strongest cross-community bridge in the web client: one function linking
about 35 communities, admin, PlayDock, lore, sync and the SDK panels among
them. AGENTS.md says components never touch the network, so the question was
whether that bridge is the transport doing its job or components going
around the world store. Traced on `main` at `77f22dac`.

## Most of the bridge is by design

`apps/web/src/api/graphqlClient.ts` is the one transport; every `api/*.ts`
module wraps it, and those wrappers are what the graph's bridge mostly is.
80 non-test files reference it. 19 of them are outside `api/`:

| Kind | Files | Verdict |
| --- | --- | --- |
| Sync and engine host | `engine/world/sync/{tokenGrid,tokenVision,exploration,heartbeat,subscriptionClient}.ts`, `engine/bevy/index.ts` (peer signalling) | Fine: this is where the network belongs. |
| Plain modules | `pages/library/library.ts`, `pages/world/compendium/{worldBooks,syncBack}.ts`, `pages/world/scenes/bringParty.ts`, `services/instanceSetup.ts`, `legal/publishedOperatorValues.ts`, `abilities/vocabulary.ts`, `host/index.ts` (SDK re-export) | Fine: reads and writes the store does not hold, outside components. |
| Components | `pages/world/ability/{AttackFieldsEditor,MultiattackPicker}.tsx`, `pages/admin/components/GitHubAppsPanel.tsx`, `pages/world/JoinWorldPage.tsx`, `components/world/SessionSetupInviteLink.tsx` | Allowed by the rule (data the store does not hold: ability fields, GitHub apps, invites, joining), but the query lives inline in the component rather than in an `api/` module or a hook with `refetch()`. Style, not correctness. |

## The real finding: tokens are read and written around the store

Tokens are a store-held kind, so components should read them from the store
and dispatch commands into it. Six components do neither:

**Reads into `useState`** with `getTokens(sceneId)`. Each is a snapshot taken
once per scene; it does not follow `worldEventsCreated`, so it goes stale the
moment anyone moves, adds or removes a token:

- `components/TokenPanel.tsx:192`
- `components/world/CanvasContextMenu/CanvasContextMenu.tsx:263`
- `components/world/PlayDock/ActorsPanel.tsx:188`
- `components/world/PlayDock/CombatPanel.tsx:122`
- `components/world/PlayDock/InPaneCharacterSheet.tsx:97`

**Writes as direct mutations**, with no optimistic update and no rollback.
The server's event still brings the store into line, so the board is right;
what each component holds locally is not:

- `createToken`: `TokenPanel.tsx:208`, `CanvasContextMenu.tsx:867`, `pages/world/usePlaceActorTokens.ts:34`
- `deleteToken`: `TokenPanel.tsx:236`, `CanvasContextMenu.tsx:676`
- `updateToken`: `TokenPanel.tsx:299`

Narrower token setters (`setTokenLink`, `setOwnPrimaryTokenPhoto`,
`setTokenDisclosure`, `setTokenNameVisibility`) go straight to the server
too. They change fields the store does not model, so they are the allowed
kind until it does.

## Why: the store's token is missing three fields

The event sync already fetches full `TokenRecord`s (`getTokens`) on every
token event. `tokenRecordToWorldToken` (`engine/world/sync/tokens.ts:74`)
then keeps the drawing fields and drops `actorId`, `linked` and `levelId`.
Every component above needs at least one of the three: which character a
token is, whether it is linked, which level it stands on. So each fetches
its own copy.

## The fix this points to

1. Carry `actorId`, `linked` and `levelId` on `WorldToken` and fill them in
   `tokenRecordToWorldToken`. No server change: the data is already fetched.
2. Move the five readers to the store (a `useWorldTokens(sceneId)`
   selector), so they follow every token event.
3. Route create and delete through `upsert_token`/`remove_token`, which the
   token mutation bridge already turns into `createToken`/`deleteToken`, so
   they gain the optimistic path and rollback.

Steps 1 and 2 are small and independent of each other per component. Step 3
touches the bridge's engine-id map and wants its own e2e proof (the
`actors`, `combat` and `tokens` slices).
