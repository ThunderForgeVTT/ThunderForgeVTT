# Contracts: Token Movement and Vision

Three surfaces change: the GraphQL API, the engine's command SDK, and the
system-pack manifest. Each is stated as what callers may rely on.

## 1. GraphQL

### `moveOwnToken` (changed, backward compatible)

```graphql
moveOwnToken(
  tokenId: UUID!
  x: Float!
  y: Float!
  path: [PointInput!]        # new, optional
): Token!
```

- `path` is the ordered points the token passed through, excluding where it
  started. Absent means a straight line from the token's stored position.
- The server judges the path against the scene's walls. A crossing of a wall
  with `blocksMovement`, or of a closed door, is **refused**: the mutation
  errors, nothing is written, and the message names a wall ("A wall is in the
  way").
- A path longer than 64 segments is refused as malformed.
- Ownership rules are unchanged.

### `updateToken` (unchanged for a Game Master)

A Game Master's move is not judged (owner decision 1). No signature change.

### Scene exploration (new)

```graphql
setSceneExploration(sceneId: UUID!, enabled: Boolean!): Scene!
resetSceneExploration(sceneId: UUID!, userId: UUID): Scene!
```

- Both are Game-Master-only.
- `resetSceneExploration` with no `userId` resets for everyone (bumps the
  scene's epoch); with one, it resets that player only.
- `Scene` gains `explorationEnabled: Boolean!` and `explorationEpoch: Int!`,
  and, for the asking player, `myExplorationEpoch: Int!` — the greater of the
  scene's epoch and their own reset.

### Door changes (behaviour, no signature change)

`setDoorDesignation`, `setDoorLock`, `setDoorSecret` and an
`activateInteractive` that changes a door MUST record a **wall changed** event
(code 10) in addition to the door-changed event (21) they record today. Clients
re-read a scene's walls on code 10; this is what makes a door reach every
board.

## 2. Engine SDK (`external_command`)

### `set_controlled_token` (new)

```json
{ "type": "set_controlled_token", "tokenId": "<uuid>" | null }
```

The token the local player may move with the keyboard. `null` means none — a
Game Master, or a player with no token. The engine tags exactly one entity
`PlayerControlled`. Distinct from `set_viewer_token`, which is whose eyes the
board is drawn through.

### `set_token_vision` (exists; now used by the product)

```json
{ "type": "set_token_vision", "tokenId": "<uuid>",
  "darkvision": 0.0, "facing": null, "fov": 6.283, "maxRange": null }
```

Already implemented and already inserting a `TokenVision`. The web begins
calling it with what the server resolved from the pack's declaration.

### Exploration (new)

```json
{ "type": "set_exploration", "enabled": true, "epoch": 3,
  "cells": "<packed>" }        // what this player had already explored
```

```json
{ "type": "clear_exploration" }
```

The engine accumulates what the viewer's token can see while exploration is
enabled, draws the remembered area into `CanvasLayer::Fog`, and exposes what it
has accumulated for the web to persist:

```ts
explored_cells(): string   // packed, for storage
```

The engine never stores anything itself and never talks to the server.

## 3. System pack manifest (new block)

A system declares where a creature's sight comes from. Shape follows the
manifest's existing style (compare Genie's `sizeCategories`):

```json
{
  "vision": {
    "darkvision": { "source": "trait_data", "field": "darkvision_ft", "unit": "feet" },
    "carriedLight": {
      "bright": { "source": "trait_data", "field": "light_bright_ft", "unit": "feet" },
      "dim":    { "source": "trait_data", "field": "light_dim_ft",    "unit": "feet" }
    }
  }
}
```

- The block is optional. A system that declares none gets the default profile:
  ordinary sight, no darkvision.
- Distances are in the system's own units and are converted through the scene's
  grid (one cell is one of the system's squares).
- D&D 5e declares darkvision (owner decision 2). Genie declares its own, or
  nothing, and its tokens keep ordinary sight until it does.
- This amends the manifest contract of ADR-027, the way spec 016's `legal`
  block did, and is documented in `packs/systems/README.md`.

## 4. What callers may rely on

- A refused move leaves the token where it was **on every client**, and the
  player who attempted it is told why.
- A door change reaches every client within a second, without a reload, for
  sight, for light and for passage.
- A vision profile reaching the engine changes what that client hides, and
  nothing else.
- Exploration is per browser. Nothing a player explores is readable by the
  server, by another player, or by the same player in another browser.
