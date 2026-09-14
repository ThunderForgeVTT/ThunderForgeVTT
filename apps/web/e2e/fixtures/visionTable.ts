import { expect, type Browser, type Page } from "@playwright/test";
import {
  graphql,
  inviteAndJoinAsPlayer,
  launchSceneByName,
  registerAndCreateWorld,
  uniqueSuffix,
} from "./helpers";

/**
 * A dark D&D 5e scene with players at it — the table spec 045's game-system
 * vision is judged at (`carried-light.spec.ts`, `darkvision-range.spec.ts`).
 *
 * D&D 5e because it is the system that declares both halves: darkvision and a
 * carried light, each read from a character's `traitData` in feet
 * (`packs/systems/dnd5e/system.json`, `vision`). Genie declares neither yet.
 */

/**
 * World units per grid square on these scenes.
 *
 * Chosen, not defaulted: the server's default is 5, which makes sixty feet
 * sixty world units — a range a token's own sprite is wider than. At 50 a
 * five-foot square is 50 units, so ten feet is 100 and every distance below
 * reads as feet times ten.
 */
export const GRID = 50;

/** Feet to world units at `GRID`, as the server converts them. */
export function feet(distance: number): number {
  return (distance / 5) * GRID;
}

type Gql<T> = { data?: T; errors?: { message: string }[] };

async function must<T>(
  page: Page,
  query: string,
  variables: Record<string, unknown>,
): Promise<T> {
  const result = await graphql<Gql<T>>(page, query, variables);
  expect(result.errors, JSON.stringify(result.errors)).toBeUndefined();
  return result.data as T;
}

export type VisionTable = { gm: Page; worldId: string; sceneId: string };

/** A Game Master's D&D 5e world, and a dark scene in it everyone can open. */
export async function openDarkDnd5eScene(
  gm: Page,
  name: string,
): Promise<VisionTable> {
  const worldId = await registerAndCreateWorld(
    gm,
    `E2E ${name} ${uniqueSuffix()}`,
  );
  await must(
    gm,
    `mutation ($input: UpdateWorldGameSystemInput!) {
      updateWorldGameSystem(input: $input) { id }
    }`,
    { input: { worldId, gameSystemId: "dnd5e" } },
  );
  const { createScene } = await must<{ createScene: { sceneId: string } }>(
    gm,
    `mutation ($input: GraphQLCreateSceneInput!) {
      createScene(input: $input) { sceneId }
    }`,
    { input: { worldId, name, gridSize: GRID } },
  );
  const sceneId = createScene.sceneId;
  await must(
    gm,
    `mutation ($sceneId: UUID!) {
      updateSceneHidden(sceneId: $sceneId, hidden: false) { sceneId }
    }`,
    { sceneId },
  );
  await must(
    gm,
    `mutation ($sceneId: UUID!) {
      updateSceneAmbientLight(sceneId: $sceneId, ambientLight: "dark") { sceneId }
    }`,
    { sceneId },
  );
  await launchSceneByName(gm, worldId, name);
  return { gm, worldId, sceneId };
}

/** A player who has joined the world, and their account id. */
export async function joinPlayer(
  browser: Browser,
  table: VisionTable,
): Promise<{ page: Page; userId: string }> {
  const page = await inviteAndJoinAsPlayer(browser, table.gm, table.worldId);
  const { me } = await must<{ me: { id: string } }>(
    page,
    `query { me { id } }`,
    {},
  );
  return { page, userId: me.id };
}

/**
 * A D&D 5e creature and its token. Given an owner, the token is that player's
 * own and primary — the eyes their canvas sees through.
 */
export async function placeCreature(
  table: VisionTable,
  options: { label: string; x: number; y: number; ownerUserId?: string },
): Promise<{ actorId: string; tokenId: string }> {
  const { createActor } = await must<{ createActor: { id: string } }>(
    table.gm,
    `mutation ($input: CreateActorInput!) { createActor(input: $input) { id } }`,
    {
      input: {
        worldId: table.worldId,
        label: options.label,
        isNpc: !options.ownerUserId,
        gameSystemId: "dnd5e",
      },
    },
  );
  const { createToken } = await must<{ createToken: { tokenId: string } }>(
    table.gm,
    `mutation ($input: GraphQLCreateTokenInput!) {
      createToken(input: $input) { tokenId }
    }`,
    {
      input: {
        sceneId: table.sceneId,
        actorId: createActor.id,
        x: options.x,
        y: options.y,
        tokenType: options.ownerUserId ? "character" : "npc",
      },
    },
  );
  if (options.ownerUserId) {
    await must(
      table.gm,
      `mutation ($tokenId: UUID!, $input: GraphQLUpdateTokenInput!) {
        updateToken(tokenId: $tokenId, input: $input) { tokenId }
      }`,
      {
        tokenId: createToken.tokenId,
        input: { ownerUserId: options.ownerUserId, isPrimary: true },
      },
    );
  }
  return { actorId: createActor.id, tokenId: createToken.tokenId };
}

/**
 * Write a character's traits, as a Game Master editing the sheet would — the
 * slot D&D 5e's `vision` block reads darkvision and a carried light from.
 */
export async function setTraits(
  table: VisionTable,
  actorId: string,
  traits: Record<string, unknown>,
): Promise<void> {
  await must(
    table.gm,
    `mutation ($input: GraphQLUpdateActorSystemDataInput!) {
      updateActorSystemData(input: $input) { id }
    }`,
    {
      input: {
        actorId,
        gameSystemId: "dnd5e",
        dataType: "trait_data",
        data: traits,
      },
    },
  );
}

/** A vision-blocking wall from one point to another. */
export async function addWall(
  table: VisionTable,
  from: { x: number; y: number },
  to: { x: number; y: number },
): Promise<void> {
  await must(
    table.gm,
    `mutation ($input: GraphQLCreateWallInput!) {
      createWall(input: $input) { wallId }
    }`,
    {
      input: {
        sceneId: table.sceneId,
        x1: from.x,
        y1: from.y,
        x2: to.x,
        y2: to.y,
        blocksVision: true,
      },
    },
  );
}
