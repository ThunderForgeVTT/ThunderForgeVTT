import {
  chestCommand,
  doorCommand,
  rollbackCommand,
  roomSetup,
  type Judged,
  type Point,
  type RoomScript,
  type RoomState,
} from "@thunderforge/encounter";
import { PARTY, type PartyHero } from "./party.ts";

/** The engine's camera, as `camera_state()` reports it. */
export interface Camera {
  x: number;
  y: number;
  scale: number;
}

/** The board the engine draws, in its own pixels (`engine-frame.css`). */
export const BOARD = { w: 960, h: 540 };

/** Where on the board a square is: its centre, and a square's side. */
export function squareOnBoard(script: RoomScript, { x, y }: Point, cam: Camera): { x: number; y: number; size: number } {
  const grid = script.map.gridSize;
  return {
    x: BOARD.w / 2 + (x * grid - cam.x) / cam.scale,
    y: BOARD.h / 2 - (y * grid - cam.y) / cam.scale,
    size: grid / cam.scale,
  };
}

// Spec 091: what the engine is sent, worked out from the referee's state.
// Pure, so `useRoom` only decides when.

type Command = Record<string, unknown>;

/** The room's scene from scratch, with the hero and doors where the state has them. */
export function sceneCommands(
  script: RoomScript,
  hero: PartyHero,
  state: RoomState,
  reducedMotion: boolean,
): Command[] {
  const grid = script.map.gridSize;
  const out: Command[] = [...roomSetup(script, hero, { reducedMotion })];
  if (state.hero.x !== script.start.x || state.hero.y !== script.start.y) {
    out.push(rollbackCommand(hero, state.hero, grid));
  }
  for (const [id, door] of Object.entries(state.doors)) {
    const wall = script.map.walls.find((w) => w.id === id);
    if (wall && door !== wall.doorState) out.push(doorCommand(wall, door));
  }
  if (state.goals.includes("chest-opened")) out.push(...chestCommand(script, true));
  return out;
}

const companyId = (slug: string) => `company-${slug}`;

/** The rest of the party, standing on `squares`: everyone but the hero played. */
export function companyCommands(script: RoomScript, hero: PartyHero, squares: readonly Point[]): Command[] {
  const grid = script.map.gridSize;
  const rest = PARTY.filter((p) => p.slug !== hero.slug);
  return squares.flatMap((sq, i) => {
    const p = rest[i];
    if (!p) return [];
    const token = { id: companyId(p.slug), x: sq.x * grid, y: sq.y * grid, z: 2, label: p.name, tokenType: "character", photoUrl: p.art };
    return [{ type: "upsert_token", token }];
  });
}

/** Takes every company token away, whoever stood where. */
export const companyTeardown = (): Command[] =>
  PARTY.map((p) => ({ type: "remove_token", tokenId: companyId(p.slug) }));

/** What a verdict changes on the table: a hero sent back, a door swung, a chest opened. */
export function outcomeCommands(
  script: RoomScript,
  hero: PartyHero,
  { outcome }: Judged,
): Command[] {
  const out: Command[] = [];
  if (outcome.rollback) out.push(rollbackCommand(hero, outcome.rollback, script.map.gridSize));
  if (outcome.door) {
    const { id, state } = outcome.door;
    const wall = script.map.walls.find((w) => w.id === id);
    if (wall) out.push(doorCommand(wall, state));
  }
  if (outcome.announcement === "roll-succeeded") out.push(...chestCommand(script, true));
  return out;
}

/** FR-012: the line the page logs when the rule and the engine disagree. */
export function parityError(
  dir: string,
  from: RoomState["hero"],
  { outcome }: Judged,
  engineMoved: boolean,
): string {
  const rule = outcome.accepted ? "allows it" : `refuses it at ${outcome.wallId}`;
  return (
    `room parity: a step ${dir} from ${JSON.stringify(from)}: ` +
    `the rule ${rule}, the engine ${engineMoved ? "moved" : "refused"}`
  );
}
