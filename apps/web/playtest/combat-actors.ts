import { expect, type Page } from "@playwright/test";
import { graphql } from "../e2e/fixtures/helpers";
import { must, type Table } from "./table";

/**
 * The creatures in a fight: dice rolled at a seat, hit points changed and
 * shown, and the actor data (scores, traits, links, disclosure, status) a
 * sheet is built from.
 */

/** Roll in this client's own dice roller, and read the number it shows. */
export async function rollInPanel(
  page: Page,
  formula: string,
): Promise<number> {
  await page.getByTestId("dice-formula-input").fill(formula);
  await page.getByTestId("dice-roll-button").click();
  const result = page.getByTestId("dice-roll-result");
  // The canvas plays a throw before the number appears.
  await expect(result).toBeVisible({ timeout: 30_000 });
  const text = (await result.textContent())?.trim() ?? "";
  const match = /(-?\d+)\s*$/.exec(text);
  if (!match) throw new Error(`the roller showed no number: "${text}"`);
  return Number(match[1]);
}

/** Whatever this client's roller is showing, or null if it shows nothing. */
export async function rollShown(page: Page): Promise<string | null> {
  const result = page.getByTestId("dice-roll-result");
  if ((await result.count()) === 0) return null;
  return (await result.textContent())?.trim() ?? null;
}

/**
 * Spec 046 FR-014: the Game Master damages or heals a creature, through the
 * same mutation the tracker's Damage and Heal buttons call.
 */
export async function changeHitPoints(
  page: Page,
  tokenId: string,
  kind: "DAMAGE" | "HEALING",
  amount: number,
): Promise<{ current: number; max: number; temporary: number }> {
  const { changeHitPoints: after } = await must<{
    changeHitPoints: { current: number; max: number; temporary: number };
  }>(
    page,
    `mutation ($tokenId: UUID!, $kind: HitPointChange!, $amount: Int!) {
      changeHitPoints(tokenId: $tokenId, kind: $kind, amount: $amount) {
        current max temporary
      }
    }`,
    { tokenId, kind, amount },
  );
  return after;
}

/** What the server says when this client asks to change hit points. */
export async function refusalOfChangeHitPoints(
  page: Page,
  tokenId: string,
): Promise<string[]> {
  const result = await graphql<{ errors?: { message: string }[] }>(
    page,
    `
      mutation ($tokenId: UUID!) {
        changeHitPoints(tokenId: $tokenId, kind: DAMAGE, amount: 1) {
          current
        }
      }
    `,
    { tokenId },
  );
  return (result.errors ?? []).map((error) => error.message);
}

/**
 * The current figure of a token's first `resourceId` bar, as this client's
 * engine holds it, or null when the engine draws no exact figure for it.
 */
export async function barCurrentOn(
  page: Page,
  tokenId: string,
  resourceId = "hitPoints",
): Promise<number | null> {
  const json = await statusOn(page, tokenId);
  if (!json) return null;
  const resources = JSON.parse(json) as {
    definition: { id: string };
    disclosed: { disclosure: string; entries?: { current: number }[] };
  }[];
  const bar = resources.find((r) => r.definition.id === resourceId);
  return bar?.disclosed.entries?.[0]?.current ?? null;
}

/**
 * A token's hit points as the server resolves them for this viewer: its
 * actor's for a linked token, its own for a copy (spec 046 ADR-102). `null`
 * when the viewer is not shown the figure.
 */
export async function tokenHitPointsOf(
  page: Page,
  sceneId: string,
  tokenId: string,
): Promise<number | null> {
  const { tokenStatus } = await must<{
    tokenStatus: {
      tokenId: string;
      resources: {
        definitionId: string;
        entries: { current: number }[] | null;
      }[];
    }[];
  }>(
    page,
    `query ($sceneId: UUID!) {
      tokenStatus(sceneId: $sceneId) {
        tokenId
        resources { definitionId entries { current } }
      }
    }`,
    { sceneId },
  );
  const bar = tokenStatus
    .find((t) => t.tokenId === tokenId)
    ?.resources.find((r) => r.definitionId === "hitPoints");
  return bar?.entries?.[0]?.current ?? null;
}

/** Make a token its actor, or a copy of it (spec 046 FR-016). */
export async function setTokenLink(
  table: Table,
  tokenId: string,
  linked: boolean,
): Promise<void> {
  await must(
    table.gm,
    `mutation ($tokenId: UUID!, $linked: Boolean!) {
      setTokenLink(tokenId: $tokenId, linked: $linked) { tokenId }
    }`,
    { tokenId, linked },
  );
}

