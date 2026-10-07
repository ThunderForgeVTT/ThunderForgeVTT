# Contract: World-Store Commands and Bridges

These are web-only. **No GraphQL is added or changed.** The one mutation
this spec newly reaches from the store is the existing:

```graphql
setDoorSecret(wallId: UUID!, secret: Boolean!): Boolean!   # DM of the scene; refused while paused
```

It applies to any wall (research R1). It is already in
`play_pause_surface_tables.rs`.

## Commands (`apps/web/src/engine/world/types.ts`)

```ts
export type GroupStamp = { id: string; size: number };

export type SetWallsHiddenCommand = {
  type: "set_walls_hidden";
  wallIds: string[];
  hidden: boolean;
};

export type SelectGroupCommand = {
  type: "select_group";
  tokenIds: string[];
  wallIds: string[];
  lightIds: string[];
  shapeIds: string[];
};

/** The Select bar's Delete: forwarded to the engine, which emits the deletes. */
export type DeleteGroupCommand = { type: "delete_group" };
```

These existing commands gain `group?: GroupStamp`:

- `upsert_token`;
- `update_wall` and `delete_wall`;
- `update_light` and `delete_light`;
- `update_shape` and `delete_shape`.

## Store (`apps/web/src/engine/world/store.ts`)

| Command                                         | State effect                                                                                                         |
| ----------------------------------------------- | -------------------------------------------------------------------------------------------------------------------- |
| `set_walls_hidden`                              | `walls[id].secret = hidden` for each id it holds (optimistic)                                                        |
| `select_group`                                  | `selectedTokenIds`, `selectedWallIds`, `selectedLightIds`, `selectedShapeIds`, and each primary = first id or `null` |
| `select_wall` / `select_light` / `select_shape` | Unchanged primary; the kind's list = `[id]` or `[]`                                                                  |
| `remove_wall` / `remove_light` / `remove_shape` | Also drops the id from the kind's list                                                                               |
| `delete_group`                                  | No state change. The engine bridge forwards it as `DeleteSelection`.                                                 |

## Bridges (`apps/web/src/engine/world/sync/`)

| Bridge      | New behaviour                                                                                                                                                                                                                          |
| ----------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `walls.ts`  | `set_walls_hidden` → `setDoorSecret(id, hidden)` per wall, in that wall's turn. On failure, restore the cached record (`"sync"`). `update_wall` / `delete_wall` failures restore the cached record too. Each answer settles its group. |
| `lights.ts` | `update_light` / `delete_light` failures restore the cached record. Each answer settles its group.                                                                                                                                     |
| `shapes.ts` | Each answer settles its group. The rollback is spec 082 T018's.                                                                                                                                                                        |
| `tokens.ts` | Each answer settles its group. `applyMoveRefusal` still re-reads, but shows no toast of its own when the command carried a group.                                                                                                      |

## Tally (`apps/web/src/engine/world/sync/groupMoves.ts`)

```ts
export function settleGroup(
  stamp: GroupStamp | undefined,
  ok: boolean,
  verb: "moved" | "deleted" | "hidden",
): void;
```

- **When it reports.** It opens a stamp's tally on first sight. Once
  `size` answers have arrived, with `refused > 0`, it shows one toast:
  `"${refused} of ${size} could not be ${verb}."` Then it forgets the
  stamp.
- **Without a stamp.** `set_walls_hidden` carries no stamp from the
  engine, so the walls bridge makes one, sized `wallIds.length`.
- **Stuck tallies.** A stamp left unanswered for 30 s is dropped without
  a toast, so it cannot leak.
