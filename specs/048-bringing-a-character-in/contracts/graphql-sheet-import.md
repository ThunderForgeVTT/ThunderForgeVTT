# Contract: GraphQL operations for bringing a character in

These operations live in three new files, each with its own `_tests.rs`:

- `crates/thunderforge-server/src/graphql/mutations_sheet_import.rs` holds
  `SheetImportMutation`;
- `mutations_staged_content.rs` holds `StagedContentMutation`;
- `queries/sheet_import.rs` holds `SheetImportQuery`.

All three are merged into `QueryRoot` and `MutationRoot` in `graphql.rs`.
Each resolver calls an `_impl(state, user_id, is_admin, …)`, following the
pattern of the other areas. `node scripts/check-graphql-contract.mjs --schema --fix`
regenerates the checked-in schema.

The server re-checks every rule in this contract. The client's flags and
hidden controls are only courtesy.

## Common errors (`extensions.code`)

| Code | When |
| --- | --- |
| `FEATURE_DISABLED` | `feature.sheet_import` is off (import and preview only; decisions and downloads stay available) |
| `FORBIDDEN` | the caller fails the rule named on the operation |
| `SHEET_ENCRYPTED` | `PdfError::Encrypted` |
| `SHEET_TOO_LARGE` | over the byte bound, before parsing |
| `SHEET_TOO_MANY_PAGES` | over the page bound |
| `SHEET_UNREADABLE` | `PdfError::Unreadable` or `Page` |
| `SHEET_NOT_RECOGNISED` | no reader of this system recognises the document ("this is not a D&D Beyond sheet") |
| `SYSTEM_HAS_NO_MAPPING` | the actor's system declares no `sheetImport` (FR-012) |
| `PLAN_CHANGED` | the server's plan hash differs from the reviewed one; "review again" |
| `CONTENT_NOT_ADOPTED` | a play mutation named staged content (FR-036a) |

## Types

```graphql
enum FieldCertainty { READ UNCERTAIN UNREAD CORRECTED }
enum StagedState { PENDING ADOPTED DECLINED }
enum ContentResolution { WORLD STAGED_EXISTING STAGED_NEW DIFFERS }

type SheetSource { page: Int!  x0: Float!  y0: Float!  x1: Float!  y1: Float!  text: String! }

type SheetFieldChange {
  path: String!            # neutral path, e.g. "abilities.str"
  target: String!          # "ability_data.strength"
  old: JSON
  new: JSON
  certainty: FieldCertainty!
  reason: String           # why uncertain/unread; both numbers on a cross-check
  source: SheetSource
  playState: Boolean!      # re-import: not written unless `overwritePlayState` names it
}

type SheetContentChange {
  kind: String!            # "spell", "feat", "feature", "species_trait", "item"
  name: String!
  resolution: ContentResolution!
  worldId: ID              # WORLD: the world's own ability/item
  stagedId: ID             # STAGED_EXISTING / DIFFERS
  removed: Boolean!        # re-import: on the actor now, not on the sheet
}

type SheetUnmapped { path: String!  value: JSON!  goesTo: String! }

type SheetImportPlan {
  readerId: String!
  readerVersion: String!
  fields: [SheetFieldChange!]!
  content: [SheetContentChange!]!
  unmapped: [SheetUnmapped!]!
  isReimport: Boolean!
  planHash: String!
}

type ActorImportRecord {
  id: ID!
  kind: String!            # "import" | "rollback"
  appliedAt: DateTime!
  appliedBy: UserSummary!
  versionNo: Int           # null for a rollback
  restoredFrom: ID
  correctedFields: [String!]!   # paths the importer corrected (FR-022, visible to the GM)
  fileAvailable: Boolean!       # true only for the owner and the GM (FR-043b)
}

type StagedContent {
  id: ID!
  kind: String!
  name: String!
  fieldValues: JSON!
  state: StagedState!
  broughtBy: UserSummary!
  actors: [ActorSummary!]!      # every actor linking it
  differsFrom: ID
  decidedBy: UserSummary
  decidedAt: DateTime
}
```

`JSON` and `DateTime` are the scalars the schema already uses.

## Queries

### `sheetImportPreview(actorId: ID!, reading: JSON!, corrections: JSON): SheetImportPlan!`

- **Rule**: Editor or Owner on the actor, or the world's GM. The flag must
  be on.
- `reading` is the browser's `ImportedCharacter`. The server
  deserialises it strictly, so an unknown field is an error, and bounds every
  string and list.
- The server maps the reading through the actor's system's declaration,
  resolves content against the world and the staged table, and diffs it
  against the actor.
- **Writes nothing.** No file is uploaded.
- The `planHash` it returns is what `applySheetImport` must match.

### `actorImports(actorId: ID!): [ActorImportRecord!]!`

- **Rule**: Viewer or above on the actor sees the list. Only the owner and
  the GM see `fileAvailable: true`.

### `stagedContent(worldId: ID!, state: StagedState, playerId: ID): [StagedContent!]!`

