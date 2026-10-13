/**
 * Spec 091: the room, as engine commands.
 *
 * The adapter between the room and the engine's command channel. Each
 * command's shape is the one `parse_command` reads
 * (`crates/thunderforge-engine/src/sdk.rs`). The setup's order is the
 * plan's, and a test pins it.
 *
 * `set_viewer_token` is a direct export, not a command, so it is not here.
 * The host calls it after the hero is controlled, and with "" on teardown.
 */

import type { Light } from "./map.ts";
import type { RoomScript, SquareRect } from "./script.ts";
import type { DoorState, Point, Wall } from "./walls.ts";

/** The hero's token id, whichever hero is played. */
export const HERO_ID = "hero";

/** The hero picked: its name on the board and its art. */
export interface Hero {
  name: string;
  art: string;
}

/** `payloads.rs` `WorldTokenPayload`, the fields the room sends. */
export interface TokenPayload {
  id: string;
  x: number;
  y: number;
  z: number;
  label: string;
  tokenType: "character" | "object";
  photoUrl: string;
}

/** `RegionGeometry::Rect` (canvas-core `interaction.rs`), in world units. */
export interface RectGeometry {
  shape: "rect";
  x: number;
  y: number;
  width: number;
  height: number;
}

export type EngineCommand =
  | { type: "set_reduced_motion"; reduced: boolean }
  | {
      type: "set_scene_grid";
      gridType: "square";
      size: number;
      mapWidth: number;
      mapHeight: number;
      visible: boolean;
    }
  | {
      type: "set_scene_background";
      backgroundImagePath: string | null;
      width: number;
      height: number;
    }
  | { type: "upsert_wall"; wall: Wall }
  | { type: "remove_wall"; wallId: string }
  | { type: "upsert_light"; light: Light }
  | { type: "remove_light"; lightId: string }
  | { type: "set_ambient_light"; level: "dark" }
  | { type: "upsert_token"; token: TokenPayload }
  | { type: "remove_token"; tokenId: string }
  | {
      type: "upsert_interactive";
      interactive: {
        id: string;
        subjectKind: "region";
        geometry: RectGeometry;
        trigger: "enter";
        fireMode: "once";
      };
    }
  | { type: "remove_interactive"; interactiveId: string }
  | { type: "set_carried_light"; tokenId: string; bright: number; dim: number }
  | { type: "set_token_vision"; tokenId: string; darkvision: number }
  | { type: "set_controlled_token"; tokenId: string | null }
  | { type: "set_scene_playing"; playing: boolean }
  | {
      type: "fit_camera_to";
      centerX: number;
      centerY: number;
      width: number;
      height: number;
    };

const HERO_Z = 2;
const CHEST_Z = 1;

function heroToken(hero: Hero, square: Point, grid: number): TokenPayload {
  return {
    id: HERO_ID,
    x: square.x * grid,
    y: square.y * grid,
    z: HERO_Z,
    label: hero.name,
    tokenType: "character",
    photoUrl: hero.art,
  };
}

const rect = (r: SquareRect, grid: number): RectGeometry => ({
  shape: "rect",
  x: r.x * grid,
  y: r.y * grid,
  width: r.width * grid,
  height: r.height * grid,
});

/**
 * Everything the room sends to take the table, in order:
 * 1. reduced motion, so nothing moves before it is known;
 * 2. the grid, anchored by the map's size as `apps/web` sends it
 *    (`useSceneContent.ts`);
 * 3. the background;
 * 4. the walls, doors shut, and 5. the lights;
 * 6. the dark;
 * 7. the chest and 8. its region;
 * 9. the hero's torch, 10. the hero, and 11. its vision, which needs the
 *    token first (`app.rs`, `SetTokenVision`);
 * 12. the hero under the player's hand;
 * 13. the scene playing, which arms the region (`interaction.rs`
 *     `detect_entries`);
 * 14. the camera on the room.
 *
 * `fit_camera_to` has no `immediate` field (`sdk.rs`), so the camera eases
 * whatever `reducedMotion` says.
 */