/** Mark an NPC a named individual, or not (spec 046 FR-016). */
export async function setActorUnique(
  table: Table,
  actorId: string,
  unique: boolean,
): Promise<void> {
  await must(
    table.gm,
    `mutation ($actorId: UUID!, $unique: Boolean!) {
      setActorUnique(actorId: $actorId, unique: $unique) { id }
    }`,
    { actorId, unique },
  );
}

/** Write a whole `resource_data` blob, as a sheet does. */
export async function setHitPoints(
  table: Table,
  actorId: string,
  hp: { current: number; max: number; temporary?: number },
): Promise<void> {
  await must(
    table.gm,
    `mutation ($input: GraphQLUpdateActorSystemDataInput!) {
      updateActorSystemData(input: $input) { id }
    }`,
    {
      input: {
        actorId,
        gameSystemId: table.system,
        dataType: "resource_data",
        // Whole, because the column is replaced and `max_hp` is required:
        // a write of `current_hp` alone is refused by the pack's validator.
        data: {
          current_hp: hp.current,
          max_hp: hp.max,
          temporary_hp: hp.temporary ?? 0,
        },
      },
    },
  );
}

/** The six scores a 5e actor must have before anything else will validate. */
export async function setAbilityScores(
  table: Table,
  actorId: string,
  scores: Record<string, number>,
): Promise<void> {
  await must(
    table.gm,
    `mutation ($input: GraphQLUpdateActorSystemDataInput!) {
      updateActorSystemData(input: $input) { id }
    }`,
    {
      input: {
        actorId,
        gameSystemId: table.system,
        dataType: "ability_data",
        data: scores,
      },
    },
  );
}

/**
 * Set a character's traits — where D&D 5e keeps darkvision (spec 045 US6).
 *
 * The same mutation `setAbilityScores` uses, on the slot the system's own
 * `vision` block names. Written through the product's API rather than into
 * the database, so what the playtest proves is what a Game Master editing a
 * sheet would get.
 */
export async function setTraits(
  table: Table,
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
        gameSystemId: table.system,
        dataType: "trait_data",
        data: traits,
      },
    },
  );
}

/**
 * The scene's grid size, in world units per cell.
 *
 * Read from the scene rather than assumed, because a distance a game system
 * quotes in feet only becomes a distance on a board through this number —
 * which is what spec 045 US6 converts through.
 */
export async function gridSizeOf(table: Table): Promise<number> {
  const { scene } = await must<{ scene: { gridSize: number } }>(
    table.gm,
    `query ($sceneId: UUID!) { scene(sceneId: $sceneId) { gridSize } }`,
    { sceneId: table.sceneId },
  );
  return scene.gridSize;
}

/** What this client's engine believes one token can see, in world units. */
export async function darkvisionOn(
  page: Page,
  tokenId: string,
): Promise<number | null> {
  return page.evaluate(
    (tokenId) =>
      (
        window as unknown as {
          __engineProbe?: { tokenVision?: (id: string) => number | null };
        }
      ).__engineProbe?.tokenVision?.(tokenId) ?? null,
    tokenId,
  );
}

export interface ActorSystemData {
  abilityData: Record<string, unknown> | null;
  resourceData: Record<string, number> | null;
  traitData: Record<string, unknown> | null;
  spellData: Record<string, unknown> | null;
}

export async function systemDataOf(
  page: Page,
  actorId: string,
): Promise<ActorSystemData> {
  const { actorSystemData } = await must<{ actorSystemData: ActorSystemData }>(
    page,
    `query ($actorId: UUID!) {
      actorSystemData(actorId: $actorId) {
        abilityData resourceData traitData spellData
      }
    }`,
    { actorId },
  );
  return actorSystemData;
}

/** How much a token tells the table about one of its resources. */
export async function setDisclosure(
  table: Table,
  tokenId: string,
  resourceId: string,
  state: "VISIBLE" | "GREYED" | "PERCENTAGE" | "CHUNKED",
): Promise<void> {
  await must(
    table.gm,
    `mutation ($input: SetTokenDisclosureInput!) {
      setTokenDisclosure(input: $input) { tokenId }
    }`,
    { input: { tokenId, resourceId, state } },
  );
}

/**
 * What this client's *engine* holds for a token's status bars, as JSON.
 *
 * Read from the engine rather than the server on purpose: the question a
 * playtest asks is what each board shows, and the gap between the two is
 * exactly what the scenario is looking for.
 */
export async function statusOn(
  page: Page,
  tokenId: string,
): Promise<string | null> {
  const status = await page.evaluate(async (id) => {
    const mod = (await import(
      /* @vite-ignore */ "/src/engine/bevy/tokenStatus.ts"
    )) as typeof import("../src/engine/bevy/tokenStatus");
    return (await mod.readTokenStatus(id)) as unknown;
  }, tokenId);
  return status === null || status === undefined
    ? null
    : JSON.stringify(status);
}