- **Rule**: the GM or a Trusted Player of the world sees everything.
- A Player sees only what they brought, so they can see "awaiting the GM".
- The GM screen groups the results by `broughtBy`. That grouping is "the
  compendium named for the player" (FR-030a).

## Mutations

### `applySheetImport(actorId: ID!, file: Upload!, corrections: JSON, overwritePlayState: [String!], planHash: String!): ActorImportRecord!`

GraphQL multipart, through `postGraphQLMultipart`. The steps run in this
order:

1. Check the flag, then the rule (as for the preview), then the byte bound
   against the upload's length before reading it.
2. Read the file natively with the actor's system's readers. Apply the page
   bound, the encryption check and recognition.
3. Apply the corrections, then map and plan. Compare the hash with
   `planHash` and refuse with `PLAN_CHANGED` if it differs.
4. Write the object `sheets/{owner}/{character}/{version}.pdf`.
5. In one transaction:
   - create or reuse `brought_characters`;
   - insert `sheet_import_versions`;
   - write the `actor_imports` row with its before-snapshot;
   - write the sheet fields through the same validators as
     `updateActorSystemData`;
   - write the links: world content as `ability_id`/`item_id`, and staged
     content as `staged_id`, upserting `world_staged_content` by its unique
     key;
   - set `world_actors.origin = 'Uploaded'`;
   - record world event 40.
6. If the transaction fails, delete the object and return the error.

Play-state fields (`playState` in the mapping) keep their current value
unless `overwritePlayState` names them (FR-051).

On a first import, the before-snapshot is the actor as it was, which is what
a rollback to "before any import" restores.

### `rollBackActor(actorId: ID!, toImportId: ID!): ActorImportRecord!`

- **Rule**: the world's GM only (FR-044b). Trusted Players are refused.
- Restores the sheet fields and links from `toImportId`'s before-snapshot.
  It keeps the play-state fields as they are now (FR-044a).
- Writes an `actor_imports` row of kind `rollback` with `restored_from`, and
  records event 42.
- The actor's origin stays `Uploaded`.

### `adoptStagedContent(id: ID!): StagedContent!`

- **Rule**: the GM or a Trusted Player (`require_manages_content`). A
  Player is refused (FR-033b).
- In one transaction:
  - insert a `world_abilities` or `world_items` row with
    `origin = 'Uploaded'`, the staged fields and `created_by` = the player
    who brought it;
  - repoint every link with that `staged_id`;
  - set the state to `adopted`, with `decided_by` and `decided_at`;
  - record event 41.

### `adoptAllStagedContent(worldId: ID!, playerId: ID!): [StagedContent!]!`

- The same rule as `adoptStagedContent`.
- Adopts that player's pending pieces as they are at this moment. Pieces
  staged afterwards stay pending (FR-033a, "a snapshot"). One transaction,
  one event 41 per piece.

### `declineStagedContent(id: ID!): StagedContent!` and `revisitStagedContent(id: ID!, state: StagedState!): StagedContent!`

- The same rule as `adoptStagedContent`.
- Decline sets the state to `declined`. Revisit moves a declined piece to
  `pending` or straight to `adopted` (FR-036b).
- Each records event 41. That event is the withdrawing event R13's
  staleness check compares against.

## Refusal in play paths (FR-036a, FR-037a, FR-038)

Every mutation that takes an actor's link id checks whether the link carries
a `staged_id`. These include `rollCheck`, `makeAttack`, ability use, item
use and share. If it does, the mutation:

1. refuses with `CONTENT_NOT_ADOPTED`;
2. reads `x-tf-last-event` and compares it with the piece's latest event 41;
3. if the header is missing or older than that event, increments
   `thunderforge.unadopted_use_attempts{result="suppressed_stale"}` and
   reports nothing (FR-038b);
4. otherwise upserts `world_unadopted_use_attempts` within its 10-minute
   window. On the first attempt in the window, it posts a GM-only chat
   message that states only facts:
   "<player> tried to use <piece> on <character> at <time>. It came in with
   the character and has not been adopted."

## File download (REST, not GraphQL)

### `GET /api/sheet-imports/{versionId}/file`

- **Rule**: the character's owner, or the GM of a world with an
  `actor_imports` row for this version. Anyone else gets 404, so the route
  does not reveal that the file exists.
- Returns `200 application/pdf` with `Content-Disposition: attachment`, and
  `Cache-Control: private, no-store`.
- It does not check the flag (R15).

## Web client

- `apps/web/src/api/sheetImport.ts` holds the typed calls.
  `applySheetImport` goes through `postGraphQLMultipart`
  (`api/graphqlClient.ts:341`) with upload progress.
- `graphqlClient.ts` adds `x-tf-last-event` from the world store's last
  applied event id to every request made inside a world.
- Components never import the client directly. The import route and the
  staged-content screen call hooks that expose `refetch()`, as AGENTS.md
  asks for reads the store does not hold. The actor itself returns through
  event 40's sync.