export function roomSetup(
  script: RoomScript,
  hero: Hero,
  { reducedMotion }: { reducedMotion: boolean },
): EngineCommand[] {
  const { map, chest, scene } = script;
  const grid = map.gridSize;
  const camera = rect(scene.camera, grid);
  return [
    { type: "set_reduced_motion", reduced: reducedMotion },
    {
      type: "set_scene_grid",
      gridType: "square",
      size: grid,
      mapWidth: map.width,
      mapHeight: map.height,
      visible: true,
    },
    {
      type: "set_scene_background",
      backgroundImagePath: scene.background,
      width: map.width,
      height: map.height,
    },
    ...map.walls.map(
      (wall): EngineCommand => ({ type: "upsert_wall", wall: { ...wall } }),
    ),
    ...map.lights.map(
      (light): EngineCommand => ({ type: "upsert_light", light: { ...light } }),
    ),
    { type: "set_ambient_light", level: "dark" },
    {
      type: "upsert_token",
      token: {
        id: chest.tokenId,
        x: chest.square.x * grid,
        y: chest.square.y * grid,
        z: CHEST_Z,
        label: "Chest",
        tokenType: "object",
        photoUrl: chest.art,
      },
    },
    {
      type: "upsert_interactive",
      interactive: {
        id: chest.interactiveId,
        subjectKind: "region",
        geometry: rect(chest.region, grid),
        trigger: "enter",
        fireMode: "once",
      },
    },
    {
      type: "set_carried_light",
      tokenId: HERO_ID,
      bright: scene.torch.bright * grid,
      dim: scene.torch.dim * grid,
    },
    { type: "upsert_token", token: heroToken(hero, script.start, grid) },
    { type: "set_token_vision", tokenId: HERO_ID, darkvision: 0 },
    { type: "set_controlled_token", tokenId: HERO_ID },
    { type: "set_scene_playing", playing: true },
    {
      type: "fit_camera_to",
      centerX: camera.x + camera.width / 2,
      centerY: camera.y + camera.height / 2,
      width: camera.width,
      height: camera.height,
    },
  ];
}

/**
 * Everything the room sends to give the table back. It removes all the
 * setup added. The torch goes by `set_carried_light` with no reach, which
 * takes a carried light away (canvas-core `lighting.rs` `LightSource::carried`).
 * A cleared background still needs a width and a height (`sdk.rs`).
 */
export function roomTeardown(script: RoomScript): EngineCommand[] {
  return [
    { type: "set_scene_playing", playing: false },
    { type: "set_controlled_token", tokenId: null },
    { type: "set_carried_light", tokenId: HERO_ID, bright: 0, dim: 0 },
    { type: "remove_token", tokenId: HERO_ID },
    { type: "remove_token", tokenId: script.chest.tokenId },
    { type: "remove_interactive", interactiveId: script.chest.interactiveId },
    ...script.map.walls.map(
      (wall): EngineCommand => ({ type: "remove_wall", wallId: wall.id }),
    ),
    ...script.map.lights.map(
      (light): EngineCommand => ({ type: "remove_light", lightId: light.id }),
    ),
    {
      type: "set_scene_background",
      backgroundImagePath: null,
      width: 0,
      height: 0,
    },
  ];
}

/** The `upsert_wall` that opens or shuts a door. */
export function doorCommand(wall: Wall, state: DoorState): EngineCommand {
  return { type: "upsert_wall", wall: { ...wall, doorState: state } };
}

/**
 * The `upsert_token` that sends the hero back to a square, with its whole
 * token, so nothing else about it changes.
 */
export function rollbackCommand(
  hero: Hero,
  square: Point,
  gridSize: number,
): EngineCommand {
  return { type: "upsert_token", token: heroToken(hero, square, gridSize) };
}
